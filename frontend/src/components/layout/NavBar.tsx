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
              className="md:hidden flex items-center justify-center w-10 h-10 rounded-sm hover:bg-canvas-soft transition-colors"
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
