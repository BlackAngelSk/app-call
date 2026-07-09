# app-call

A privacy-focused, peer-to-peer communication platform. No central servers, cryptographic identities, end-to-end encryption.

## Getting Started

### Install Rust (if needed)

```sh
./install.sh          # Linux / macOS
.\install.ps1         # Windows PowerShell
install.bat           # Windows cmd
```

### Build and Run

```sh
cargo run -p desktop
```

Or use the start scripts:

```sh
./start.sh            # Linux / macOS
.\start.ps1           # Windows PowerShell
start.bat             # Windows cmd
```

If you are using fish:

```fish
source $HOME/.cargo/env.fish
cargo run -p desktop
```

## P2P Networking

The app listens on TCP port **9000** (by default) on all interfaces (`0.0.0.0`).
When no GUI is available (e.g. inside a VM), it automatically falls back to a **networked console mode** with a terminal command interface.

### Console Commands

| Command | Description |
|---------|-------------|
| `connect <ip:port>` | Connect to a remote peer |
| `msg <text>` | Send a message to all connected peers |
| *(just type text)* | Anything that isn't a command is sent as a message |
| `peers` | List connected peers |
| `myid` | Show your identity and listen address |
| `port` | Show the listening port |
| `help` | Show available commands |
| `quit` / `exit` | Exit |

### Connecting Two Instances

**On the same machine (two terminals):**

Terminal 1:
```sh
APP_CALL_PORT=9000 cargo run -p desktop
```

Terminal 2:
```sh
APP_CALL_PORT=9001 cargo run -p desktop
# Then type: connect 127.0.0.1:9000
```

**Between two VMs / machines:**

1. Find the IP of Machine A: `hostname -I` or `ip addr`
2. On Machine A: `./start.sh` (listens on 0.0.0.0:9000)
3. On Machine B: `./start.sh` then type `connect <machine-a-ip>:9000`
4. Messages typed on either side appear on both

**Custom port:**

```sh
APP_CALL_PORT=12345 ./start.sh
```

### Firewall (VM / Server)

Make sure the TCP port is open:

```sh
# Ubuntu / Debian
sudo ufw allow 9000/tcp

# firewalld (Fedora / RHEL)
sudo firewall-cmd --add-port=9000/tcp --permanent
sudo firewall-cmd --reload

# Windows
netsh advfirewall firewall add rule name="app-call" dir=in action=allow protocol=TCP localport=9000
```

### How Discovery Works

1. Each instance generates a cryptographic identity (Ed25519 keypair) on first run
2. When you `connect` to one peer, they automatically share addresses of other peers they know (peer exchange)
3. Messages are relayed (gossip-flooded) to all connected peers
4. Message IDs are deduplicated so each message is displayed only once, even if it arrives via multiple paths

## Data Directory

On first start the app creates a persisted local identity:

- **Linux:** `$HOME/.local/share/app-call/identity.json`
- **Windows:** `%APPDATA%\app-call\identity.json`
- **Override:** set `APP_CALL_DATA_DIR` before running

## Current Scope

- Rust workspace with reusable core crate and native desktop binary
- Ed25519 + X25519 cryptographic identity (generated and persisted automatically)
- **TCP-based P2P networking** with handshake, peer exchange, and chat message relay
- **Networked console mode** that works headless / in VMs (auto-selected when no GPU is available)
- GUI mode using egui/eframe (on systems with GPU)
- User settings persistence

## Workspace Layout

```text
app-call/
  apps/
    desktop/          # Main binary (GUI + console)
  crates/
    app-core/         # Core library: identity, networking, models
  docs/
    technical-blueprint.md
```

## Next Steps

- encrypted message transport (Noise protocol / TLS)
- DHT-based peer discovery (no manual connect needed)
- offline mailbox replication
- voice and media channels
- Tor / I2P transport for metadata protection