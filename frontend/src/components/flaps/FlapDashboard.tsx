import { useEffect, useRef } from 'react';
import { RotateCw, Activity, Calendar, BarChart3 } from 'lucide-react';
import { useFlapEvents, useFlapStats } from '../../hooks/useFlaps';
import { cn } from '../../lib/utils';
import { ResponsiveTable } from '../ui/ResponsiveTable';

const SOURCE_LABELS: Record<string, string> = {
  ibgp: 'iBGP',
  socket: 'Socket',
  probe: 'Probe',
};

function relativeTime(rfc3339: string): string {
  if (!rfc3339) return '—';
  const diff = Date.now() - new Date(rfc3339).getTime();
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

export default function FlapDashboard() {
  const { events: activeEvents, loading, error, refetch } = useFlapEvents(true);
  const { events: allEvents } = useFlapEvents(false);
  const { stats, refetch: refetchStats } = useFlapStats();
  const intervalRef = useRef<ReturnType<typeof setInterval>>();

  // Auto-refresh every 30s
  useEffect(() => {
    intervalRef.current = setInterval(() => {
      refetch();
      refetchStats();
    }, 30_000);
    return () => clearInterval(intervalRef.current);
  }, [refetch, refetchStats]);

  return (
    <div className="space-y-xl animate-fade-in">
      <div>
        <h1 className="text-display-md text-ink">Route Flap Detection</h1>
        <p className="text-body-md text-body mt-1">
          Real-time BGP route flap monitoring across cluster nodes.
        </p>
      </div>

      {/* Stats cards */}
      <div className="grid grid-cols-2 sm:grid-cols-3 gap-lg">
        <div className="card flex items-center gap-md">
          <Activity className="w-5 h-5 text-warning" />
          <div>
            <p className="text-caption-mono text-mute uppercase">Active Flaps</p>
            <p className="text-display-sm text-ink">{stats.activeCount}</p>
          </div>
        </div>
        <div className="card flex items-center gap-md">
          <Calendar className="w-5 h-5 text-body" />
          <div>
            <p className="text-caption-mono text-mute uppercase">Today's Events</p>
            <p className="text-display-sm text-ink">{stats.totalToday}</p>
          </div>
        </div>
        <div className="card flex items-center gap-md">
          <BarChart3 className="w-5 h-5 text-link" />
          <div>
            <p className="text-caption-mono text-mute uppercase">Avg Rate</p>
            <p className="text-display-sm text-ink">
              {stats.avgChangesPerHour.toFixed(1)}/h
            </p>
          </div>
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="card border border-error bg-error-soft">
          <p className="text-body-sm text-error">{error}</p>
        </div>
      )}

      {/* Loading */}
      {loading && (
        <div className="card flex items-center gap-3 text-body">
          <RotateCw className="w-4 h-4 animate-spin" />
          <span className="text-body-sm">Loading flap data...</span>
        </div>
      )}

      {/* Active Flaps */}
      {!loading && (
        <div>
          <h2 className="text-display-sm text-ink mb-md">
            Active Flaps
            <span className="text-body-md text-mute ml-2 font-normal">
              ({activeEvents.length})
            </span>
          </h2>

          {activeEvents.length === 0 ? (
            <div className="card-soft text-center py-2xl">
              <p className="text-body-md text-mute">No active flaps detected.</p>
              <p className="text-body-sm text-mute mt-1">
                BGP routes are currently stable.
              </p>
            </div>
          ) : (
            <div className="card overflow-hidden !p-0">
              <ResponsiveTable>
                <thead>
                  <tr>
                    <th>Prefix</th>
                    <th>Type</th>
                    <th>Changes</th>
                    <th>Source</th>
                    <th>Detected</th>
                  </tr>
                </thead>
                <tbody>
                  {activeEvents.map((e) => (
                    <tr key={e.id}>
                      <td data-label="Prefix" className="text-body-sm-strong font-mono">{e.prefix}</td>
                      <td data-label="Type">
                        <span className="badge">{e.prefixType}</span>
                      </td>
                      <td data-label="Changes" className="text-body-sm-strong text-warning">
                        {String(e.changeCount)}
                      </td>
                      <td data-label="Source">
                        <span
                          className={cn(
                            'badge',
                            e.source === 'ibgp' && 'bg-link-bg-soft text-link-deep',
                            e.source === 'socket' && 'bg-cyan-soft text-cyan-deep',
                            e.source === 'probe' && 'bg-warning-soft text-warning-deep'
                          )}
                        >
                          {SOURCE_LABELS[e.source] || e.source}
                        </span>
                      </td>
                      <td data-label="Detected" className="text-body-sm text-body">
                        {relativeTime(e.detectedAt)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </ResponsiveTable>
            </div>
          )}
        </div>
      )}

      {/* History */}
      {!loading && allEvents.length > 0 && (
        <div>
          <h2 className="text-display-sm text-ink mb-md">Recent History</h2>
          <div className="card overflow-hidden !p-0">
            <ResponsiveTable>
              <thead>
                <tr>
                  <th>Prefix</th>
                  <th>Type</th>
                  <th>Changes</th>
                  <th>Source</th>
                  <th>Status</th>
                  <th>Detected</th>
                  <th>Resolved</th>
                </tr>
              </thead>
              <tbody>
                {allEvents.slice(0, 50).map((e) => (
                  <tr key={e.id}>
                    <td data-label="Prefix" className="text-body-sm-strong font-mono">{e.prefix}</td>
                    <td data-label="Type">
                      <span className="badge">{e.prefixType}</span>
                    </td>
                    <td data-label="Changes" className="text-body-sm-strong">{String(e.changeCount)}</td>
                    <td data-label="Source">
                      <span
                        className={cn(
                          'badge',
                          e.source === 'ibgp' && 'bg-link-bg-soft text-link-deep',
                          e.source === 'socket' && 'bg-cyan-soft text-cyan-deep',
                          e.source === 'probe' && 'bg-warning-soft text-warning-deep'
                        )}
                      >
                        {SOURCE_LABELS[e.source] || e.source}
                      </span>
                    </td>
                    <td data-label="Status">
                      <span
                        className={cn(
                          'badge',
                          e.active
                            ? 'bg-warning-soft text-warning-deep'
                            : 'bg-canvas-soft-2 text-mute'
                        )}
                      >
                        {e.active ? 'Active' : 'Resolved'}
                      </span>
                    </td>
                    <td data-label="Detected" className="text-body-sm text-body">
                      {relativeTime(e.detectedAt)}
                    </td>
                    <td data-label="Resolved" className="text-body-sm text-mute">
                      {e.resolvedAt ? relativeTime(e.resolvedAt) : '—'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </ResponsiveTable>
          </div>
        </div>
      )}
    </div>
  );
}
