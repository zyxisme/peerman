# Peerman

DN42 peer management web application. Manage WireGuard tunnels and BGP sessions through a web UI, with automatic config generation.

## Stack

| Layer | Tech |
|-------|------|
| Backend | Rust, tonic (gRPC), axum, sqlx (SQLite) |
| Frontend | React 18, TypeScript, Vite, shadcn/ui, Tailwind CSS |
| API | gRPC-Web (tonic-web, no envoy sidecar) |
| Design | Vercel-inspired design system (see [DESIGN.md](DESIGN.md)) |

## Quick Start

```bash
# Build (compiles frontend + backend into single binary)
cargo build --release

# Run (copy config.toml.example to config.toml first, or use -c)
./target/release/peerman -c config.toml
```

Open `http://localhost:3000` in your browser.

## Development

```bash
# Backend (uses config.toml by default, copy config.toml.example first)
cargo run -- -c config.toml

# Frontend dev server (with hot reload, proxies /api to backend)
cd frontend && pnpm dev

# Skip frontend build during Rust compilation (use pre-built dist/)
SKIP_FRONTEND_BUILD=1 cargo build
```

## Project Structure

```
src/            # Rust backend
  grpc/         # gRPC service implementations
  models/       # Data models + SQLite repositories
  services/     # WireGuard keygen, BIRD config gen, validation
frontend/       # React frontend (Vite)
  src/
    components/ # UI components (layout, peers, settings, config)
    hooks/      # gRPC client hooks
    lib/        # gRPC client + proto stubs
proto/          # Protobuf service definitions
migrations/     # SQLite schema migrations
```

## Features

- **Peer CRUD** — Create, view, edit, delete DN42 peers
- **WireGuard keys** — Generate Curve25519 keypairs in-app
- **Config generation** — WireGuard INI and BIRD2 `protocol bgp` blocks
- **Multi-mode BGP** — MP-BGP with extended nexthop, separate IPv4/IPv6 sessions
- **Batch export** — Export all peer configs at once
- **Settings** — Global defaults (ASN, BIRD template name, port, prefixes)

## Configuration

Peerman uses a TOML config file. Copy `config.toml.example` to `config.toml` and edit as needed.

```toml
[server]
listen_addr = "0.0.0.0:3000"

[storage]
db_path = "data/peerman.db"

[logging]
level = "info"

[cluster]
node_name = ""           # set to enable cluster mode
bootstrap_nodes = []     # cluster peer addresses
probe_interval_secs = 60
sync_interval_secs = 30
```

CLI only takes one argument:

```
peerman -c config.toml   # defaults to ./config.toml if omitted
```

## gRPC API

The gRPC-Web API is served at `/api/`. Proto definitions in [proto/peerman.proto](proto/peerman.proto).

```protobuf
service PeerService {
  rpc ListPeers, GetPeer, CreatePeer, UpdatePeer, DeletePeer,
      TogglePeer, GenerateKeypair, GetWireGuardConfig,
      GetBirdConfig, ExportAllWireGuard, ExportAllBird
}

service SettingsService {
  rpc GetSettings, SaveSettings
}
```
