# Mobile Responsive + Frontend-Backend Disconnect Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Peerman frontend fully usable on mobile devices and fix all frontend-backend disconnect issues.

**Architecture:** Radix Dialog for NavBar drawer, CSS-based ResponsiveTable for card layout, shared UI components extracted to `components/ui/`, semantic CSS tokens replacing hardcoded colors, and proto-level fixes for missing fields/RPCs.

**Tech Stack:** React 18, Tailwind CSS 3.4, `@radix-ui/react-dialog`, tonic (Rust gRPC), protobuf

---

## File Map

### New Files
- `frontend/src/components/ui/Input.tsx` — shared Input component
- `frontend/src/components/ui/Textarea.tsx` — shared Textarea component
- `frontend/src/components/ui/Toggle.tsx` — shared Toggle component
- `frontend/src/components/ui/ResponsiveTable.tsx` — responsive table wrapper

### Modified Files (Frontend)
- `frontend/package.json` — add `@radix-ui/react-dialog`
- `frontend/tailwind.config.ts` — add semantic color tokens
- `frontend/src/styles/globals.css` — add drawer animation + responsive table styles
- `frontend/src/components/layout/NavBar.tsx` — hamburger menu + drawer
- `frontend/src/components/peers/PeerTable.tsx` — use ResponsiveTable
- `frontend/src/components/nodes/NodesTable.tsx` — use ResponsiveTable
- `frontend/src/components/communities/CommunityRules.tsx` — use ResponsiveTable
- `frontend/src/components/flaps/FlapDashboard.tsx` — ResponsiveTable + responsive stats grid
- `frontend/src/components/probes/ProbeDashboard.tsx` — overflow fix
- `frontend/src/components/bird/LookingGlass.tsx` — responsive controls
- `frontend/src/components/peers/PeerForm.tsx` — use shared Input/Toggle
- `frontend/src/components/settings/SettingsForm.tsx` — use shared Input/Toggle/Textarea + 3 new fields
- `frontend/src/components/status/StatusPage.tsx` — RestartWireGuard button + semantic tokens
- `frontend/src/components/ErrorBoundary.tsx` — fix bg-bg
- `frontend/src/hooks/useNodes.ts` — use GetNode RPC

### Modified Files (Backend)
- `proto/peerman.proto` — add GetNode RPC
- `src/grpc/cluster_service.rs` — implement GetNode

---

## Task 1: CSS Token Fixes

**Files:**
- Modify: `frontend/tailwind.config.ts`
- Modify: `frontend/src/styles/globals.css`

- [ ] **Step 1: Add semantic color tokens to tailwind.config.ts**

Add `success-bg`, `warning-bg`, `error-bg` tokens to the `colors` section in `frontend/tailwind.config.ts`. These replace hardcoded `bg-green-500/20` etc.

```ts
// In tailwind.config.ts colors, add after existing success/error/warning:
colors: {
  // ... existing tokens ...
  success: '#0070f3',
  'success-bg': 'rgba(0, 112, 243, 0.06)',
  error: {
    DEFAULT: '#ee0000',
    soft: '#f7d4d6',
    deep: '#c50000',
  },
  'error-bg': 'rgba(238, 0, 0, 0.06)',
  warning: {
    DEFAULT: '#f5a623',
    soft: '#ffefcf',
    deep: '#ab570a',
  },
  'warning-bg': 'rgba(245, 166, 35, 0.06)',
}
```

Note: `success` already exists as `#0070f3`. Just add `success-bg`. For `error` and `warning`, they already exist as objects — add the `-bg` variant to each.

- [ ] **Step 2: Fix ErrorBoundary.tsx — bg-bg → bg-canvas**

In `frontend/src/components/ErrorBoundary.tsx` line 21, change `bg-bg` to `bg-canvas`:

```tsx
<div className="flex items-center justify-center min-h-screen bg-canvas">
```

- [ ] **Step 3: Fix SettingsForm.tsx — bg-surface-3 → bg-hairline**

In `frontend/src/components/settings/SettingsForm.tsx` line 290, change `bg-surface-3` to `bg-hairline`:

```tsx
className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${checked ? 'bg-cyan' : 'bg-hairline'}`}
```

- [ ] **Step 4: Replace hardcoded colors in NavBar.tsx**

In `frontend/src/components/layout/NavBar.tsx` lines 28-30, replace:

```tsx
// Before:
const dotColor =
  health === 'all-online' ? 'bg-green-500' :
  health === 'partial' ? 'bg-yellow-500' :
  'bg-red-500';

// After:
const dotColor =
  health === 'all-online' ? 'bg-success' :
  health === 'partial' ? 'bg-warning' :
  'bg-error';
```

- [ ] **Step 5: Replace hardcoded colors in StatusPage.tsx**

In `frontend/src/components/status/StatusPage.tsx`, replace all hardcoded color classes:

Line 82 (BIRD protocol state badges):
```tsx
// Before:
className={`badge ${p.state === 'up' ? 'bg-green-500/20 text-green-500' : 'bg-red-500/20 text-red-500'}`}
// After:
className={`badge ${p.state === 'up' ? 'bg-success-bg text-success' : 'bg-error-bg text-error'}`}
```

Lines 106-112 (Config Status badges):
```tsx
// Before:
applyStatus.status.pending
  ? 'bg-yellow-500/20 text-yellow-500'
  : applyStatus.status.lastError
    ? 'bg-red-500/20 text-red-500'
    : 'bg-green-500/20 text-green-500'
// After:
applyStatus.status.pending
  ? 'bg-warning-bg text-warning'
  : applyStatus.status.lastError
    ? 'bg-error-bg text-error'
    : 'bg-success-bg text-success'
```

- [ ] **Step 6: Commit**

```bash
cd frontend && git add tailwind.config.ts src/styles/globals.css src/components/ErrorBoundary.tsx src/components/settings/SettingsForm.tsx src/components/layout/NavBar.tsx src/components/status/StatusPage.tsx
git commit -m "fix: semantic CSS tokens, replace hardcoded colors, fix invalid class refs"
```

---

## Task 2: Extract Shared UI Components

**Files:**
- Create: `frontend/src/components/ui/Input.tsx`
- Create: `frontend/src/components/ui/Textarea.tsx`
- Create: `frontend/src/components/ui/Toggle.tsx`

- [ ] **Step 1: Create shared Input component**

Create `frontend/src/components/ui/Input.tsx`:

```tsx
export interface InputProps {
  label: string;
  value: string;
  onChange: (v: string) => void;
  type?: string;
  placeholder?: string;
  required?: boolean;
  mono?: boolean;
}

export function Input({
  label, value, onChange, type = 'text', placeholder, required, mono,
}: InputProps) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-caption text-mute">{label}</label>
      <input
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        required={required}
        className={`form-input ${mono ? 'font-mono' : ''}`}
      />
    </div>
  );
}
```

- [ ] **Step 2: Create shared Textarea component**

Create `frontend/src/components/ui/Textarea.tsx`:

```tsx
export interface TextareaProps {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  code?: boolean;
}

export function Textarea({
  label, value, onChange, placeholder, code,
}: TextareaProps) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-caption text-mute">{label}</label>
      <textarea
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        rows={3}
        className={code ? 'form-input font-mono' : 'form-input'}
        style={code ? { fontFamily: 'Geist Mono, ui-monospace, monospace', fontSize: '13px' } : undefined}
      />
    </div>
  );
}
```

- [ ] **Step 3: Create shared Toggle component**

Create `frontend/src/components/ui/Toggle.tsx`. Uses SettingsForm's interface (with `description`, `role`, `aria-checked`), fixes the broken `bg-surface-3`:

```tsx
export interface ToggleProps {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}

export function Toggle({
  label, description, checked, onChange, disabled,
}: ToggleProps) {
  return (
    <div className="flex items-start gap-sm">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${checked ? 'bg-cyan' : 'bg-hairline'}`}
      >
        <span
          className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out ${checked ? 'translate-x-4' : 'translate-x-0'}`}
        />
      </button>
      <div className="flex flex-col gap-0.5">
        <span className="text-body-sm-strong text-ink">{label}</span>
        {description && <span className="text-caption text-mute">{description}</span>}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Update PeerForm.tsx to use shared components**

In `frontend/src/components/peers/PeerForm.tsx`:

1. Add import at top:
```tsx
import { Input } from '../ui/Input';
import { Toggle } from '../ui/Toggle';
```

2. Delete the local `Input` function (lines 256-275) and local `Toggle` function (lines 277-296).

3. Update Toggle usage — PeerForm's Toggle uses `label, checked, onChange` (no `description`), which is compatible with the shared component (description is optional).

- [ ] **Step 5: Update SettingsForm.tsx to use shared components**

In `frontend/src/components/settings/SettingsForm.tsx`:

1. Add import at top:
```tsx
import { Input } from '../ui/Input';
import { Textarea } from '../ui/Textarea';
import { Toggle } from '../ui/Toggle';
```

2. Delete the local `Input` function (lines 239-256), local `Textarea` function (lines 258-276), and local `Toggle` function (lines 278-302).

- [ ] **Step 6: Verify TypeScript compiles**

Run: `cd frontend && pnpm exec tsc --noEmit`
Expected: No errors.

- [ ] **Step 7: Commit**

```bash
git add frontend/src/components/ui/ frontend/src/components/peers/PeerForm.tsx frontend/src/components/settings/SettingsForm.tsx
git commit -m "refactor: extract shared Input, Textarea, Toggle components to ui/"
```

---

## Task 3: NavBar Responsive (Radix Dialog)

**Files:**
- Modify: `frontend/package.json`
- Modify: `frontend/src/components/layout/NavBar.tsx`
- Modify: `frontend/src/styles/globals.css`

- [ ] **Step 1: Install @radix-ui/react-dialog**

Run: `cd frontend && pnpm add @radix-ui/react-dialog`

- [ ] **Step 2: Add drawer animation to globals.css**

Append to `frontend/src/styles/globals.css` (after the `@layer components` block):

```css
/* Mobile drawer overlay */
.drawer-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  z-index: 40;
  animation: fade-in-overlay 0.2s ease-out;
}

@keyframes fade-in-overlay {
  from { opacity: 0; }
  to { opacity: 1; }
}

/* Mobile drawer content */
.drawer-content {
  position: fixed;
  top: 0;
  left: 0;
  bottom: 0;
  width: 280px;
  background: white;
  z-index: 50;
  box-shadow: 4px 0 20px rgba(0, 0, 0, 0.1);
  animation: slide-in-drawer 0.2s ease-out;
  overflow-y: auto;
}

@keyframes slide-in-drawer {
  from { transform: translateX(-100%); }
  to { transform: translateX(0); }
}
```

- [ ] **Step 3: Rewrite NavBar.tsx with Radix Dialog drawer**

Replace `frontend/src/components/layout/NavBar.tsx` with:

```tsx
import { useState } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { Plus, Settings, Download, Home, Cable, Server, Activity, Tag, Search, AlertCircle, LogIn, LogOut, Menu, X } from 'lucide-react';
import * as Dialog from '@radix-ui/react-dialog';
import { cn } from '../../lib/utils';
import { useAuth } from '../../lib/auth';
import { useClusterHealth } from '../../hooks/useNodes';

const publicLinks = [
  { to: '/', label: 'Home', icon: Home },
  { to: '/nodes', label: 'Nodes', icon: Server },
  { to: '/probes', label: 'Probes', icon: Activity },
  { to: '/looking-glass', label: 'Looking Glass', icon: Search },
  { to: '/flaps', label: 'Flaps', icon: AlertCircle },
  { to: '/status', label: 'Status', icon: Activity },
];

const authLinks = [
  { to: '/peers/new', label: 'New Peer', icon: Plus },
  { to: '/communities', label: 'Communities', icon: Tag },
  { to: '/export', label: 'Export', icon: Download },
  { to: '/settings', label: 'Settings', icon: Settings },
];

export default function NavBar() {
  const location = useLocation();
  const { isAuthenticated, username, logout } = useAuth();
  const health = useClusterHealth();
  const [open, setOpen] = useState(false);

  const dotColor =
    health === 'all-online' ? 'bg-success' :
    health === 'partial' ? 'bg-warning' :
    'bg-error';
  const dotTitle =
    health === 'all-online' ? 'All nodes online' :
    health === 'partial' ? 'Some nodes offline' :
    'Only local node online';

  // Close drawer on navigation
  const handleNav = () => setOpen(false);

  return (
    <nav className="nav-bar">
      <div className="max-w-[1400px] w-full mx-auto flex items-center justify-between">
        {/* Logo */}
        <Link to="/" className="flex items-center gap-2 text-ink no-underline">
          <Cable className="w-5 h-5" />
          <span
            className={`inline-block w-2 h-2 rounded-full flex-shrink-0 ${dotColor}`}
            title={dotTitle}
          />
          <span className="text-body-md-strong tracking-tight">Peerman</span>
        </Link>

        {/* Desktop nav links — hidden on mobile */}
        <div className="hidden md:flex items-center gap-1">
          {publicLinks.map((link) => {
            const isActive = location.pathname === link.to;
            const Icon = link.icon;
            return (
              <Link
                key={link.to}
                to={link.to}
                className={cn(
                  'flex items-center gap-1.5 rounded-full px-sm h-8 text-body-sm transition-colors',
                  isActive
                    ? 'bg-primary text-primary-foreground'
                    : 'text-body hover:bg-canvas-soft'
                )}
              >
                <Icon className="w-3.5 h-3.5" />
                {link.label}
              </Link>
            );
          })}
          {isAuthenticated && authLinks.map((link) => {
            const isActive = location.pathname === link.to;
            const Icon = link.icon;
            return (
              <Link
                key={link.to}
                to={link.to}
                className={cn(
                  'flex items-center gap-1.5 rounded-full px-sm h-8 text-body-sm transition-colors',
                  isActive
                    ? 'bg-primary text-primary-foreground'
                    : 'text-body hover:bg-canvas-soft'
                )}
              >
                <Icon className="w-3.5 h-3.5" />
                {link.label}
              </Link>
            );
          })}
        </div>

        {/* Desktop auth section — hidden on mobile */}
        <div className="hidden md:flex items-center gap-2 min-w-[120px] justify-end">
          {isAuthenticated ? (
            <>
              <span className="text-body-sm text-body">{username}</span>
              <button
                onClick={logout}
                className="flex items-center gap-1.5 rounded-full px-sm h-8 text-body-sm transition-colors text-body hover:bg-canvas-soft"
              >
                <LogOut className="w-3.5 h-3.5" />
                Logout
              </button>
            </>
          ) : (
            <Link
              to="/login"
              className="flex items-center gap-1.5 rounded-full px-sm h-8 text-body-sm transition-colors text-body hover:bg-canvas-soft"
            >
              <LogIn className="w-3.5 h-3.5" />
              Login
            </Link>
          )}
        </div>

        {/* Mobile hamburger — visible only on mobile */}
        <Dialog.Root open={open} onOpenChange={setOpen}>
          <Dialog.Trigger asChild>
            <button
              className="md:hidden flex items-center justify-center w-8 h-8 rounded-sm hover:bg-canvas-soft transition-colors"
              aria-label="Open menu"
            >
              <Menu className="w-5 h-5 text-ink" />
            </button>
          </Dialog.Trigger>

          <Dialog.Portal>
            <Dialog.Overlay className="drawer-overlay" />
            <Dialog.Content className="drawer-content" aria-label="Navigation menu">
              <div className="flex items-center justify-between px-md py-sm border-b border-hairline">
                <div className="flex items-center gap-2">
                  <Cable className="w-4 h-4" />
                  <span
                    className={`inline-block w-2 h-2 rounded-full flex-shrink-0 ${dotColor}`}
                    title={dotTitle}
                  />
                  <span className="text-body-sm-strong">Peerman</span>
                </div>
                <Dialog.Close asChild>
                  <button
                    className="flex items-center justify-center w-8 h-8 rounded-sm hover:bg-canvas-soft transition-colors"
                    aria-label="Close menu"
                  >
                    <X className="w-4 h-4 text-body" />
                  </button>
                </Dialog.Close>
              </div>

              <div className="py-sm">
                {publicLinks.map((link) => {
                  const isActive = location.pathname === link.to;
                  const Icon = link.icon;
                  return (
                    <Link
                      key={link.to}
                      to={link.to}
                      onClick={handleNav}
                      className={cn(
                        'flex items-center gap-3 px-md py-2.5 text-body-sm transition-colors mx-xs rounded-sm',
                        isActive
                          ? 'bg-primary text-primary-foreground'
                          : 'text-body hover:bg-canvas-soft'
                      )}
                    >
                      <Icon className="w-4 h-4 flex-shrink-0" />
                      {link.label}
                    </Link>
                  );
                })}

                {isAuthenticated && (
                  <>
                    <div className="border-t border-hairline my-sm mx-md" />
                    {authLinks.map((link) => {
                      const isActive = location.pathname === link.to;
                      const Icon = link.icon;
                      return (
                        <Link
                          key={link.to}
                          to={link.to}
                          onClick={handleNav}
                          className={cn(
                            'flex items-center gap-3 px-md py-2.5 text-body-sm transition-colors mx-xs rounded-sm',
                            isActive
                              ? 'bg-primary text-primary-foreground'
                              : 'text-body hover:bg-canvas-soft'
                          )}
                        >
                          <Icon className="w-4 h-4 flex-shrink-0" />
                          {link.label}
                        </Link>
                      );
                    })}
                  </>
                )}
              </div>

              {/* Auth section at bottom */}
              <div className="border-t border-hairline px-md py-sm">
                {isAuthenticated ? (
                  <div className="flex items-center justify-between">
                    <span className="text-body-sm text-body">{username}</span>
                    <button
                      onClick={() => { logout(); handleNav(); }}
                      className="flex items-center gap-1.5 px-sm py-1.5 text-body-sm text-body hover:bg-canvas-soft rounded-sm transition-colors"
                    >
                      <LogOut className="w-3.5 h-3.5" />
                      Logout
                    </button>
                  </div>
                ) : (
                  <Link
                    to="/login"
                    onClick={handleNav}
                    className="flex items-center gap-1.5 px-sm py-1.5 text-body-sm text-body hover:bg-canvas-soft rounded-sm transition-colors"
                  >
                    <LogIn className="w-3.5 h-3.5" />
                    Login
                  </Link>
                )}
              </div>
            </Dialog.Content>
          </Dialog.Portal>
        </Dialog.Root>
      </div>
    </nav>
  );
}
```

- [ ] **Step 4: Verify TypeScript compiles**

Run: `cd frontend && pnpm exec tsc --noEmit`
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add frontend/package.json frontend/pnpm-lock.yaml frontend/src/components/layout/NavBar.tsx frontend/src/styles/globals.css
git commit -m "feat: responsive NavBar with Radix Dialog drawer for mobile"
```

---

## Task 4: ResponsiveTable Component

**Files:**
- Create: `frontend/src/components/ui/ResponsiveTable.tsx`
- Modify: `frontend/src/styles/globals.css`

- [ ] **Step 1: Add responsive table CSS to globals.css**

Append to `frontend/src/styles/globals.css`:

```css
/* Responsive table — card layout on mobile */
@media (max-width: 767px) {
  .responsive-table thead {
    display: none;
  }

  .responsive-table tbody tr {
    display: block;
    border: 1px solid #ebebeb;
    border-radius: 8px;
    padding: 12px;
    margin-bottom: 8px;
    background: white;
  }

  .responsive-table tbody tr:last-child {
    margin-bottom: 0;
  }

  .responsive-table tbody td {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 4px 0;
    border-bottom: none;
    font-size: 13px;
  }

  .responsive-table tbody td::before {
    content: attr(data-label);
    color: #888888;
    font-size: 12px;
    flex-shrink: 0;
    margin-right: 12px;
  }

  .responsive-table tbody td:last-child {
    border-top: 1px solid #ebebeb;
    margin-top: 8px;
    padding-top: 8px;
    justify-content: flex-end;
    gap: 8px;
  }

  .responsive-table tbody td:last-child::before {
    display: none;
  }
}
```

- [ ] **Step 2: Create ResponsiveTable wrapper component**

Create `frontend/src/components/ui/ResponsiveTable.tsx`:

```tsx
import { type ReactNode } from 'react';

interface ResponsiveTableProps {
  children: ReactNode;
  className?: string;
}

export function ResponsiveTable({ children, className }: ResponsiveTableProps) {
  return (
    <div className={className}>
      <table className="data-table responsive-table w-full">
        {children}
      </table>
    </div>
  );
}
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/ui/ResponsiveTable.tsx frontend/src/styles/globals.css
git commit -m "feat: add ResponsiveTable component with mobile card layout CSS"
```

---

## Task 5: Apply ResponsiveTable to PeerTable

**Files:**
- Modify: `frontend/src/components/peers/PeerTable.tsx`

- [ ] **Step 1: Update PeerTable to use ResponsiveTable**

In `frontend/src/components/peers/PeerTable.tsx`:

1. Add import:
```tsx
import { ResponsiveTable } from '../ui/ResponsiveTable';
```

2. Replace the `<table className="data-table">` with `<ResponsiveTable>` and add `data-label` attributes to each `<td>`:

```tsx
// Replace lines 59-148 (the <table> block) with:
<ResponsiveTable>
  <thead>
    <tr>
      <th>Name</th>
      <th>ASN</th>
      <th>Endpoint</th>
      <th>Tunnel</th>
      <th>Sessions</th>
      <th>Origin Node</th>
      <th>Status</th>
      <th>Actions</th>
    </tr>
  </thead>
  <tbody>
    {peers.map((peer) => {
      const isStale = peer.originNodeId && offlineNodeIds.has(peer.originNodeId);
      return (
        <tr
          key={peer.id}
          className={isStale ? 'opacity-50' : ''}
          title={isStale ? 'Node offline; data from cache' : undefined}
        >
          <td data-label="Name">
            <button
              className="text-link hover:text-link-deep font-medium"
              onClick={() => navigate(`/peers/${peer.id}`)}
            >
              {peer.name}
            </button>
          </td>
          <td data-label="ASN" className="text-caption-mono">AS{peer.asn.toString()}</td>
          <td data-label="Endpoint" className="text-caption-mono">
            {peer.wgRemoteAddress}:{peer.wgRemotePort || '—'}
          </td>
          <td data-label="Tunnel" className="text-caption-mono">
            {peer.ipv6TunnelLocal || peer.ipv4TunnelLocal || '—'}
          </td>
          <td data-label="Sessions">
            <span className="badge">
              {peer.sessions === 0 ? 'IPv4' : peer.sessions === 1 ? 'IPv6' : 'Both'}
              {peer.multiprotocol ? ' MP' : ''}
            </span>
          </td>
          <td data-label="Node" className="text-caption text-mute">
            {peer.originNodeId ? nodeName(peer.originNodeId) : 'local'}
          </td>
          <td data-label="Status">
            <button
              onClick={() => handleToggle(peer.id)}
              className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                peer.enabled ? 'bg-primary' : 'bg-hairline-strong'
              }`}
            >
              <span
                className={`inline-block h-3.5 w-3.5 rounded-full bg-white transition-transform ${
                  peer.enabled ? 'translate-x-[18px]' : 'translate-x-[3px]'
                }`}
              />
            </button>
          </td>
          <td data-label="Actions">
            <div className="flex items-center gap-1">
              <button
                onClick={() => navigate(`/peers/${peer.id}`)}
                className="p-1 rounded-xs hover:bg-canvas-soft text-body hover:text-ink"
                title="View"
              >
                <Eye className="w-3.5 h-3.5" />
              </button>
              <button
                onClick={() => navigate(`/peers/${peer.id}/edit`)}
                className="p-1 rounded-xs hover:bg-canvas-soft text-body hover:text-ink"
                title="Edit"
              >
                <Pencil className="w-3.5 h-3.5" />
              </button>
              <button
                onClick={() => handleDelete(peer.id, peer.name)}
                className="p-1 rounded-xs hover:bg-error-soft text-body hover:text-error"
                title="Delete"
              >
                <Trash2 className="w-3.5 h-3.5" />
              </button>
            </div>
          </td>
        </tr>
      );
    })}
  </tbody>
</ResponsiveTable>
```

Also remove the outer `<div className="card overflow-hidden">` wrapping and replace with `<div className="card overflow-hidden">` keeping the header section, but the table inside uses ResponsiveTable.

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd frontend && pnpm exec tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/peers/PeerTable.tsx
git commit -m "feat: PeerTable mobile card layout via ResponsiveTable"
```

---

## Task 6: Apply ResponsiveTable to Other Tables

**Files:**
- Modify: `frontend/src/components/nodes/NodesTable.tsx`
- Modify: `frontend/src/components/communities/CommunityRules.tsx`
- Modify: `frontend/src/components/flaps/FlapDashboard.tsx`

- [ ] **Step 1: Update NodesTable.tsx**

In `frontend/src/components/nodes/NodesTable.tsx`:

1. Add import:
```tsx
import { ResponsiveTable } from '../ui/ResponsiveTable';
```

2. Replace `<table className="data-table w-full">` with `<ResponsiveTable>` and add `data-label` to each `<td>`:

```tsx
<ResponsiveTable className="card overflow-hidden !p-0">
  <thead>
    <tr>
      <th>Name</th>
      <th>Listen Address</th>
      <th>ASN</th>
      <th>Status</th>
      <th>Last Seen</th>
      <th>Actions</th>
    </tr>
  </thead>
  <tbody>
    {nodes.map((node) => (
      <tr key={node.id}>
        <td data-label="Name">
          <Link to={`/nodes/${node.id}`} className="text-link font-medium no-underline hover:underline">
            {node.name}
          </Link>
        </td>
        <td data-label="Address"><code className="text-code text-body-sm">{node.listenAddr}</code></td>
        <td data-label="ASN" className="text-body-sm">AS{node.localAsn > 0n ? String(node.localAsn) : '—'}</td>
        <td data-label="Status">
          <span className={cn(
            'inline-flex items-center gap-1.5 rounded-full px-xs py-0.5 text-caption font-medium',
            node.online ? 'bg-link-bg-soft text-link-deep' : 'bg-canvas-soft text-mute'
          )}>
            <span className={cn('w-1.5 h-1.5 rounded-full', node.online ? 'bg-success' : 'bg-hairline-strong')} />
            {node.online ? 'Online' : 'Offline'}
          </span>
        </td>
        <td data-label="Last Seen" className="text-body-sm text-mute">{node.lastSeenAt?.slice(0, 19).replace('T', ' ') ?? '—'}</td>
        <td data-label="Actions">
          <div className="flex items-center gap-1">
            <button
              onClick={() => handleProbe(nodes[0]?.id, node.id)}
              disabled={probeLoading || !nodes[0] || nodes[0].id === node.id}
              className="p-1 rounded-sm hover:bg-canvas-soft text-body disabled:opacity-30"
              title="Probe"
            >
              <RefreshCw className={cn('w-3.5 h-3.5', probeLoading && 'animate-spin')} />
            </button>
            <button
              onClick={() => handleDelete(node.id)}
              className="p-1 rounded-sm hover:bg-error-soft text-mute hover:text-error"
              title="Delete"
            >
              <Trash2 className="w-3.5 h-3.5" />
            </button>
          </div>
        </td>
      </tr>
    ))}
    {/* ... empty state row stays the same ... */}
  </tbody>
</ResponsiveTable>
```

- [ ] **Step 2: Update CommunityRules.tsx**

Read the full CommunityRules.tsx first (it's a large file with inline editing), then apply the same pattern: import ResponsiveTable, add `data-label` to each `<td>`.

- [ ] **Step 3: Update FlapDashboard.tsx — tables + stats grid**

In `frontend/src/components/flaps/FlapDashboard.tsx`:

1. Add import:
```tsx
import { ResponsiveTable } from '../ui/ResponsiveTable';
```

2. Update stats cards grid (line 50):
```tsx
// Before:
<div className="grid grid-cols-3 gap-lg">
// After:
<div className="grid grid-cols-2 sm:grid-cols-3 gap-lg">
```

3. Replace both `<table className="data-table">` instances with `<ResponsiveTable>` and add `data-label` attributes.

- [ ] **Step 4: Update ProbeDashboard.tsx — overflow fix**

In `frontend/src/components/probes/ProbeDashboard.tsx`, find the latency matrix wrapper and add `overflow-x-auto` if missing. Also wrap any plain `<table>` with `<ResponsiveTable>`.

- [ ] **Step 5: Verify TypeScript compiles**

Run: `cd frontend && pnpm exec tsc --noEmit`
Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/nodes/NodesTable.tsx frontend/src/components/communities/CommunityRules.tsx frontend/src/components/flaps/FlapDashboard.tsx frontend/src/components/probes/ProbeDashboard.tsx
git commit -m "feat: apply ResponsiveTable to NodesTable, CommunityRules, FlapDashboard, ProbeDashboard"
```

---

## Task 7: LookingGlass Responsive Controls

**Files:**
- Modify: `frontend/src/components/bird/LookingGlass.tsx`

- [ ] **Step 1: Update LookingGlass controls for mobile**

In `frontend/src/components/bird/LookingGlass.tsx`, update the controls section (lines 52-83):

```tsx
{/* Controls */}
<div className="card space-y-lg">
  <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-sm sm:gap-md">
    <select
      value={targetNodeId}
      onChange={(e) => setTargetNodeId(e.target.value)}
      className="form-input sm:w-48"
    >
      <option value="">All Nodes</option>
      {nodes.map((n) => (
        <option key={n.id} value={n.id}>
          {n.name}
        </option>
      ))}
    </select>

    <input
      type="text"
      value={command}
      onChange={(e) => setCommand(e.target.value)}
      onKeyDown={(e) => e.key === 'Enter' && handleExecute()}
      placeholder="show route for 8.8.8.8..."
      className="form-input flex-1"
    />

    <button
      onClick={handleExecute}
      disabled={loading || !command.trim()}
      className="btn-primary"
    >
      <Search className="w-4 h-4" />
      Execute
    </button>
  </div>
  {/* ... presets stay the same ... */}
```

Also update the traceroute form (line 153):
```tsx
<form onSubmit={handleTrace} className="flex flex-col sm:flex-row items-stretch sm:items-center gap-sm sm:gap-md">
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/components/bird/LookingGlass.tsx
git commit -m "feat: responsive LookingGlass controls for mobile"
```

---

## Task 8: Settings Form — Add 3 Missing Fields

**Files:**
- Modify: `frontend/src/components/settings/SettingsForm.tsx`

- [ ] **Step 1: Add 3 missing fields to DEFAULT_FORM**

In `frontend/src/components/settings/SettingsForm.tsx`, add to `DEFAULT_FORM` (after line 31):

```ts
clusterTunnelIpv6Range: '',
enableConfederation: false,
confederationLocalAsn: '0',
```

- [ ] **Step 2: Add fields to useEffect form population**

In the `useEffect` that populates form from settings (line 42), add after line 68:

```ts
clusterTunnelIpv6Range: settings.clusterTunnelIpv6Range || '',
enableConfederation: settings.enableConfederation || false,
confederationLocalAsn: String(settings.confederationLocalAsn || '0'),
```

- [ ] **Step 3: Add fields to handleSubmit**

In `handleSubmit` (line 72), add to the `create(SettingsSchema, {...})` call:

```ts
clusterTunnelIpv6Range: form.clusterTunnelIpv6Range,
enableConfederation: form.enableConfederation,
confederationLocalAsn: BigInt(form.confederationLocalAsn || '0'),
```

- [ ] **Step 4: Add Cluster Configuration section to the form**

Add a new card section after the BFD section (before the submit button):

```tsx
{/* Cluster Configuration */}
<div className="card space-y-md">
  <h2 className="text-body-sm-strong text-ink">Cluster Configuration</h2>
  <p className="text-body-sm text-mute">IPv6 tunnel range and BGP confederation settings for multi-node clusters.</p>
  <div className="grid grid-cols-1 gap-sm">
    <Input
      label="IPv6 Tunnel Range"
      value={f('clusterTunnelIpv6Range')}
      onChange={(v) => setForm((p) => ({ ...p, clusterTunnelIpv6Range: v }))}
      placeholder="fd42:cluster::/48"
    />
  </div>
  <Toggle
    label="Enable BGP Confederation"
    description="Use BGP confederation instead of iBGP full mesh for inter-node routing."
    checked={form.enableConfederation}
    onChange={(v) => setForm((p) => ({ ...p, enableConfederation: v }))}
  />
  {form.enableConfederation && (
    <div className="grid grid-cols-1 gap-sm">
      <Input
        label="Confederation Local ASN"
        value={f('confederationLocalAsn')}
        onChange={(v) => setForm((p) => ({ ...p, confederationLocalAsn: v }))}
        type="number"
        placeholder="65000"
      />
    </div>
  )}
</div>
```

- [ ] **Step 5: Verify TypeScript compiles**

Run: `cd frontend && pnpm exec tsc --noEmit`
Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/settings/SettingsForm.tsx
git commit -m "feat: add IPv6 tunnel range, confederation toggle + ASN to Settings form"
```

---

## Task 9: GetNode RPC (Proto + Backend + Frontend)

**Files:**
- Modify: `proto/peerman.proto`
- Modify: `src/grpc/cluster_service.rs`
- Modify: `frontend/src/hooks/useNodes.ts`

- [ ] **Step 1: Add GetNode RPC to proto**

In `proto/peerman.proto`, add after the `ListNodes` line in `ClusterService`:

```protobuf
rpc GetNode(GetNodeRequest) returns (Node);
```

Add the request message after `ListNodesResponse`:

```protobuf
message GetNodeRequest { string id = 1; }
```

- [ ] **Step 2: Implement GetNode in backend**

In `src/grpc/cluster_service.rs`:

1. Add `GetNodeRequest` to the imports (line 4-11):
```rust
use super::generated::{
    // ... existing imports ...
    GetNodeRequest, GetNodeResponse,
    // ... rest ...
};
```

Wait — the proto returns `Node` directly, not a wrapper. So just add `GetNodeRequest` to imports.

2. Add the `get_node` method to `ClusterServiceImpl` (after `list_nodes`):

```rust
async fn get_node(
    &self,
    request: Request<GetNodeRequest>,
) -> Result<Response<Node>, Status> {
    crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
    let req = request.into_inner();
    let node = self
        .node_repo
        .find_by_id(&req.id)
        .await
        .map_err(|e| Status::not_found(e.to_string()))?;

    Ok(Response::new(node_to_proto(&node)))
}
```

- [ ] **Step 3: Regenerate proto stubs**

Run:
```bash
cd /home/cc/peerman
source "$HOME/.cargo/env"
cargo build 2>&1 | tail -5
```

This regenerates the Rust proto stubs. Then regenerate frontend stubs:

```bash
cd /home/cc/peerman
PATH="frontend/node_modules/.bin:$PATH" protoc -I proto --es_out frontend/src/lib --es_opt target=ts proto/peerman.proto
```

- [ ] **Step 4: Update useNodes.ts to use GetNode RPC**

Replace `frontend/src/hooks/useNodes.ts` `useNode` function (lines 39-44):

```tsx
export function useNode(id: string | undefined) {
  const [node, setNode] = useState<Node | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchNode = useCallback(async () => {
    if (!id) {
      setLoading(false);
      return;
    }
    try {
      setLoading(true);
      const res = await clusterClient.getNode({ id });
      setNode(res);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => {
    fetchNode();
  }, [fetchNode]);

  return { node, loading, error };
}
```

Also add the necessary imports at the top:
```tsx
import { useCallback, useEffect, useState } from 'react';
import type { Node } from '../lib/peerman_pb';
import { clusterClient } from '../lib/grpc';
```

- [ ] **Step 5: Verify build**

Run:
```bash
cd /home/cc/peerman
source "$HOME/.cargo/env"
cargo build 2>&1 | tail -5
```
Expected: Build succeeds.

Run:
```bash
cd /home/cc/peerman/frontend && pnpm exec tsc --noEmit
```
Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add proto/peerman.proto src/grpc/cluster_service.rs frontend/src/hooks/useNodes.ts frontend/src/lib/peerman_pb.ts
git commit -m "feat: add GetNode RPC, update useNode hook to use direct query"
```

---

## Task 10: RestartWireGuard UI

**Files:**
- Modify: `frontend/src/components/status/StatusPage.tsx`
- Create or modify: `frontend/src/hooks/usePeers.ts` (add restartWireGuard hook)

- [ ] **Step 1: Add restartWireGuard hook**

In `frontend/src/hooks/usePeers.ts`, add at the end:

```tsx
export function useRestartWireGuard() {
  const [loading, setLoading] = useState(false);

  const restart = useCallback(async () => {
    setLoading(true);
    try {
      await peerClient.restartWireGuard({});
    } finally {
      setLoading(false);
    }
  }, []);

  return { restart, loading };
}
```

Make sure `peerClient` is imported from `../lib/grpc` and `useState`, `useCallback` from `react`.

- [ ] **Step 2: Add Restart WireGuard button to StatusPage**

In `frontend/src/components/status/StatusPage.tsx`:

1. Add import:
```tsx
import { useRestartWireGuard } from '../../hooks/usePeers';
```

2. Add hook call:
```tsx
const restartWg = useRestartWireGuard();
```

3. Add a "Restart WireGuard" button in the WireGuard section (after the interfaces list, around line 51):

```tsx
<div className="flex items-center gap-md mt-md">
  <button
    onClick={async () => {
      if (!confirm('Are you sure? This will briefly disconnect all WireGuard peers.')) return;
      await restartWg.restart();
      wg.refetch();
    }}
    disabled={restartWg.loading}
    className="btn-secondary-sm"
  >
    {restartWg.loading ? 'Restarting...' : 'Restart WireGuard'}
  </button>
</div>
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd frontend && pnpm exec tsc --noEmit`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add frontend/src/hooks/usePeers.ts frontend/src/components/status/StatusPage.tsx
git commit -m "feat: add Restart WireGuard button to Status page"
```

---

## Final Verification

- [ ] **Step 1: Full TypeScript check**

Run: `cd /home/cc/peerman/frontend && pnpm exec tsc --noEmit`
Expected: No errors.

- [ ] **Step 2: Full Rust build**

Run: `cd /home/cc/peerman && source "$HOME/.cargo/env" && cargo build 2>&1 | tail -10`
Expected: Build succeeds.

- [ ] **Step 3: Run clippy**

Run: `cd /home/cc/peerman && source "$HOME/.cargo/env" && cargo clippy 2>&1 | tail -10`
Expected: No warnings.

- [ ] **Step 4: Run Rust tests**

Run: `cd /home/cc/peerman && source "$HOME/.cargo/env" && cargo test 2>&1 | tail -10`
Expected: All tests pass.
