# Peerman — DN42 Peer Manager

Rust backend (tonic + axum + sqlx) + React frontend (Vite + TypeScript + Tailwind CSS), gRPC-Web API, single binary.

## Build

- `source "$HOME/.cargo/env"` to put cargo/rustup on PATH (not in default env)
- `cargo build` — full build (proto gen → frontend pnpm build → rust compile → embed dist/)
- `SKIP_FRONTEND_BUILD=1 cargo build` — skip frontend, use pre-built dist/ (needed on low-RAM machines; debug build may OOM linking)
- `cargo build --release` — release build (smaller binary, less linking memory)
- `cd frontend && pnpm dev` — Vite dev server (proxies /api to localhost:3000)
- `cargo run -- -c config.toml` — start server (copy config.toml.example first)
- `cd frontend && pnpm run build` — build frontend only

## Testing

- `cargo test` — run all 36 unit tests (validation, wireguard, bird, probe, cluster)
- `cargo clippy` — lint check
- `cd frontend && pnpm exec tsc --noEmit` — TypeScript type-check

## Key crate/version constraints

- `tonic 0.12` depends on `axum 0.7` — do NOT upgrade axum past 0.7
- `x25519-dalek 2.x` requires `features = ["static_secrets"]` for StaticSecret
- `sqlx` with `sqlite` feature = bundled SQLite (compiled from C, no system lib needed)
- Frontend uses pnpm (not npm), `packageManager: "pnpm@11.1.3"` in package.json

## Config

- Config via TOML file: `cargo run -- -c config.toml` (defaults to `./config.toml`). Copy `config.toml.example` as a starting point.
- `Cli` struct (clap) handles only `-c`/`--config`. `Config` struct has five sections: `[server]`, `[storage]`, `[logging]`, `[auth]`, `[cluster]`.
- **Serde default gotcha:** `#[serde(default)]` on a struct only fills in when the entire section is absent from TOML. For partial sections, missing fields ERROR unless annotated with `#[serde(default = "fn_name")]` pointing to a function. See `default_listen_addr()` etc. in `src/config.rs` for the pattern.

## Patterns

- **Proto conversion**: `Peer::apply_proto()` in `src/models/peer.rs` handles proto→model field mapping; `peer_to_proto()` in `peer_service.rs` for model→proto
- **Validation**: `validate_peer_fields()` in `peer_service.rs` centralizes all peer field validation (name, ASN, WG key, tunnel IPs)
- **Dynamic SQL**: Use `sqlx::QueryBuilder` for queries with optional WHERE clauses (see `ProbeResultRepository::list_by_filters`)
- **Graceful shutdown**: `tokio_util::sync::CancellationToken` propagated to all background tasks (stale cleanup, probe, flap detection) via `tokio::select!`

## sqlx — use runtime API, not macros

sqlx macros (`query_as!`, `query!`) need DATABASE_URL at compile time. Use runtime versions instead:
- `sqlx::query_as::<_, T>(sql)` + `.bind()` + `.fetch_all(&pool)`
- `sqlx::query(sql)` + `.bind()` + `.execute(&pool)`
- Models derive `sqlx::FromRow` for `query_as` to work

## Proto & gRPC

- `tonic::include_proto!("peerman")` puts generated types directly in the calling module (no `peerman` submodule wrapper). Note: prost normalizes consecutive capitals — `WG` becomes `Wg` in Rust type names (`WgInterface`, `GetWgStatusRequest`), but frontend TS keeps original casing (`WGInterface`).
- `build.rs` runs `tonic_build::compile_protos("proto/peerman.proto")` → `$OUT_DIR/peerman.rs`
- Frontend proto: `protoc -I proto --es_out frontend/src/lib --es_opt target=ts peerman.proto` with `protoc-gen-es` in PATH (`frontend/node_modules/.bin`)
- `@connectrpc/connect v2`: use `createClient()` (not `createPromiseClient`), messages via `create(Schema, {...})`

## SQLite WAL

PRAGMA journal_mode=WAL must run OUTSIDE a transaction. Set it before `sqlx::migrate!()`, not inside the migration SQL.

## DESIGN.md

Vercel-inspired design system. Tokens (colors, typography, spacing, border-radius, shadows) mapped to `tailwind.config.ts`.
All UI uses custom CSS component classes (`@apply` in `globals.css`): `.card`, `.btn-primary`, `.btn-secondary-sm`, `.form-input`, `.data-table`, `.code-block`, `.badge`, `.tab-ghost`, `.tab-active`. No shadcn/ui components in use despite packages installed.
Geist/Inter fonts loaded from Google Fonts CDN.

## Cluster mode

- Set `node_name` in `[cluster]` section to enable cluster mode. Without it, cluster features are dormant.
- `peer_nodes = ["host:port", ...]` — initial bootstrap peers (public IPs). Nodes exchange full membership via `ExchangeNodes` gossip.
- `cluster_key = "shared-secret"` — shared secret for inter-node gRPC auth (`x-cluster-key` metadata).
- `probe_interval_secs` (default 60) — health check interval. `sync_interval_secs` (default 30) — anti-entropy exchange interval.
- `migrations/002_cluster.sql` adds `nodes`, `probe_results`, `community_rules` tables + `origin_node_id` on `peers`.

## Inter-node communication (gRPC client)

- `build.rs` uses `build_client(true)` to generate client stubs alongside server stubs.
- **Outbound gRPC from server handlers:** use `tonic::transport::Endpoint::from_shared(uri).connect().await` to build a channel, then `ClusterServiceClient::new(channel)`. The generated client module is at `crate::grpc::generated::cluster_service_client::ClusterServiceClient`.
- **Metadata injection:** `"key".parse()` converts a `&str` to `tonic::metadata::MetadataValue` for inserting into request headers.
- `tonic::Request` does NOT implement Clone — reconstruct per destination node.
- `into_router()` is deprecated in tonic 0.12; `into_axum_router()` not available on Server::Router. `main.rs` suppresses with `#[allow(deprecated)]`.

## Cluster module (`src/cluster/`)

- `auth.rs` — `check_cluster_key()` validates `x-cluster-key` metadata against shared secret
- `cache.rs` — `ClusterCache` in-memory cache with partial update methods (`update_peers`, `update_probe_results`, `update_community_rules`), keyed by node `listen_addr`
- `aggregator.rs` — `ClusterAggregator` with `fanout_peers()`, `fanout_probe_results()`, `fanout_community_rules()`, `health_check()`, `exchange_with()`; 2s timeout, cache fallback on failure
- `tunnel.rs` — Cluster inter-node WG tunnel management: keypair generation, tunnel IP assignment from `tunnel_ip_range`, `sync_cluster_wg()` writes `/etc/wireguard/wg-cluster.conf` + `wg syncconf`, `sync_cluster_bird()` regenerates bird.conf with iBGP full mesh.
- **Dual auth pattern:** inter-node RPCs (`push_peer`, `push_probe_result`, `save_community_rule`) accept EITHER JWT (user) OR cluster key (node) — check: `jwt_ok || cluster_ok`
- **Health check:** gRPC `HealthCheck` + ICMP ping; 2 consecutive failures → offline, 2 successes → online (flap suppression)
- **Write proxy:** `create_peer`/`update_peer` proxy to target node via `PushPeer` when `origin_node_id != listen_addr`
- **Node discovery:** startup `ExchangeNodes` with all `peer_nodes` + periodic anti-entropy with random peer every `sync_interval_secs`

## WG & BIRD auto-apply

- **Peer CRUD → auto-apply:** `create_peer`/`update_peer`/`delete_peer`/`toggle_peer` call `auto_apply_wg_bird()` which regenerates `/etc/wireguard/wg0.conf` → `wg syncconf` and `/etc/bird/bird.conf` → `birdc configure`. No manual apply step needed.
- **Cluster interconnect:** `wg-cluster` interface managed via `sync_cluster_wg()`, node keypairs auto-generated and exchanged via `ExchangeNodes` gossip (new fields: `wg_pubkey`, `tunnel_ip`).
- **iBGP full mesh:** `generate_ibgp_blocks()` creates `protocol bgp node_<name>` blocks for all nodes with assigned tunnel IPs, using `direct` + `next hop self yes`.
- **Atomic writes:** Config files written to `.tmp` then `rename` to avoid partial reads.
- **Config key:** `[cluster] tunnel_ip_range = "10.255.0.0/24"` — internal tunnel IP pool for inter-node iBGP.

## ManagementService gRPC

- `GetWireGuardStatus` — parses `wg show <iface> dump` output into structured `WgInterface`/`WgPeerStatus` proto messages. Defaults to `all` interfaces.
- `GetBirdStatus` — parses `birdc show protocols` output into `BirdProtocol` messages (name, proto, state, since, info).
- Frontend: `/status` page displays both WG peers (endpoint, handshake, RX/TX) and BIRD protocols (state-colored badges). Read-only, no apply buttons.

## Frontend gotchas

- Proto `int64` fields become TypeScript `bigint` — cannot render directly in React. Use `String(v)` or `.toString()`.
- After proto changes, regenerate TS stubs: `PATH="frontend/node_modules/.bin:$PATH" protoc -I proto --es_out frontend/src/lib --es_opt target=ts proto/peerman.proto`
- `pnpm exec tsc --noEmit` for fast type-check without full build.

## Timestamps in SQLite

- All timestamps use RFC3339 UTC (`chrono::Utc::now().to_rfc3339()`). String comparison works because UTC offsets are consistent.
- Use `chrono::Duration::seconds(n)` / `Duration::days(n)` for threshold arithmetic, NOT raw Unix timestamp strings.

## Community & probe

- `CommunityRuleRepository::seed_defaults()` auto-populates 5 latency-tier rules on first run (empty table).
- `services::probe::ping()` is async (tokio `Command`) — calls `ping -c 5 -i 0.2 <target>`, parses rtt/packet-loss with regex cached in `OnceLock`.
- Probe results stored locally only; cross-node push via `PushProbeResult` RPC is defined but client-side not yet wired.

## BIRD integration (Looking Glass + Flap Detection)

- `BirdSocket::connect()` talks to `/var/run/bird.ctl` — Unix socket → welcome banner → `restrict\n` → command → parse 4-digit status codes (1xxx/2xxx=table rows, 0xxx=terminal, 8xxx/9xxx=error, ` ` prefix=continuation).
- `BgpListener` passive iBGP on `[::1]:1790` — BIRD connects as client. Parses BGP UPDATE messages to count per-prefix path changes. AddPath capability (code 69) negotiated in OPEN.
- BGP stream handling: `handle_session` must take `impl AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static` (owned, not `&mut`). tokio::spawn requires `'static`.
- Flap detection is dual-channel: iBGP listener (primary, real-time) → channel → FlapDetector; falls back to BIRD socket polling `show route all` every 30s comparing route snapshots.
- Looking Glass queries all cluster nodes: local via socket, remote via gRPC `ExecuteCommand` RPC (placeholder for now).
- Traceroute runs as subprocess: `traceroute -q 1 -w 1 -m 15 <target>`.
- `flap_events` table (migration 003) stores detected flaps — sources: `ibgp`/`socket`/`probe`.

## Auth (JWT + httpOnly cookie)

- Single admin user — credentials in `[auth]` config section. `jwt_secret` empty = auto-generate on startup.
- JWT issued via `POST /api/auth/login` (axum handler, not gRPC), stored as httpOnly cookie (30 day expiry).
- Write gRPC methods call `crate::auth::check_auth(&request, &secret)?` for per-method auth.
- **Tonic interceptor gotcha:** tonic's `.interceptor()` on Server::builder has type issues with tonic-web's GrpcWebLayer. Per-method checks are simpler and avoid this.
- **Axum State + tonic Router(.nest()) gotcha:** `.nest("/api", grpc_router)` requires both routers to have the same state type. Tonic's `into_router()` returns `Router<()>` which can't use `.with_state(S)`. Use `std::sync::OnceLock<Arc<Config>>` static for sharing config across HTTP handlers instead of axum's `State` extractor.
- `jsonwebtoken` crate for HS256 signing. `src/auth.rs` contains JWT utils + `check_auth()` helper.
- Frontend: `AuthProvider` in `main.tsx`, `ProtectedRoute` wrapping write pages, `LoginPage` at `/login`.

## Adding a new gRPC service

1. Define service + messages in `proto/peerman.proto`
2. `build.rs` auto-generates Rust stubs (tonic) — no extra config needed
3. Implement `XServiceImpl` in `src/grpc/X_service.rs`
4. Register in `main.rs`: import `XServiceServer`, instantiate, add to tonic router
5. Regenerate frontend stubs: `PATH="frontend/node_modules/.bin:$PATH" protoc -I proto --es_out frontend/src/lib --es_opt target=ts proto/peerman.proto`
6. Add client in `frontend/src/lib/grpc.ts`: `createClient(XService, transport)`
7. Add hooks in `frontend/src/hooks/useX.ts`
8. Add page component + route in `App.tsx` + nav item in `NavBar.tsx`
9. If service has DB state: model in `src/models/`, repository in same file, migration SQL, repo field on `AppState`
