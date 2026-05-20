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

# Run
./target/release/peerman --db-path data/peerman.db --listen-addr 0.0.0.0:3000
```

Open `http://localhost:3000` in your browser.

## Development

```bash
# Backend
cargo run -- --db-path data/peerman.db --log-level debug

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

## CLI

```
peerman --db-path <path> --listen-addr <addr> --log-level <level>

Options:
  --db-path       SQLite database path (default: data/peerman.db)
  --listen-addr   Bind address (default: 0.0.0.0:3000)
  --log-level     trace, debug, info, warn, error (default: info)
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
