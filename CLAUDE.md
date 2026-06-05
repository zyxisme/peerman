# Peerman — DN42 Peer Manager

Rust backend (tonic + axum + sqlx) + React frontend (Vite + TypeScript + Tailwind CSS), gRPC-Web API, single binary.

## Build

- `source "$HOME/.cargo/env"` to put cargo/rustup on PATH (not in default env)
- `cargo build` — full build (proto gen → frontend pnpm build → rust compile → embed dist/)
- `SKIP_FRONTEND_BUILD=1 cargo build` — skip frontend, use pre-built dist/ (needed on low-RAM machines; debug build may OOM linking)
- **build.rs dist/ logic:** if `frontend/dist/` exists → skip; if missing → build frontend via pnpm; if `SKIP_FRONTEND_BUILD` set → skip (dist must already exist)
- `cargo build --release` — release build (smaller binary, less linking memory)
- `cd frontend && pnpm dev` — Vite dev server (proxies /api to localhost:3000)
- `cargo run -- -c config.toml` — start server (copy config.toml.example first)
- `cd frontend && pnpm run build` — build frontend only

## Testing

- `cargo test` — run all 71 unit tests
- `cargo clippy` — lint check
- `cargo fmt` — format all Rust code (separate from clippy, run both after changes)
- `cd frontend && pnpm exec tsc --noEmit` — TypeScript type-check
- **Disk space:** Machine has ~30GB disk. If `cargo test` fails with disk errors, clean other projects' target dirs: `rm -rf <other-project>/target`
- **CI:** `.github/workflows/ci.yml` runs fmt, clippy, test, tsc on push/PR to master

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

- **Proto conversion**: `Peer::apply_proto()` in `src/models/peer.rs`; `peer_to_proto()` redacts `wg_private_key` (security)
- **Validation**: `src/services/validation.rs` — `validate_peer_fields()`, `validate_settings()`, `validate_host()`, `validate_wg_interface_name()`, `validate_port()`, `validate_asn()`, `validate_wg_public_key()`
- **Input sanitization**: `src/services/input_sanitizer.rs` — `validate_post_script()` blocks shell metacharacters in WG PostUp/PostDown
- **BIRD allowlist**: `src/services/bird_allowlist.rs` — Looking Glass restricted to `show *` commands only
- **Dynamic SQL**: Use `sqlx::QueryBuilder` for queries with optional WHERE clauses (see `ProbeResultRepository::list_by_filters`)
- **Graceful shutdown**: `tokio_util::sync::CancellationToken` propagated to all background tasks (stale cleanup, probe, flap detection) via `tokio::select!`
- **SQL column const**: `PEER_COLUMNS` in `src/models/peer.rs` deduplicates the 25-column SELECT list
- **SQL column constants**: `PEER_COLUMNS`, `NODE_COLUMNS`, `SETTINGS_COLUMNS` in respective model files. Use `format!("SELECT {CONST} FROM ...")` for all `query_as` calls.
- **Single-step create**: `PeerRepository::create_full()` does INSERT ... RETURNING in one query. Don't use the old two-step `create` + `apply_proto` + `update` pattern.
- **PeerState sub-struct**: `src/app_state.rs` has `PeerState` grouping `peer_repo`, `settings_repo`, `node_repo`. gRPC services hold `state: PeerState` instead of individual repo fields.

## sqlx — use runtime API, not macros

sqlx macros (`query_as!`, `query!`) need DATABASE_URL at compile time. Use runtime versions instead:
- `sqlx::query_as::<_, T>(sql)` + `.bind()` + `.fetch_all(&pool)`
- `sqlx::query(sql)` + `.bind()` + `.execute(&pool)`
- Models derive `sqlx::FromRow` for `query_as` to work

## Proto & gRPC

- `tonic::include_proto!("peerman")` puts generated types directly in the calling module (no `peerman` submodule wrapper). Note: prost normalizes consecutive capitals — `WG` becomes `Wg` in Rust type names (`WgInterface`, `GetWgStatusRequest`), but frontend TS keeps original casing (`WGInterface`).
- `build.rs` runs `tonic_build::compile_protos("proto/peerman.proto")` → `$OUT_DIR/peerman.rs`
- **Proto field numbering:** Settings fields go up to 26 (`confederation_local_asn`). Always check existing field numbers before adding new ones to avoid conflicts.
- Frontend proto: `protoc -I proto --es_out frontend/src/lib --es_opt target=ts peerman.proto` with `protoc-gen-es` in PATH (`frontend/node_modules/.bin`)
- `@connectrpc/connect v2`: use `createClient()` (not `createPromiseClient`), messages via `create(Schema, {...})`

## SQLite WAL

PRAGMA journal_mode=WAL must run OUTSIDE a transaction. Set it before `sqlx::migrate!()`, not inside the migration SQL.
- `PRAGMA foreign_keys = ON` and `PRAGMA busy_timeout = 5000` are set after pool creation in `src/db.rs`

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

- `auth.rs` — `check_cluster_key()` validates `x-cluster-key` metadata against shared secret (constant-time comparison)
- `aggregator.rs` — fan-out methods use `futures::future::join_all` for parallel execution; `call_node()` helper handles timeout + cache fallback. gRPC channels cached in `DashMap` pool via `ClusterAggregator::connect()` (pub(crate) static method).
- `cache.rs` — `ClusterCache` in-memory cache with partial update methods (`update_peers`, `update_probe_results`, `update_community_rules`), keyed by node `listen_addr`
- `aggregator.rs` — `ClusterAggregator` with `fanout_peers()`, `fanout_probe_results()`, `fanout_community_rules()`, `health_check()`, `exchange_with()`; 2s timeout, cache fallback on failure
- `tunnel.rs` — Cluster inter-node WG tunnel management: keypair generation, tunnel IP assignment from `tunnel_ip_range`, `sync_cluster_wg()` writes `/etc/wireguard/wg-cluster.conf` + `wg syncconf`, `sync_cluster_bird()` regenerates bird.conf with iBGP full mesh.
- **Dual auth pattern:** inter-node RPCs (`push_peer`, `push_probe_result`, `save_community_rule`) accept EITHER JWT (user) OR cluster key (node) — check: `jwt_ok || cluster_ok`
- **Health check:** gRPC `HealthCheck` + ICMP ping; 2 consecutive failures → offline, 2 successes → online (flap suppression)
- **Write proxy:** `create_peer`/`update_peer` proxy to target node via `PushPeer` when `origin_node_id != listen_addr`
- **Node discovery:** startup `ExchangeNodes` with all `peer_nodes` + periodic anti-entropy with random peer every `sync_interval_secs`

## WG & BIRD auto-apply

- **Peer CRUD → debounced apply:** `create_peer`/`update_peer`/`delete_peer`/`toggle_peer` set `config_dirty` flag. Background task applies WG+BIRD configs every 5 seconds if dirty. `apply_wg_bird()` in `src/grpc/peer_service.rs`.
- **Config file permissions:** Generated config files (wg0.conf, bird.conf, wg-cluster.conf) set to 0o600 before atomic rename.
- **WG keypair persistence:** Node WG private key stored in DB (`wg_private_key` column, migration 011). Only generated on first startup.
- **Cluster interconnect:** `wg-cluster` interface managed via `sync_cluster_wg()`, node keypairs auto-generated and exchanged via `ExchangeNodes` gossip (new fields: `wg_pubkey`, `tunnel_ip`).
- **iBGP full mesh:** `generate_ibgp_blocks()` creates `protocol bgp node_<name>` blocks for all nodes with assigned tunnel IPs, using `direct` + `next hop self yes`.
- **Atomic writes:** Config files written to `.tmp` then `rename` to avoid partial reads.
- **Config key:** `[cluster] tunnel_ip_range = "10.255.0.0/24"` — internal tunnel IP pool for inter-node iBGP.
- **IPv6 tunnels:** `[cluster] tunnel_ipv6_range = "fd42:cluster::/48"` — optional IPv6 tunnel pool. Nodes get both IPv4 + IPv6 tunnel IPs; iBGP generates dual-stack blocks.
- **BGP Confederation:** `enable_confederation = true` + `confederation_local_asn = 65000` in `[cluster]` — uses DN42 ASN as confederation ID, private ASN per node. Alternative to iBGP full mesh.
- **Dead code in cluster module:** `ClusterAggregator`, `ClusterCache`, `NodeStatus` impl blocks use `#[allow(dead_code)]` — pre-defined public API for cluster infrastructure not yet called from all code paths.

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
- `CommunityMapper::compute_communities()` matches probe results against rules to determine which DN42 communities apply to a peer. Called during `auto_apply_wg_bird()` when `enable_community_filters` is true.
- `CommunityMapper::latency_to_tier()` / `parse_community_tier()` / `best_latency_tier()` — tier extraction helpers (currently unused in production, available for future integration).
- `services::probe::ping()` is async (tokio `Command`) — calls `ping -c 5 -i 0.2 <target>`, parses rtt/packet-loss with regex cached in `OnceLock`.
- Probe results stored locally only; cross-node push via `PushProbeResult` RPC is defined but client-side not yet wired.

## BIRD integration (Looking Glass + Flap Detection)

- `BirdSocket::connect()` talks to `/var/run/bird.ctl` — Unix socket → welcome banner → `restrict\n` → command → parse 4-digit status codes (1xxx/2xxx=table rows, 0xxx=terminal, 8xxx/9xxx=error, ` ` prefix=continuation).
- `BgpListener` passive iBGP on `[::1]:1790` — BIRD connects as client. Parses BGP UPDATE messages to count per-prefix path changes. AddPath capability (code 69) negotiated in OPEN.
- BGP stream handling: `handle_session` must take `impl AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static` (owned, not `&mut`). tokio::spawn requires `'static`.
- Flap detection is dual-channel: iBGP listener (primary, real-time) → channel → FlapDetector; falls back to BIRD socket polling `show route all` every 30s comparing route snapshots.
- Looking Glass queries all cluster nodes: local via socket, remote via gRPC `ExecuteCommand` RPC (implemented via `ClusterAggregator::execute_bird_command`).
- **BFD support:** `enable_bfd = true` in settings generates `protocol bfd { interface "wg*"; interval 300ms; multiplier 3; }` in bird.conf.
- **DN42 community filters:** `enable_community_filters = true` generates `update_latency`/`update_bandwidth`/`update_crypto` + `dn42_import_filter`/`dn42_export_filter` functions using AS 64511 standard.
- **WireGuard lifecycle:** `RestartWireGuard` RPC runs `wg-quick down <iface> && wg-quick up <iface>`.
- Traceroute runs as subprocess: `traceroute -q 1 -w 1 -m 15 <target>`.
- `flap_events` table (migration 003) stores detected flaps — sources: `ibgp`/`socket`/`probe`.

## Auth (JWT + httpOnly cookie)

- Single admin user — credentials in `[auth]` config section. `jwt_secret` empty = auto-generate on startup.
- JWT issued via `POST /api/auth/login` (axum handler, not gRPC), stored as httpOnly cookie (1 hour expiry, Secure flag).
- **ALL** gRPC methods (read + write) call `crate::auth::check_auth(&request, &secret)?` for per-method auth.
- **Password hashing:** argon2id via `src/auth/password.rs`. Config supports `password_hash` (preferred) or `password` (auto-hashed on startup).
- **Rate limiting:** Login endpoint limited to 5 attempts/min per IP via `LoginRateLimiter` in `src/http/rate_limit.rs`.
- **Tonic interceptor gotcha:** tonic's `.interceptor()` on Server::builder has type issues with tonic-web's GrpcWebLayer. Per-method checks are simpler and avoid this.
- **Axum State + tonic Router(.nest()) gotcha:** `.nest("/api", grpc_router)` requires both routers to have the same state type. Tonic's `into_router()` returns `Router<()>` which can't use `.with_state(S)`. Use `std::sync::OnceLock<Arc<Config>>` static for sharing config across HTTP handlers instead of axum's `State` extractor.
- `jsonwebtoken` crate for HS256 signing. `src/auth/mod.rs` contains JWT utils + `check_auth()` helper; `src/auth/password.rs` has argon2id hash/verify.
- Frontend: `AuthProvider` in `main.tsx`, `ProtectedRoute` wrapping write pages, `LoginPage` at `/login`.

## Adding a new gRPC service

1. Define service + messages in `proto/peerman.proto`
2. `build.rs` auto-generates Rust stubs (tonic) — no extra config needed
3. Implement `XServiceImpl` in `src/grpc/X_service.rs`
4. Register in `main.rs`: import `XServiceServer`, instantiate, add to tonic router. Use `state.peer_state()` for repo dependencies.
5. Regenerate frontend stubs: `PATH="frontend/node_modules/.bin:$PATH" protoc -I proto --es_out frontend/src/lib --es_opt target=ts proto/peerman.proto`
6. Add client in `frontend/src/lib/grpc.ts`: `createClient(XService, transport)`
7. Add hooks in `frontend/src/hooks/useX.ts`
8. Add page component + route in `App.tsx` + nav item in `NavBar.tsx`
9. If service has DB state: model in `src/models/`, repository in same file, migration SQL, repo field on `AppState`

## Project structure (post-refactor)

- **Module structure**: `src/http/` has HTTP handlers (`handlers.rs`) and rate limiter (`rate_limit.rs`). `src/tasks/` has background task spawning (`cluster.rs`, `apply.rs`, `retention.rs`). `main.rs` handles startup orchestration only (~400 lines).
- **Connection pool reuse**: `ClusterAggregator::connect()` is a `pub(crate)` static method that caches gRPC channels in a global `DashMap`. Use it instead of `Endpoint::from_shared().connect()` for inter-node calls.
- **Frontend 401 interceptor**: `frontend/src/lib/http.ts` provides `fetchJson<T>()` and `fetchWithAuth()` with automatic redirect to `/login` on 401. Use for all non-gRPC HTTP calls.

## Release & Versioning

- Default version bump: `0.0.1` unless user specifies otherwise
- Pre-publish: version bump → commit Cargo.toml + Cargo.lock + frontend/package.json → push to CI → wait for CI pass → `cargo publish` → push tag. Publish on a clean tree, don't use `--allow-dirty`.
- Release CI triggers on `v*` tag push, builds 2 Linux musl static targets (x86_64 + aarch64), uploads to GitHub Releases
- **`frontend/dist/` must be committed** — `rust-embed` needs it at compile time. Removed from `.gitignore`; CI builds overwrite the placeholder.
- **`cargo publish` disk space** — verification compiles from scratch (~2.3GB). If disk is near full, `rm -rf target` before publishing.
- **Release workflow artifact packaging** — `upload-artifact@v4` preserves directory structure. Packaging script uses staging dir + `cp` to flatten, then removes artifact dirs before moving.

### Version update checklist

Check existing tags: `git tag -l 'v*' --sort=-v:refname`. Bump from the latest tag (e.g. `v0.1.5` → `v0.1.6`).

1. Update version in `Cargo.toml` and `frontend/package.json` (must match)
2. Run `source "$HOME/.cargo/env" && cargo generate-lockfile` to update `Cargo.lock`
3. Commit all three files: `git add Cargo.toml Cargo.lock frontend/package.json && git commit -m "chore: bump version to X.Y.Z"`
4. Push to master: `git push origin master`
5. Publish to crates.io: `source "$HOME/.cargo/env" && cargo publish`
6. Create and push tag: `git tag vX.Y.Z && git push origin vX.Y.Z` — this triggers the Release workflow (`.github/workflows/release.yml`)
7. Monitor release: `gh run list --limit 3` then `gh run watch <id> --exit-status`
8. Release produces 2 targets: x86_64-unknown-linux-musl, aarch64-unknown-linux-musl (fully static binaries)
