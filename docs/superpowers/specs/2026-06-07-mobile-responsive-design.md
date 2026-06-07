# Peerman 移动端适配 + 前后端断层修复设计

日期：2026-06-07

## 概述

Peerman 前端目前基本没有移动端适配，NavBar 溢出、数据表格不可用、表单布局未考虑小屏幕。同时存在前后端断层：Settings 表单缺少 3 个 proto 字段、`useNode` hook 效率低下、`RestartWireGuard` RPC 无 UI 入口、CSS 引用不存在的 token。

本设计覆盖全部问题：移动端响应式适配（Radix UI + Tailwind）、CSS 语义化 token 修复、共享组件提取、前后端断层修复。

## 技术方案

**方案：Radix UI + Tailwind**

- NavBar 移动端使用 `@radix-ui/react-dialog` 实现侧边抽屉，内置无障碍支持（focus trap、ESC、aria）
- 表格使用 CSS `display: block` + `data-label` 属性实现卡片布局，无 JS 条件渲染
- 共享组件提取到 `components/ui/`
- 零其他新依赖（仅 `@radix-ui/react-dialog`）

## 1. NavBar 响应式

### 现状

- 6-10 个链接水平排列在 64px 高的 sticky nav bar 中
- 无折叠、无汉堡菜单、无溢出处理
- 移动端链接溢出或换行，完全不可用

### 方案

- `md:`（768px）以上：保持现有水平布局不变
- `md:` 以下：
  - 隐藏所有导航链接
  - 显示汉堡图标（☰），位于 nav bar 右侧
  - 点击汉堡 → 打开 Radix `<Dialog>` 侧边抽屉（280px 宽，左侧滑入）
  - 抽屉内：Logo + 关闭按钮、导航链接（带 lucide 图标，垂直排列）、auth 链接分隔线、用户信息 + Logout
  - 自带 focus trap、ESC 关闭、点击遮罩关闭
  - 遮罩：半透明黑色（`rgba(0,0,0,0.4)`）覆盖页面内容

### 受影响文件

- `frontend/src/components/layout/NavBar.tsx` — 主要改动
- `frontend/src/styles/globals.css` — 新增抽屉过渡动画
- `frontend/package.json` — 新增 `@radix-ui/react-dialog`

## 2. 数据表格卡片布局

### 现状

- PeerTable 8 列、CommunityRules 9 列、NodesTable、FlapDashboard 表格
- 无横向滚动、无列隐藏、无响应式重排
- 移动端完全不可用

### 方案

创建 `<ResponsiveTable>` 通用包装组件：

- **Desktop**（`md:` 以上）：保持现有 `.data-table` 样式，标准行列布局
- **Mobile**（`md:` 以下）：
  - 表头隐藏（`display: none`）
  - 每个 `<tr>` 转为独立卡片（`display: block`，带 border + border-radius）
  - 每个 `<td>` 转为 label-value 行（`display: flex; justify-content: space-between`）
  - label 通过 `data-label` 属性获取（从表头文本映射）
  - 卡片顶部：主字段（Name）+ 状态 Badge
  - 卡片底部：操作链接

### 实现方式

纯 CSS + data 属性，不需要 JS 条件渲染：

```tsx
<ResponsiveTable>
  <thead>
    <tr>
      <th>Name</th>
      <th>ASN</th>
      ...
    </tr>
  </thead>
  <tbody>
    <tr>
      <td data-label="Name">peer-us-east</td>
      <td data-label="ASN">4242420101</td>
      ...
    </tr>
  </tbody>
</ResponsiveTable>
```

### 受影响组件

每个表格组件需要在 `<td>` 上手动添加 `data-label` 属性（值与对应 `<th>` 文本一致），以驱动移动端卡片布局的 label 显示。

- `PeerTable.tsx` — 8 列，主字段 Name + Status 作为卡片头部
- `NodesTable.tsx` — 主字段 Name
- `CommunityRules.tsx` — 主字段 Community
- `FlapDashboard.tsx`（flap events 表格）— 主字段 Peer
- `ProbeDashboard.tsx`（probe results 表格）— 主字段 Source

### 新增文件

- `frontend/src/components/ui/ResponsiveTable.tsx`

## 3. 统计卡片响应式

### 现状

- FlapDashboard 使用 `grid-cols-3`，无响应式变体
- 移动端三个卡片被挤压

### 方案

- `grid-cols-2 sm:grid-cols-3`（小屏幕两列，最后一项 `col-span-2`，640px+ 三列）

### 受影响组件

- `FlapDashboard.tsx`（stats cards）
- `ProbeDashboard.tsx`（如有类似网格）

## 4. 其他移动端适配

### ProbeDashboard 延迟矩阵

- 外层 card 加 `overflow-x-auto`，允许横向滚动

### LookingGlass 控件

- select + input + button 的 flex row 加 `flex-wrap`
- 移动端每个控件变为全宽（`w-full`）

### 表单布局

- 现有 `grid-cols-1 md:grid-cols-2` 模式已正确，无需修改
- 移动端输入框 padding 适当增大（`py-2.5` → `py-3`）以改善触控体验

## 5. CSS Token 语义化

### 新增语义色到 tailwind.config.ts

```js
colors: {
  success: '#0070f3',
  'success-bg': 'rgba(0, 112, 243, 0.06)',
  warning: '#f5a623',
  'warning-bg': 'rgba(245, 166, 35, 0.06)',
  error: '#ee0000',
  'error-bg': 'rgba(238, 0, 0, 0.06)',
}
```

### 修复不存在的类

| 文件 | 问题 | 修复 |
|------|------|------|
| `SettingsForm.tsx` L290 | `bg-surface-3` 不存在 | → `bg-hairline` |
| `ErrorBoundary.tsx` L21 | `bg-bg` 不存在 | → `bg-canvas` |

### 替换硬编码颜色

| 文件 | 硬编码 | 替换为 |
|------|--------|--------|
| `NavBar.tsx` | `bg-green-500` | `bg-success` |
| `NavBar.tsx` | `bg-yellow-500` | `bg-warning` |
| `NavBar.tsx` | `bg-red-500` | `bg-error` |
| `StatusPage.tsx` | `bg-green-500/20 text-green-500` | `bg-success-bg text-success` |
| `StatusPage.tsx` | `bg-red-500/20 text-red-500` | `bg-error-bg text-error` |
| `StatusPage.tsx` | `bg-yellow-500/20 text-yellow-500` | `bg-warning-bg text-warning` |

## 6. 共享组件提取

### 从 PeerForm.tsx 和 SettingsForm.tsx 提取

| 组件 | 说明 | 提取到 |
|------|------|--------|
| `Input` | label + input + error 显示 | `components/ui/Input.tsx` |
| `Textarea` | label + textarea + error | `components/ui/Textarea.tsx` |
| `Toggle` | 开关切换，label + description | `components/ui/Toggle.tsx` |

两个表单 import 同一组件，消除实现差异。Toggle 组件接口统一为：

```tsx
interface ToggleProps {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}
```

## 7. 前后端断层修复

### A. Settings 表单补全 3 个字段

在 `SettingsForm.tsx` 的 Cluster 字段组中添加：

| 字段 | 类型 | 说明 |
|------|------|------|
| `cluster_tunnel_ipv6_range` | 文本输入 | IPv6 隧道地址池（如 `fd42:cluster::/48`） |
| `enable_confederation` | Toggle | BGP 联邦开关 |
| `confederation_local_asn` | 数字输入 | 联邦本地 ASN（仅在 `enable_confederation` 开启时显示） |

### B. GetNode RPC

**Proto 变更**（`proto/peerman.proto`）：

```protobuf
// 在 ClusterService 中添加
rpc GetNode(GetNodeRequest) returns (Node);

message GetNodeRequest {
  string id = 1;
}
```

**后端实现**（`src/grpc/cluster_service.rs`）：

在 `ClusterServiceImpl` 中添加 `get_node` 方法，从 `NodeRepository` 按 ID 查询。

**前端变更**（`frontend/src/hooks/useNodes.ts`）：

`useNode` hook 改为调用 `clusterClient.getNode({ id })` 而非 `listNodes` 再过滤。

**Proto 重新生成**：

```bash
PATH="frontend/node_modules/.bin:$PATH" protoc -I proto --es_out frontend/src/lib --es_opt target=ts proto/peerman.proto
```

### C. RestartWireGuard UI

在 `StatusPage.tsx` 中添加 "Restart WireGuard" 按钮：

- 位置：WireGuard 状态区域的操作栏
- 点击 → 确认对话框（"Are you sure? This will briefly disconnect all WG peers."）
- 确认 → 调用 `peerClient.restartWireGuard({})`
- 执行中：按钮显示 loading spinner，disabled
- 完成：toast 提示成功/失败

### D. 现有 useNode hook 效率问题

当前实现（`useNodes.ts` L40）：
```ts
// getNode doesn't exist as an RPC -- find from list
const nodes = await clusterClient.listNodes({});
const node = nodes.nodes.find(n => n.id === id);
```

新增 `GetNode` RPC 后改为直接查询。

## 实施顺序

1. **CSS Token 修复** — 先修复 tailwind.config.ts + 替换硬编码颜色（无功能变更，安全）
2. **共享组件提取** — 提取 Input/Toggle/Textarea，验证两个表单仍正常工作
3. **NavBar 响应式** — 安装 Radix Dialog，改造 NavBar
4. **ResponsiveTable 组件** — 创建组件，改造 PeerTable
5. **其他表格改造** — NodesTable、CommunityRules、FlapDashboard、ProbeDashboard
6. **统计卡片 + 控件适配** — FlapDashboard stats、LookingGlass controls
7. **Settings 表单补全** — 添加 3 个缺失字段
8. **GetNode RPC** — proto 变更 + 后端实现 + 前端 hook 改造
9. **RestartWireGuard UI** — StatusPage 添加按钮
10. **验证** — pnpm exec tsc --noEmit 类型检查 + cargo build 确认 proto 生成

## 验证标准

- [ ] `pnpm exec tsc --noEmit` 无类型错误
- [ ] `cargo build` 成功（proto 生成正常）
- [ ] Chrome DevTools 375px 宽度下：
  - NavBar 显示汉堡菜单，抽屉可正常打开/关闭
  - PeerTable 显示卡片布局
  - 表单单列排列
  - 统计卡片单列或两列
- [ ] Desktop（1280px+）下所有页面布局无变化
- [ ] Settings 页面显示全部 26 个 proto 字段
- [ ] Status 页面 Restart WireGuard 按钮可正常调用
- [ ] Node 详情页 `useNode` 使用 `getNode` RPC
