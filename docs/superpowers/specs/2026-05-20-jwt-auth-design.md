# JWT Authentication — Design Spec

**Date**: 2026-05-20
**Status**: draft

## Context

Peerman currently has zero authentication — all gRPC APIs and frontend pages are open. We need to add login protection for write operations while keeping read/view pages public. The target is a simple single-admin setup with credentials in the config file.

**Constraints:**
- Single admin user (no multi-user, no roles)
- Write/sensitive operations require login; read/view pages remain public
- JWT stored in httpOnly cookie (not localStorage)
- 30-day token expiry, no refresh mechanism
- Credentials in config.toml

## Architecture Overview

```
config.toml [auth]            Frontend
  username                    LoginPage → POST /api/auth/login → Set-Cookie (jwt)
  password              →     AuthContext (global state: isAuthenticated, username)
  jwt_secret                  ProtectedRoute wrapper on write-page routes
                              NavBar login/logout button

Backend
  POST /api/auth/login   (axum HTTP, validate credentials, issue JWT, set cookie)
  POST /api/auth/logout  (axum HTTP, clear cookie)
  GET  /api/auth/me      (axum HTTP, verify JWT, return username)
  tonic interceptor      (validate JWT from cookie on write gRPC methods)
```

## Backend Design

### 1. Config (`src/config.rs`)

New `AuthConfig` struct + `[auth]` TOML section:

```toml
[auth]
username = "admin"
password = "your-password-here"
jwt_secret = ""  # empty = auto-generate on startup
```

- `jwt_secret`: when empty, generate a random 64-char hex string at startup and log it. All tokens invalidate on restart.
- `password`: plaintext in config file (single-admin tool; config file should be protected by filesystem permissions).

### 2. Dependencies (`Cargo.toml`)

- `jsonwebtoken` — JWT signing and verification (Rust)
- `rand` — random secret generation (may already be a transitive dep)

### 3. Auth module (`src/auth.rs`)

New file containing:

- `AuthConfig` — moved from config.rs (or re-exported)
- `generate_jwt_secret() -> String` — 64-char random hex
- `create_token(username: &str, secret: &str, ttl_days: i64) -> Result<String>` — issue JWT
- `verify_token(token: &str, secret: &str) -> Result<Claims>` — verify and decode
- `Claims { sub: String, exp: usize, iat: usize }` — JWT payload
- `PUBLIC_METHODS: &[&str]` — list of gRPC methods that skip auth
- `auth_interceptor(req: Request<BoxBody>) -> Result<Request<BoxBody>, Status>` — tonic interceptor fn

**Interceptor logic:**
1. Extract method name from request URI (format: `/peerman.PeerService/CreatePeer`)
2. If method is in `PUBLIC_METHODS`, pass through
3. Otherwise, extract `Cookie` header → parse JWT → verify
4. On failure: return `Status::unauthenticated("...")`

**Public method whitelist:**
```
ListPeers, GetPeer, GenerateKeypair, GenerateWireGuardConfig, GenerateBirdConfig,
GetSettings,
ListNodes, GetNode, ListProbeResults, ListCommunityRules,
ListFlapEvents, GetFlapStats
```

Everything else (Create/Update/Delete/Save/Push/Pull/Execute/Run) requires auth.

### 4. HTTP auth endpoints (`main.rs`)

Registered as axum routes (not gRPC):

**`POST /api/auth/login`**
- Request: `{ "username": "str", "password": "str" }`
- Check against config, issue JWT on match
- Response: `Set-Cookie` header with httpOnly cookie + `{ "success": true, "user": { "username": "admin" } }`
- Cookie: `jwt=<token>; HttpOnly; SameSite=Strict; Path=/; Max-Age=2592000`
- On failure: `401 { "success": false, "error": "Invalid credentials" }`

**`POST /api/auth/logout`**
- Set `jwt` cookie with `Max-Age=0` to clear
- Response: `{ "success": true }`

**`GET /api/auth/me`**
- Read JWT from cookie, verify, return username
- Response: `{ "authenticated": true, "username": "admin" }` or `{ "authenticated": false }`

### 5. Server wiring (`main.rs`)

```rust
let auth_layer = AuthInterceptorLayer::new(auth_config.jwt_secret.clone());
// or: .interceptor(auth_interceptor) on the tonic builder

let grpc_router = tonic::transport::Server::builder()
    .accept_http1(true)
    .layer(tonic_web::GrpcWebLayer::new())
    .interceptor(auth_interceptor)
    .add_service(...)
    .into_router();

let app = Router::new()
    .route("/api/auth/login", post(handle_login))
    .route("/api/auth/logout", post(handle_logout))
    .route("/api/auth/me", get(handle_me))
    .nest("/api", grpc_router)
    .fallback(static_files::serve_static)
    .layer(TraceLayer::new_for_http());
```

**Cookie parsing in interceptor:** tonic interceptor receives raw HTTP headers. The `Cookie` header needs to be parsed client-side. Since we use `tonic-web` layer, the browser sends cookies in the `Cookie` header of the underlying HTTP request. The interceptor reads this from request metadata.

## Frontend Design

### 1. Auth context (`frontend/src/lib/auth.tsx`)

React context providing:

```typescript
interface AuthState {
  isAuthenticated: boolean;
  username: string | null;
  loading: boolean;  // true while checking /api/auth/me on mount
}

interface AuthActions {
  login(username: string, password: string): Promise<void>;
  logout(): Promise<void>;
}
```

- On mount: `GET /api/auth/me` → update state (restores session after page refresh)
- `login()`: `POST /api/auth/login` → parse JSON → update state
- `logout()`: `POST /api/auth/logout` → clear state

Provider wraps the app in `main.tsx`:
```tsx
<BrowserRouter>
  <AuthProvider>
    <App />
  </AuthProvider>
</BrowserRouter>
```

### 2. Login page (`frontend/src/components/auth/LoginPage.tsx`)

- Username + password form
- On success: navigate to `?redirect=<path>` or `/`
- Error: show inline error message
- Styled with existing card/form design tokens

### 3. Protected route (in `auth.tsx`)

Wrapper component:
```tsx
function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, loading } = useAuth();
  if (loading) return null; // or spinner
  if (!isAuthenticated) return <Navigate to={`/login?redirect=${location.pathname}`} />;
  return <>{children}</>;
}
```

### 4. Route changes (`App.tsx`)

Wrap write pages with ProtectedRoute:
```tsx
<Route path="/peers/new" element={<ProtectedRoute><PeerForm /></ProtectedRoute>} />
<Route path="/peers/:id/edit" element={<ProtectedRoute><PeerForm /></ProtectedRoute>} />
<Route path="/settings" element={<ProtectedRoute><SettingsPage /></ProtectedRoute>} />
<Route path="/nodes/new" element={<ProtectedRoute><NodeForm /></ProtectedRoute>} />
<Route path="/nodes/:id/edit" element={<ProtectedRoute><NodeForm /></ProtectedRoute>} />
```

Read-only routes remain unwrapped. Login page route: `<Route path="/login" element={<LoginPage />} />`.

### 5. NavBar changes

- Right side: if `isAuthenticated` → show username + "Logout" button
- If not authenticated → show "Login" link
- Use existing link/button styles

## Files Changed

| File | Change |
|------|--------|
| `config.toml.example` | Add `[auth]` section |
| `src/config.rs` | Add `AuthConfig` struct |
| `src/auth.rs` | **New** — JWT utils, interceptor, public method list |
| `src/main.rs` | Add auth HTTP endpoints, wire interceptor |
| `Cargo.toml` | Add `jsonwebtoken` dep |
| `frontend/src/main.tsx` | Wrap app with `AuthProvider` |
| `frontend/src/App.tsx` | Add routes, wrap with ProtectedRoute |
| `frontend/src/lib/auth.tsx` | **New** — AuthContext, ProtectedRoute, useAuth |
| `frontend/src/components/auth/LoginPage.tsx` | **New** — Login form |
| `frontend/src/components/layout/NavBar.tsx` | Add login/logout state |

## Verification

1. **Config parsing**: `cargo run -- -c config.toml.example` starts without error, random secret logged when `jwt_secret` is empty
2. **Login flow**: `curl -X POST localhost:3000/api/auth/login -H 'Content-Type: application/json' -d '{"username":"admin","password":"test"}'` → returns 200 with Set-Cookie header
3. **Auth rejection**: `curl -X POST localhost:3000/api/peerman.PeerService/CreatePeer` without cookie → returns `401 Unauthenticated`
4. **Public pass-through**: `curl localhost:3000/api/peerman.PeerService/ListPeers` without cookie → returns 200
5. **Frontend integration**: `cd frontend && pnpm dev` → login page at /login, logged-out visit to /peers/new redirects to /login, login succeeds and redirects back
6. **Unit tests**: `cargo test` — JWT create/verify roundtrip, interceptor public/write method routing
7. **Type check**: `cd frontend && pnpm exec tsc --noEmit`
