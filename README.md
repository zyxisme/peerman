<div align="center">

# Peerman

[![License](https://img.shields.io/badge/License-MIT-blue?style=for-the-badge&logo=open-source-initiative&logoColor=white)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_edition-%23dea584?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![CI](https://img.shields.io/badge/CI-passing-brightgreen?style=for-the-badge&logo=githubactions&logoColor=white)](https://github.com/zyxisme/peerman/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/peerman.svg?style=for-the-badge&logo=rust&logoColor=white)](https://crates.io/crates/peerman)

**DN42 对等互联管理工具。**

通过 Web 界面管理 WireGuard 隧道和 BGP 会话，**创建 Peer 后自动生成并应用配置**，无需手动操作。

</div>

## 功能

- **WireGuard 管理** — 自动生成 per-peer 配置，`wg syncconf` 无缝更新，支持接口重启
- **BIRD2 管理** — 自动生成完整 bird.conf，`birdc configure` 热重载
- **DN42 标准 BGP Communities** — 基于 AS 64511 标准的 latency/bandwidth/crypto 三级社区标记，自动根据探测结果匹配
- **ROA/RPKI 验证** — 支持静态文件和 RTR 两种模式
- **BFD 快速故障检测** — 可配置 interval/multiplier，加速链路故障发现
- **集群模式** — 多节点自动发现、WG 隧道建立、iBGP full mesh 或 BGP Confederation
- **跨节点 Looking Glass** — 在任意集群节点执行 BIRD 命令和 traceroute
- **BGP 抖动检测** — 基于 iBGP 监听 + BIRD socket 轮询的双通道检测
- **自动探测** — 定期 ICMP 探测节点间延迟和丢包率
- **Web 界面** — React 前端，Vercel 风格设计系统

## 快速开始

```bash
cargo install peerman
peerman -c config.toml
```

浏览器打开 `http://localhost:3000`，默认管理员账号 `admin`，密码在 `config.toml` 中设置。

## 安装

### 一键安装（推荐）

```bash
curl -fsSL https://raw.githubusercontent.com/zyxisme/peerman/master/install.sh | sudo bash
```

交互式脚本，自动完成：依赖检查与安装（wg、bird、ping、traceroute，支持 Debian/Ubuntu/Alpine/Fedora/Arch/openSUSE/Void）→ 下载预编译二进制或源码编译 → 创建系统用户和目录 → 交互式生成配置文件 → 安装 systemd/OpenRC 服务 → 配置 sudoers → 启动并验证。

### 前提条件

- [Rust](https://rustup.rs) (1.75+)
- [pnpm](https://pnpm.io) (11.x，用于编译前端)
- 运行环境需要 `ping` 和 `traceroute` 命令（探测功能）
- `wg-quick`（WireGuard 生命周期管理）
- `birdc`（BIRD2 配置热重载）

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
- 自动建立节点间 WG 隧道（`wg-cluster` 接口），支持 IPv4 + IPv6 双栈
- 自动配置 iBGP full mesh 或 BGP Confederation，使用独立的内部 tunnel IP
- 新节点加入/离开时自动更新配置
- 跨节点 Looking Glass：在任意节点执行 BIRD 命令

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
tunnel_ip_range = ""            # 内部 IPv4 tunnel IP 段（如 10.255.0.0/24），用于节点间 iBGP
tunnel_ipv6_range = ""          # 内部 IPv6 tunnel IP 段（如 fd42:cluster::/48），可选
probe_interval_secs = 60        # ICMP 探测间隔（秒），0 表示禁用
sync_interval_secs = 30         # 过期节点下线检查间隔（秒）
enable_confederation = false    # 启用 BGP Confederation 替代 iBGP full mesh
confederation_local_asn = 0     # Confederation 模式下的私有 ASN

[auth]
username = "admin"              # 管理员用户名
password = ""                   # 管理员密码（空则登录始终失败）
jwt_secret = ""                 # JWT 签名密钥（空则启动时自动生成）
```

集群模式用于多机部署，支持节点发现、跨节点延迟探测和 BGP Community 自动匹配。单机使用时无需修改 `[cluster]` 配置。

## 网络配置功能

### BGP Communities（DN42 标准）

启用 `enable_community_filters` 后，系统自动根据探测结果为每个 peer 生成符合 DN42 AS 64511 标准的社区标记：

- **Latency tier**（1-5）：`<asn>,10`（<5ms）到 `<asn>,50`（>150ms）
- **Bandwidth tier**（1-4）：`<asn>,11`（>1Gbps）到 `<asn>,14`（<50Mbps）
- **Crypto tier**（1-3）：`<asn>,31`（无加密）到 `<asn>,33`（强加密）

生成的 BIRD 配置包含标准的 `dn42_import_filter` 和 `dn42_export_filter` 函数，带 ROA 验证和 MED 惩罚。

### BFD（快速故障检测）

启用 `enable_bfd` 后，在 bird.conf 中生成：

```bird
protocol bfd {
    interface "wg*" {
        interval 300ms;
        multiplier 3;
    };
}
```

### BGP Confederation

启用 `enable_confederation` 后，iBGP full mesh 替换为 confederation 模式：

```bird
protocol bgp node_<name> from dnpeers {
    local as <private_asn>;
    confederation <dn42_asn>;
    confederation member yes;
    neighbor <tunnel_ip> external;
    ...
}
```

### 跨节点 Looking Glass

在 Looking Glass 页面选择目标集群节点，即可远程执行 BIRD 命令（`show protocols`、`show route all` 等）和 traceroute。

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

# 测试
cargo test

# Lint + 格式化
cargo clippy && cargo fmt
```

## 项目结构

```
src/            # Rust 后端
  cluster/      # 集群管理：tunnel（节点间 WG）、aggregator、cache、auth
  grpc/         # gRPC 服务实现（Peer, Settings, Cluster, Bird, Flap, Management）
  models/       # 数据模型 + SQLite 仓库（peer, node, settings, probe, community, flap）
  services/     # WireGuard/BIRD 配置生成与系统命令执行、验证、探测、社区映射
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

## gRPC 服务一览

| 服务 | 主要 RPC |
|------|----------|
| PeerService | CreatePeer, UpdatePeer, DeletePeer, TogglePeer, GenerateKeypair, RestartWireGuard, ExportAllWireGuard, ExportAllBird |
| SettingsService | GetSettings, SaveSettings |
| ClusterService | ListNodes, ExchangeNodes, HealthCheck, RunProbe, ListCommunityRules, SaveCommunityRule, GetPeerCommunities |
| BirdService | ExecuteCommand, RunTraceroute |
| ManagementService | GetWireGuardStatus, GetBirdStatus |
| FlapService | ListFlapEvents, GetFlapStats |

## 开源协议

MIT
