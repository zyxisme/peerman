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
