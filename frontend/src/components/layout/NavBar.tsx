import { Link, useLocation } from 'react-router-dom';
import { Plus, Settings, Download, Home, Cable, Server, Activity, Tag } from 'lucide-react';
import { cn } from '../../lib/utils';

const links = [
  { to: '/', label: 'Home', icon: Home },
  { to: '/peers/new', label: 'New Peer', icon: Plus },
  { to: '/nodes', label: 'Nodes', icon: Server },
  { to: '/probes', label: 'Probes', icon: Activity },
  { to: '/communities', label: 'Communities', icon: Tag },
  { to: '/export', label: 'Export', icon: Download },
  { to: '/settings', label: 'Settings', icon: Settings },
];

export default function NavBar() {
  const location = useLocation();

  return (
    <nav className="nav-bar">
      <div className="max-w-[1400px] w-full mx-auto flex items-center justify-between">
        {/* Logo */}
        <Link to="/" className="flex items-center gap-2 text-ink no-underline">
          <Cable className="w-5 h-5" />
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

        {/* Spacer for symmetry */}
        <div className="w-20" />
      </div>
    </nav>
  );
}
