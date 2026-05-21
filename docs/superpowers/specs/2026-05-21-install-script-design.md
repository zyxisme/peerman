# Peerman 交互式安装脚本设计

## 目标

编写一个自包含的交互式 bash 安装脚本，支持 Peerman 的一键部署：
依赖检查 → 选择安装方式 → 安装二进制 → 交互式配置 → 设置守护进程 → 启动验证。

## 脚本架构

单文件 bash 脚本 `install.sh`，内嵌所有模板（systemd unit、OpenRC init、sudoers 配置）通过 heredoc 生成。

### 执行流程

1. **权限检查** — 必须以 root 运行（需要写 `/etc/`、创建用户、安装 service）
2. **依赖检查** — 检测并报告可用性（不阻断，仅警告）：
   - systemd 或 OpenRC
   - wg（WireGuard 命令行工具）
   - birdc（BIRD 控制命令）
   - ping / traceroute（探测功能）
   - curl / git（下载或编译方式对应的依赖）
   - Rust / cargo / pnpm（源码编译方式）
3. **安装方式选择** — 用户选择：
   - 从 GitHub Releases 下载预编译二进制
   - 从本地源码编译（`cargo build --release`）
4. **创建系统用户** — 创建 `peerman` 用户（无登录 shell），无 home 目录
5. **安装二进制** — 下载/编译后放入 `/usr/local/bin/peerman`，设置 owner `peerman:peerman`
6. **创建目录结构**：
   - `/etc/peerman/` — 配置目录，owner `root:peerman`，mode `0750`
   - `/var/lib/peerman/` — 数据目录，owner `peerman:peerman`，mode `0750`
7. **交互式配置生成** — 逐项引导用户填写 `config.toml`（见下文）
8. **守护进程安装** — 自动检测 init 系统，安装 systemd unit 或 OpenRC init 脚本
9. **sudoers 配置** — 写入 `/etc/sudoers.d/peerman`，精确允许必要命令
10. **启动服务** — `systemctl enable --now peerman` 或 `rc-update add peerman && rc-service peerman start`
11. **验证** — 等待 2 秒，检查进程是否运行，端口是否监听，打印访问地址

### 安全设计：权限最小化

创建专用系统用户 `peerman`，配置 sudoers 精确允许以下命令无密码执行：
- `/usr/bin/wg` — WireGuard 配置管理
- `/usr/sbin/birdc` — BIRD 控制
- `/usr/bin/ping` — ICMP 探测
- `/usr/sbin/traceroute` — 路由追踪

二进制文件权限：`peerman:peerman`，sudoers 配置允许 peerman 用户执行上述特定命令。

## 交互式配置流程

每项显示默认值，用户回车接受默认（或输入新值）。密码项不回显输入。

| 步骤 | 配置项 | 默认值 | 触发条件 |
|------|--------|--------|----------|
| 1 | `server.listen_addr` | `0.0.0.0:3000` | 始终 |
| 2 | `storage.db_path` | `/var/lib/peerman/peerman.db` | 始终 |
| 3 | `logging.level` | `info` | 始终 |
| 4 | `auth.username` | `admin` | 始终 |
| 5 | `auth.password` | 无默认，必填，确认输入两次 | 始终 |
| 6 | `cluster.node_name` | 空（跳过集群） | 始终 |
| 7 | `cluster.cluster_key` | 空时自动生成 32 位 hex | node_name 非空时 |
| 8 | `cluster.peer_nodes` | 空（逗号分隔的 host:port） | node_name 非空时 |
| 9 | `cluster.tunnel_ip_range` | `10.255.0.0/24` | node_name 非空时 |

如果用户在第 6 步留空，跳过 7-9，`[cluster]` 段写入注释掉的默认值。

## 守护进程模板

### systemd unit (`/etc/systemd/system/peerman.service`)

```ini
[Unit]
Description=Peerman - DN42 Peer Manager
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=peerman
Group=peerman
ExecStart=/usr/local/bin/peerman -c /etc/peerman/config.toml
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

### OpenRC init (`/etc/init.d/peerman`)

```sh
#!/sbin/openrc-run
name="peerman"
command="/usr/local/bin/peerman"
command_args="-c /etc/peerman/config.toml"
command_user="peerman"
command_background=false
pidfile="/run/${RC_SVCNAME}.pid"

depend() {
    need net
    after firewall
}
```

## 错误处理

- 所有命令失败时打印错误并退出（`set -euo pipefail`）
- 下载失败时提示检查网络或手动下载
- 编译失败时打印日志路径
- 配置写入失败时回滚（删除已写的文件）
- 服务启动失败时打印 `journalctl` 或 `tail /var/log/messages` 的命令提示
- 所有模板写入使用 `.tmp` → `mv` 原子操作

## 兼容性

- shell：bash 4.0+（`set -euo pipefail`，`${var,,}` 小写转换）
- init 系统：systemd（主流 Linux）和 OpenRC（Alpine、Gentoo）
- 架构：amd64 / arm64（GitHub Releases 方式）；源码编译支持所有 Rust 支持的架构
- 操作系统：Linux（脚本首行检查 `uname -s`）

## 非目标

- 不支持 macOS / FreeBSD（Peerman 依赖 Linux 特定的 WG + BIRD 接口）
- 不处理远程批量部署（Ansible/Puppet 等），本脚本仅用于单机交互式安装
- 不执行数据库迁移/升级（首次安装，升级脚本另做）
- 不做 Docker/容器化部署
