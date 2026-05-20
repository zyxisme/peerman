import { Link, useLocation } from 'react-router-dom';
import { Plus, Settings, Download, Home, Cable, Server, Activity, Tag, Search, AlertCircle, LogIn, LogOut } from 'lucide-react';
import { cn } from '../../lib/utils';
import { useAuth } from '../../lib/auth';
import { useClusterHealth } from '../../hooks/useNodes';

const links = [
  { to: '/', label: 'Home', icon: Home },
  { to: '/peers/new', label: 'New Peer', icon: Plus },
  { to: '/nodes', label: 'Nodes', icon: Server },
  { to: '/probes', label: 'Probes', icon: Activity },
  { to: '/communities', label: 'Communities', icon: Tag },
  { to: '/looking-glass', label: 'LG', icon: Search },
  { to: '/flaps', label: 'Flaps', icon: AlertCircle },
  { to: '/export', label: 'Export', icon: Download },
  { to: '/settings', label: 'Settings', icon: Settings },
];

export default function NavBar() {
  const location = useLocation();
  const { isAuthenticated, username, logout } = useAuth();
  const health = useClusterHealth();
  const dotColor =
    health === 'all-online' ? 'bg-green-500' :
    health === 'partial' ? 'bg-yellow-500' :
    'bg-red-500';
  const dotTitle =
    health === 'all-online' ? 'All nodes online' :
    health === 'partial' ? 'Some nodes offline' :
    'Only local node online';

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

        {/* Nav links */}
        <div className="flex items-center gap-1">
          {links.map((link) => {
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

        {/* Auth section */}
        <div className="flex items-center gap-2 min-w-[120px] justify-end">
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
      </div>
    </nav>
  );
}
