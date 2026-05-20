# Peerman — DN42 Peer Manager

Rust backend (tonic + axum + sqlx) + React frontend (Vite + TypeScript + shadcn/ui), gRPC-Web API, single binary.

## Build

- `cargo build` — full build (proto gen → frontend pnpm build → rust compile → embed dist/)
- `SKIP_FRONTEND_BUILD=1 cargo build` — skip frontend, use pre-built dist/
- `cd frontend && pnpm dev` — Vite dev server (proxies /api to localhost:3000)
- `cargo run -- --db-path /tmp/peerman.db` — start server
- `cd frontend && pnpm run build` — build frontend only

## Key crate/version constraints

- `tonic 0.12` depends on `axum 0.7` — do NOT upgrade axum past 0.7
- `x25519-dalek 2.x` requires `features = ["static_secrets"]` for StaticSecret
- `sqlx` with `sqlite` feature = bundled SQLite (compiled from C, no system lib needed)
- Frontend uses pnpm (not npm), `packageManager: "pnpm@11.1.3"` in package.json

## sqlx — use runtime API, not macros

sqlx macros (`query_as!`, `query!`) need DATABASE_URL at compile time. Use runtime versions instead:
- `sqlx::query_as::<_, T>(sql)` + `.bind()` + `.fetch_all(&pool)`
- `sqlx::query(sql)` + `.bind()` + `.execute(&pool)`
- Models derive `sqlx::FromRow` for `query_as` to work

## Proto & gRPC

- `tonic::include_proto!("peerman")` puts generated types directly in the calling module (no `peerman` submodule wrapper)
- `build.rs` runs `tonic_build::compile_protos("proto/peerman.proto")` → `$OUT_DIR/peerman.rs`
- Frontend proto: `protoc -I proto --es_out frontend/src/lib --es_opt target=ts peerman.proto` with `protoc-gen-es` in PATH (`frontend/node_modules/.bin`)
- `@connectrpc/connect v2`: use `createClient()` (not `createPromiseClient`), messages via `create(Schema, {...})`

## SQLite WAL

PRAGMA journal_mode=WAL must run OUTSIDE a transaction. Set it before `sqlx::migrate!()`, not inside the migration SQL.

## DESIGN.md

Vercel-inspired design system. Tokens (colors, typography, spacing, border-radius, shadows) mapped to `tailwind.config.ts`. shadcn/ui components styled via `globals.css` utility classes. Geist/Inter fonts loaded from Google Fonts CDN.

## Cluster mode

- `--node-name <name>` enables cluster mode (self-registers in `nodes` table). Without it, node/probe/community features are dormant.
- `--cluster-nodes <host:port,...>` comma-separated bootstrap peers (added to `nodes` on startup).
- `--probe-interval-secs <N>` (default 60) background ICMP probe interval. `--sync-interval-secs <N>` (default 30) stale-node check interval.
- `migrations/002_cluster.sql` adds `nodes`, `probe_results`, `community_rules` tables + `origin_node_id` on `peers`.

## Inter-node communication (gRPC client)

- `build.rs` uses `build_client(true)` to generate client stubs alongside server stubs.
- `tonic::Request` does NOT implement Clone — reconstruct per destination node.
- `tonic_web::GrpcWebClientLayer` wrapping a tonic Channel has complex type bounds. For now, cross-node sync uses utility functions (`apply_proto_to_model`, `probe_result_to_proto`) — live gRPC client push/pull needs further type work.
- `into_router()` is deprecated in tonic 0.12; the suggested `into_axum_router()` may not exist. Keep using `into_router()` (warnings are harmless).

## Frontend gotchas

- Proto `int64` fields become TypeScript `bigint` — cannot render directly in React. Use `String(v)` or `.toString()`.
- After proto changes, regenerate TS stubs: `PATH="frontend/node_modules/.bin:$PATH" protoc -I proto --es_out frontend/src/lib --es_opt target=ts proto/peerman.proto`
- `pnpm exec tsc --noEmit` for fast type-check without full build.

## Timestamps in SQLite

- All timestamps use RFC3339 UTC (`chrono::Utc::now().to_rfc3339()`). String comparison works because UTC offsets are consistent.
- Use `chrono::Duration::seconds(n)` / `Duration::days(n)` for threshold arithmetic, NOT raw Unix timestamp strings.

## Community & probe

- `CommunityRuleRepository::seed_defaults()` auto-populates 5 latency-tier rules on first run (empty table).
- `services::probe::ping()` calls `ping -c 5 -i 0.2 <target>` subprocess, parses rtt/packet-loss with regex.
- Probe results stored locally only; cross-node push via `PushProbeResult` RPC is defined but client-side not yet wired.
