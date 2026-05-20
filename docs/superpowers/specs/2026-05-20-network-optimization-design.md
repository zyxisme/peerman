# Network Configuration Optimization

> 学习 Lantian Blog + DN42 Wiki 最佳实践，优化 WireGuard、BIRD BGP、Community MED 配置生成

## Scope

- **WireGuard** 配置生成：Table=off、MTU、fwmark、PostUp/Down
- **BIRD BGP** 配置生成：ROA 过滤、prefix limit、is_valid_network()、import table
- **Community MED** 增强：latency + bandwidth + crypto 三维度
- **Settings** 扩展：ROA、WG 高级选项、BIRD filter 模板
- **前端**：Settings 表单、Community Rules 表单同步更新

## 1. Database — Migration 005

### settings table new columns

| Column | Type | Default | Description |
|--------|------|---------|-------------|
| `wg_mtu` | INTEGER | 1420 | WireGuard MTU |
| `wg_fwmark` | INTEGER | 0 | fwmark (0 = disabled) |
| `wg_post_up` | TEXT | '' | Custom PostUp script |
| `wg_post_down` | TEXT | '' | Custom PostDown script |
| `roa_mode` | TEXT | 'none' | `none` / `static_file` / `rtr` |
| `roa_static_v4_url` | TEXT | '' | Static ROA v4 file URL |
| `roa_static_v6_url` | TEXT | '' | Static ROA v6 file URL |
| `roa_rtr_address` | TEXT | '' | RTR server address |
| `roa_rtr_port` | INTEGER | 323 | RTR port |
| `bird_import_limit` | INTEGER | 9000 | BGP import prefix limit |
| `bird_export_filter` | TEXT | '' | Custom export filter body |
| `bird_import_filter` | TEXT | '' | Custom import filter body |

### community_rules table new columns

| Column | Type | Default | Description |
|--------|------|---------|-------------|
| `min_bandwidth_mbps` | REAL | 0 | Minimum bandwidth (0 = unlimited) |
| `crypto_weight` | INTEGER | 0 | Crypto overhead weight (0 = unlimited) |
| `med_penalty` | INTEGER | 0 | MED penalty value |

Default seed rules updated to include bandwidth/crypto/MED dimensions.

## 2. Proto — `peerman.proto`

### Settings message (new fields 8-19)

```protobuf
uint32 wg_mtu = 8;
uint32 wg_fwmark = 9;
string wg_post_up = 10;
string wg_post_down = 11;
string roa_mode = 12;
string roa_static_v4_url = 13;
string roa_static_v6_url = 14;
string roa_rtr_address = 15;
uint32 roa_rtr_port = 16;
uint32 bird_import_limit = 17;
string bird_export_filter = 18;
string bird_import_filter = 19;
```

### CommunityRule message (new fields 8-10)

```protobuf
double min_bandwidth_mbps = 8;
int32 crypto_weight = 9;
int32 med_penalty = 10;
```

### Regeneration

```
protoc -I proto --es_out frontend/src/lib --es_opt target=ts proto/peerman.proto
```

## 3. Backend — Config Generation

### wireguard.rs

`generate_config()` output changes:

- `Table = off` (per Lantian recommendation)
- `MTU = <settings.wg_mtu>` (only if > 0)
- `FwMark = <settings.wg_fwmark>` (only if > 0)
- Auto-generated PostUp: add tunnel IPs, disable IPv6 autoconf
- Append `settings.wg_post_up` / `settings.wg_post_down` if non-empty
- `AllowedIPs` unchanged: full DN42 ranges + fe80::/10
- `PersistentKeepalive = 25` unchanged

### bird.rs

`generate_full_config()` output rewritten to DN42 best practices:

```bird
router id ...;
define OWNAS = ...;
define OWNNETSET = [...];

# ROA tables — mode-dependent:
# none: empty roa table
# static_file: include "/etc/bird/roa_dn42_v4.conf"; include "/etc/bird/roa_dn42_v6.conf";
# rtr: protocol rpki roa_dn42 { roa4/roa6 tables, remote, port, refresh, expire }

# Filter functions with -> bool return types
function is_valid_network() -> bool {
  return net ~ [
    172.20.0.0/14{21,29},     # dn42
    172.20.0.0/24{28,32},     # dn42 Anycast
    172.21.0.0/24{28,32},     # dn42 Anycast
    172.22.0.0/24{28,32},     # dn42 Anycast
    172.23.0.0/24{28,32},     # dn42 Anycast
    172.31.0.0/16+,           # ChaosVPN
    10.100.0.0/14+,           # ChaosVPN
    10.127.0.0/16+,           # neonetwork
    10.0.0.0/8{15,24}         # Freifunk.net
  ];
}

function is_valid_network_v6() -> bool {
  return net ~ [ fd00::/8{44,64} ];
}

function is_self_net() -> bool { return net ~ OWNNETSET; }

template bgp dnpeers {
    local as OWNAS;
    path metric 1;

    ipv4 {
        import filter {
          if is_valid_network() && !is_self_net() then {
            if (roa_check(dn42_roa, net, bgp_path.last) != ROA_VALID) then {
              print "[dn42] ROA check failed for ", net, " ASN ", bgp_path.last;
              reject;
            } else accept;
          } else reject;
        };
        export filter { ... settings.bird_export_filter or default ... };
        import limit <settings.bird_import_limit> action block;
        import table;
    };

    ipv6 { /* mirror */ };
}
```

`generate_peer_block()` unchanged — already correct.

### community_mapper.rs

`compute_communities()` enhanced to 3-dimension matching:

- latency: `probe.avg_latency_ms <= rule.max_latency_ms`
- bandwidth: `probe.bandwidth_mbps >= rule.min_bandwidth_mbps` (0 = skip)
- crypto: peer WireGuard overhead matches `rule.crypto_weight`

New `compute_med()` function:

```
med = base_med + latency_bucket * 300 + bandwidth_penalty + crypto_penalty
```

Used in export filter generation for MED-based path selection.

### settings model

`src/models/settings.rs`: add new fields to Settings struct, repository load/save queries.

## 4. Frontend

### Settings page

Three new sections using `card-soft` grouping:

- **WireGuard advanced**: MTU (number), FwMark (number), PostUp/PostDown (textarea, code style)
- **ROA/RPKI**: `tab-ghost` 3-way toggle (None / Static File / RTR), conditional URL/address fields
- **BIRD filter**: Import Limit (number), Export/Import filter (textarea, optional advanced)

### Community Rules page

Add 3 fields to rule edit form:
- Min Bandwidth Mbps (number)
- Crypto Weight (number)
- MED Penalty (number)

Form uses 3-column grid layout, `template-card` style.

### Design system compliance

All new UI uses DESIGN.md tokens: `form-input`, `card-soft`, `tab-ghost`, `badge-secondary`, `caption`, `hairline` border, 6px border-radius. Geist/Inter fonts, Vercel spacing scale.

## 5. Implementation Order

1. Migration `005_network_optimization.sql`
2. Proto: Settings + CommunityRule new fields, regenerate Rust + TS stubs
3. Rust model: `Settings` struct + repository
4. Rust model: `CommunityRule` struct + repository + seed defaults
5. Config generation: `wireguard.rs` generate_config() rewrite
6. Config generation: `bird.rs` generate_full_config() rewrite
7. `community_mapper.rs`: 3-dimension compute + MED function
8. Settings gRPC service: wire new settings fields
9. Community gRPC service: wire new community fields
10. Frontend: Settings page form
11. Frontend: Community Rules page form
12. `cargo build` + `cargo test` + `cargo clippy` + `tsc --noEmit`
