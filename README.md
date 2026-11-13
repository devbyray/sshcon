# sshcon

**The easiest way to re-connect to SSH connections**

`sshcon` is a small, local connection picker for OpenSSH. Save memorable names for your servers, choose one from an interactive list, or connect directly by name. `sshcon` passes the selected target to your installed `ssh` command; it does not implement SSH or manage credentials.

## Features

- Keep a simple JSON connection library in `~/.sshcon`.
- Select a saved connection with the arrow keys instead of retyping its address.
- Connect directly with `sshcon connect <name>`.
- Use IP addresses, hostnames, Tailscale MagicDNS names, `user@host` targets, or `ssh://` URLs.
- Let OpenSSH handle keys, passwords, agents, host verification, and `~/.ssh/config`.
- Run locally without an account, cloud service, or `sshcon` server.

## Requirements

- OpenSSH's `ssh` command must be installed and available on your `PATH`.
- Rust and Cargo are needed only when installing from source.

## Install

Install the latest source version with Cargo:

```sh
cargo install --git https://github.com/devbyray/sshcon
```

Alternatively, clone the repository and install the checked-out version:

```sh
git clone https://github.com/devbyray/sshcon.git
cd sshcon
cargo install --path .
```

Cargo installs the `sshcon` executable in its binary directory. If your shell cannot find the command afterward, add Cargo's binary directory to your `PATH` (usually `~/.cargo/bin`).

## Quick start

Initialize the local configuration, add a connection, and open the picker:

```sh
sshcon init
sshcon add --name JellyFin --connection jellyfin
sshcon list
```

Use the arrow keys to select a connection and press **Enter** to connect. Press **Esc** to cancel.

Connect directly by its saved name whenever you like:

```sh
sshcon connect JellyFin
```

Names are matched without regard to capitalization, so `sshcon connect jellyfin` works too. Running `sshcon connect` with no name opens the same picker.

## Add connections

The `--connection` value is passed to OpenSSH as a single target argument. Examples:

```sh
# An IP address
sshcon add --name Proxmox --connection 192.168.1.10

# A hostname or Tailscale MagicDNS name
sshcon add --name JellyFin --connection jellyfin

# A target that includes the remote username
sshcon add --name HomeAssistant --connection admin@ha.example.ts.net

# An SSH URL, with an optional username and port
sshcon add --name Router --connection ssh://admin@192.168.1.1:2222
```

Only `ssh://` URLs are accepted. URL passwords, paths, query parameters, and fragments are rejected. To use a non-default port with a plain host target, configure it in `~/.ssh/config` and save that host alias.

## Commands

| Command | Description |
| --- | --- |
| `sshcon init` | Create `~/.sshcon` and its JSON files if they do not exist. Safe to run more than once. |
| `sshcon add --name <name> --connection <target>` | Add a named SSH target to the local library. |
| `sshcon list` | In a terminal, open a picker and connect to the selected host. When output is piped, print the saved connections without connecting. |
| `sshcon connect <name>` | Connect directly to a saved host. Names are case-insensitive. |
| `sshcon connect` | Open the interactive picker. |

## Local files

`sshcon init` creates these files in the current user's home directory:

```text
~/.sshcon/
├── config.json
└── connections.json
```

On Windows, `~` means your user profile directory. The connection library is plain JSON and can be backed up or edited directly:

```json
{
  "version": 1,
  "connections": [
    {
      "name": "JellyFin",
      "connection": "jellyfin"
    },
    {
      "name": "Proxmox",
      "connection": "192.168.1.10"
    }
  ]
}
```

The library contains only names and SSH targets. `sshcon` does not store keys or passwords, invoke a shell, or change your OpenSSH configuration. Authentication and host-key verification are handled by OpenSSH using its normal settings.

## Build from source

```sh
cargo build --release
```

The executable is written to `target/release/sshcon` (or `target/release/sshcon.exe` on Windows).

## License

MIT. See [LICENSE](LICENSE).
