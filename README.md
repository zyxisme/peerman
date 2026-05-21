# Peerman

DN42 对等互联管理工具。通过 Web 界面管理 WireGuard 隧道和 BGP 会话，**创建 Peer 后自动生成并应用配置**，无需手动操作。

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

### 4. 自动应用配置

创建/修改/删除 Peer 后，系统**自动**将配置写入系统并热重载：

- **WireGuard** → 写入 `/etc/wireguard/wg0.conf` → `wg syncconf wg0`（无缝更新，无需 down/up）
- **BIRD2** → 写入 `/etc/bird/bird.conf` → `birdc configure`（热重载，BGP 会话不中断）

在 **Export** 页面预览生成的配置文本，在 **Status** 页面查看 WireGuard 接口和 BIRD 协议的实时状态。

### 5. 集群模式（多节点）

配置 `[cluster]` 段后，Peerman 自动管理节点间互联：

- 自动生成 WG 密钥对，通过 gossip 交换公钥
- 自动建立节点间 WG 隧道（`wg-cluster` 接口）
- 自动配置 iBGP full mesh，使用独立的内部 tunnel IP
- 新节点加入/离开时自动更新配置

需要 Peerman 进程具有 root 权限（写系统配置文件和执行 `wg`/`birdc` 命令）。

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
cluster_key = ""                # 集群共享密钥（用于 inter-node gRPC 认证）
peer_nodes = []                 # 引导节点列表，格式 ["10.0.0.1:3000", "10.0.0.2:3000"]
tunnel_ip_range = ""            # 内部 tunnel IP 段（如 10.255.0.0/24），用于节点间 iBGP
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
  cluster/      # 集群管理：tunnel（节点间 WG）、aggregator、cache、auth
  grpc/         # gRPC 服务实现（Peer, Settings, Cluster, Bird, Flap, Management）
  models/       # 数据模型 + SQLite 仓库（peer, node, settings, probe, community, flap）
  services/     # WireGuard/BIRD 配置生成与系统命令执行、验证、探测
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
