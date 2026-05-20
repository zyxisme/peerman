# Peerman

DN42 对等互联管理工具。通过 Web 界面管理 WireGuard 隧道和 BGP 会话，支持自动生成配置文件。

## 安装

### 前提条件

- [Rust](https://rustup.rs) (1.75+)
- [pnpm](https://pnpm.io) (11.x，用于编译前端)
- 运行环境需要 `ping` 和 `traceroute` 命令（探测功能）

### 编译

```bash
# 克隆仓库
git clone <repo-url> && cd peerman

# 编译（自动构建前端 + 后端，输出单个二进制文件）
cargo build --release

# 如果前端已预先编译好，可跳过前端构建
SKIP_FRONTEND_BUILD=1 cargo build --release
```

编译产物位于 `target/release/peerman`，单个二进制文件包含前端静态资源，无需额外部署。

## 使用

### 1. 创建配置文件

```bash
cp config.toml.example config.toml
```

### 2. 启动服务

```bash
./target/release/peerman -c config.toml
```

浏览器打开 `http://localhost:3000` 即可访问 Web 界面。默认管理员账号为 `admin`，密码在 `config.toml` 的 `[auth]` 配置段中设置。

### 3. 添加 Peer

在 Web 界面中创建 Peer 时需要填写：

| 字段 | 说明 |
|------|------|
| 名称 | 对等点的标识，仅允许字母数字、连字符和下划线 |
| ASN | 对方的 AS 号，必须在 DN42 私有范围 4242420000-4242429999 内 |
| WireGuard 公钥 | 对方的公钥（44 字符 Base64），也可在界面内生成密钥对 |
| 端点地址 | 对方的 IP 地址和端口 |
| 隧道 IP | IPv4/IPv6 隧道地址，用于配置 BGP 邻居 |

### 4. 导出配置

创建 Peer 后，系统会自动生成：
- **WireGuard 配置** — 可直接写入 `/etc/wireguard/` 的 INI 格式
- **BIRD2 配置** — `protocol bgp` 块，可直接嵌入 BIRD 配置文件

单个 Peer 的配置在详情页查看，所有 Peer 的批量导出在 **Export** 页面。

## 配置说明

```toml
[server]
listen_addr = "0.0.0.0:3000"   # 监听地址

[storage]
db_path = "data/peerman.db"    # SQLite 数据库路径

[logging]
level = "info"                  # 日志级别: trace, debug, info, warn, error

[cluster]
node_name = ""                  # 设为非空即启用集群模式
bootstrap_nodes = []            # 引导节点列表，格式 ["10.0.0.1:3000", "10.0.0.2:3000"]
probe_interval_secs = 60        # ICMP 探测间隔（秒），0 表示禁用
sync_interval_secs = 30         # 过期节点下线检查间隔（秒）

[auth]
username = "admin"              # 管理员用户名
password = ""                   # 管理员密码（空则登录始终失败）
jwt_secret = ""                 # JWT 签名密钥（空则启动时自动生成）
```

集群模式用于多机部署，支持节点发现、跨节点延迟探测和 BGP Community 自动匹配。单机使用时无需修改 `[cluster]` 配置。

## 开发

```bash
# 后端（默认读取 ./config.toml）
cargo run -- -c config.toml

# 前端开发服务器（热更新，/api 代理到后端 localhost:3000）
cd frontend && pnpm dev

# 单独构建前端
cd frontend && pnpm run build

# 类型检查
cd frontend && pnpm exec tsc --noEmit
```

## 项目结构

```
src/            # Rust 后端
  grpc/         # gRPC 服务实现
  models/       # 数据模型 + SQLite 仓库
  services/     # WireGuard 密钥生成、BIRD 配置生成、验证、探测等
frontend/       # React 前端 (Vite + TypeScript + Tailwind)
proto/          # Protobuf 服务定义
migrations/     # SQLite 数据库迁移文件
```

## 技术栈

| 层 | 技术 |
|---|------|
| 后端 | Rust, tonic (gRPC), axum, sqlx (SQLite) |
| 前端 | React 18, TypeScript, Vite, Tailwind CSS |
| API | gRPC-Web (tonic-web，无需 envoy 边车) |
| 设计 | Vercel 风格设计系统 (详见 [DESIGN.md](DESIGN.md)) |
