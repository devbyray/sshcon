use std::{
    fs::{self, File, OpenOptions},
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use dialoguer::Select;
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use url::Url;

const APP_DIR: &str = ".sshcon";
const CONFIG_FILE: &str = "config.json";
const CONNECTIONS_FILE: &str = "connections.json";

#[derive(Debug, Parser)]
#[command(
    name = "sshcon",
    version,
    about = "Pick and connect to saved SSH hosts"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Create the local sshcon directory and its JSON files if missing.
    Init,
    /// Add a named OpenSSH target, such as a host, IP, or ssh:// URL.
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        connection: String,
    },
    /// Show saved connections and, in an interactive terminal, choose one to connect to.
    List,
    /// Connect by saved name, or choose interactively when no name is provided.
    Connect { name: Option<String> },
}

#[derive(Debug, Serialize, Deserialize)]
struct GlobalConfig {
    version: u32,
    connections_file: String,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            version: 1,
            connections_file: CONNECTIONS_FILE.to_owned(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConnectionLibrary {
    version: u32,
    connections: Vec<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Connection {
    name: String,
    connection: String,
}

struct Paths {
    dir: PathBuf,
    config: PathBuf,
    connections: PathBuf,
}

impl Paths {
    fn discover() -> Result<Self> {
        let home = dirs::home_dir().context("Could not determine your home directory")?;
        let dir = home.join(APP_DIR);
        Ok(Self {
            config: dir.join(CONFIG_FILE),
            connections: dir.join(CONNECTIONS_FILE),
            dir,
        })
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("sshcon: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let paths = Paths::discover()?;

    match cli.command {
        Commands::Init => {
            initialize(&paths)?;
            println!("Initialized {}", paths.dir.display());
            Ok(ExitCode::SUCCESS)
        }
        Commands::Add { name, connection } => {
            ensure_initialized(&paths)?;
            add_connection(&paths, name, connection)?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::List => {
            ensure_initialized(&paths)?;
            let library = read_library(&paths)?;
            match choose_connection(&library.connections, io::stdin().is_terminal())? {
                Some(connection) => connect(&connection),
                None => Ok(ExitCode::SUCCESS),
            }
        }
        Commands::Connect { name } => {
            ensure_initialized(&paths)?;
            let library = read_library(&paths)?;
            let selected = match name {
                Some(name) => find_connection(&library.connections, &name)?.clone(),
                None => match choose_connection(&library.connections, io::stdin().is_terminal())? {
                    Some(connection) => connection,
                    None => return Ok(ExitCode::SUCCESS),
                },
            };
            connect(&selected)
        }
    }
}

fn initialize(paths: &Paths) -> Result<()> {
    fs::create_dir_all(&paths.dir)
        .with_context(|| format!("Could not create {}", paths.dir.display()))?;
    create_json_if_missing(&paths.config, &GlobalConfig::default())?;
    create_json_if_missing(
        &paths.connections,
        &ConnectionLibrary {
            version: 1,
            connections: Vec::new(),
        },
    )?;
    Ok(())
}

fn create_json_if_missing<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not create {}", path.display()));
        }
    };
    serde_json::to_writer_pretty(&mut file, value)
        .with_context(|| format!("Could not write {}", path.display()))?;
    file.write_all(b"\n")?;
    Ok(())
}

fn ensure_initialized(paths: &Paths) -> Result<()> {
    ensure!(
        paths.config.is_file() && paths.connections.is_file(),
        "sshcon is not initialized. Run `sshcon init` first."
    );
    Ok(())
}

fn read_library(paths: &Paths) -> Result<ConnectionLibrary> {
    let file = File::open(&paths.connections)
        .with_context(|| format!("Could not open {}", paths.connections.display()))?;
    serde_json::from_reader(file)
        .with_context(|| format!("Invalid JSON in {}", paths.connections.display()))
}

fn write_library(paths: &Paths, library: &ConnectionLibrary) -> Result<()> {
    let temp_path = paths.connections.with_extension("json.tmp");
    let mut file = File::create(&temp_path)
        .with_context(|| format!("Could not write {}", temp_path.display()))?;
    serde_json::to_writer_pretty(&mut file, library)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(&temp_path, &paths.connections)
        .with_context(|| format!("Could not replace {}", paths.connections.display()))?;
    Ok(())
}

fn add_connection(paths: &Paths, name: String, connection: String) -> Result<()> {
    let name = name.trim();
    let connection = connection.trim();
    ensure!(!name.is_empty(), "Connection name cannot be empty");
    ensure!(!connection.is_empty(), "Connection target cannot be empty");
    validate_target(connection)?;

    let mut library = read_library(paths)?;
    ensure!(
        !library
            .connections
            .iter()
            .any(|item| item.name.eq_ignore_ascii_case(name)),
        "A connection named `{name}` already exists"
    );
    library.connections.push(Connection {
        name: name.to_owned(),
        connection: connection.to_owned(),
    });
    write_library(paths, &library)?;
    println!("Added `{name}` → `{connection}`");
    Ok(())
}

fn validate_target(target: &str) -> Result<()> {
    ensure!(
        !target.starts_with('-'),
        "Connection cannot start with `-` because OpenSSH would treat it as an option"
    );
    ensure!(
        !target.chars().any(char::is_whitespace),
        "Connection must be a single host/target without spaces"
    );
    ensure!(
        !target.chars().any(char::is_control),
        "Connection cannot contain control characters"
    );
    if target.contains("://") {
        let url = Url::parse(target).context("Invalid connection URL")?;
        ensure!(url.scheme() == "ssh", "Only `ssh://` URLs are supported");
        ensure!(url.host_str().is_some(), "SSH URL must include a host");
        ensure!(
            url.password().is_none(),
            "Passwords in SSH URLs are not supported"
        );
        ensure!(
            url.query().is_none(),
            "Query parameters in SSH URLs are not supported"
        );
        ensure!(
            url.fragment().is_none(),
            "Fragments in SSH URLs are not supported"
        );
        ensure!(
            url.path().is_empty() || url.path() == "/",
            "Paths in SSH URLs are not supported"
        );
    }
    Ok(())
}

fn choose_connection(connections: &[Connection], interactive: bool) -> Result<Option<Connection>> {
    if connections.is_empty() {
        println!(
            "No saved connections. Add one with `sshcon add --name <name> --connection <host>`. "
        );
        return Ok(None);
    }
    if !interactive {
        for connection in connections {
            println!("{}\t{}", connection.name, connection.connection);
        }
        return Ok(None);
    }

    let labels: Vec<String> = connections
        .iter()
        .map(|item| format!("{}  ({})", item.name, item.connection))
        .collect();
    let selection = Select::new()
        .with_prompt("Select an SSH connection")
        .items(&labels)
        .default(0)
        .interact_opt()?;
    Ok(selection.map(|index| connections[index].clone()))
}

fn find_connection<'a>(connections: &'a [Connection], name: &str) -> Result<&'a Connection> {
    connections
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case(name))
        .with_context(|| {
            let available = connections
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            if available.is_empty() {
                format!("No saved connections. Add one with `sshcon add`; run `sshcon list` to choose interactively")
            } else {
                format!("Unknown connection `{name}`. Saved connections: {available}")
            }
        })
}

fn connect(connection: &Connection) -> Result<ExitCode> {
    let mut command = Command::new("ssh");
    command.args(ssh_arguments(&connection.connection)?);
    let status = command
        .status()
        .context("Could not start `ssh`. Install OpenSSH or make it available on PATH")?;
    Ok(status
        .code()
        .map(|code| ExitCode::from(code.clamp(0, 255) as u8))
        .unwrap_or(ExitCode::FAILURE))
}

fn ssh_arguments(target: &str) -> Result<Vec<String>> {
    if !target.contains("://") {
        validate_target(target)?;
        return Ok(vec![target.to_owned()]);
    }

    validate_target(target)?;
    let url = Url::parse(target)?;
    let host = url.host_str().context("SSH URL must include a host")?;
    let decoded_user = percent_decode_str(url.username()).decode_utf8_lossy();
    ensure!(
        !decoded_user
            .chars()
            .any(|character| matches!(character, '@' | ':' | '/' | '\\' | ' ' | '\t')),
        "SSH URL contains an unsupported username"
    );

    let mut args = Vec::new();
    if let Some(port) = url.port() {
        args.push("-p".to_owned());
        args.push(port.to_string());
    }
    let destination = if decoded_user.is_empty() {
        host.to_owned()
    } else {
        format!("{decoded_user}@{host}")
    };
    args.push(destination);
    Ok(args)
}
