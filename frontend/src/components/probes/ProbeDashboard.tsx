import { useState } from 'react';
import { RefreshCw } from 'lucide-react';
import { useNodes } from '../../hooks/useNodes';
import { useProbes, useRunProbe } from '../../hooks/useProbes';
import { cn } from '../../lib/utils';

function latencyColor(ms: number): string {
  if (ms <= 0) return 'bg-canvas-soft';
  if (ms < 5) return 'bg-cyan-soft text-cyan-deep';
  if (ms < 20) return 'bg-success/20 text-success';
  if (ms < 50) return 'bg-warning-soft text-warning-deep';
  if (ms < 150) return 'bg-warning/40 text-warning-deep';
  return 'bg-error-soft text-error-deep';
}

export default function ProbeDashboard() {
  const { nodes } = useNodes();
  const { probes, refetch } = useProbes();
  const { run, loading: probeRunning } = useRunProbe();
  const [msg, setMsg] = useState('');

  // Build latency matrix: node_id -> node_id -> latest probe
  const matrix = new Map<string, Map<string, number>>();
  for (const p of probes) {
    if (!matrix.has(p.fromNodeId)) matrix.set(p.fromNodeId, new Map());
    const inner = matrix.get(p.fromNodeId)!;
    if (!inner.has(p.toNodeId)) {
      inner.set(p.toNodeId, p.avgLatencyMs);
    }
  }

  const handleProbeAll = async () => {
    if (nodes.length < 2) {
      setMsg('Need at least 2 nodes to probe.');
      return;
    }
    setMsg('Running probes...');
    const from = nodes[0];
    for (const to of nodes) {
      if (to.id === from.id) continue;
      try {
        await run(from.id, to.id);
      } catch {
        // skip failures
      }
    }
    setMsg('Probes complete.');
    refetch();
  };

  return (
    <div className="space-y-lg animate-fade-in">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-display-md text-ink">Network Probe Dashboard</h1>
          <p className="text-body-sm text-mute mt-xxs">Latency matrix between cluster nodes</p>
        </div>
        <button onClick={handleProbeAll} disabled={probeRunning || nodes.length < 2}
          className="btn-primary inline-flex items-center gap-1.5">
          <RefreshCw className={cn('w-4 h-4', probeRunning && 'animate-spin')} />
          Probe All
        </button>
      </div>

      {msg && <div className="card border border-hairline text-body-sm">{msg}</div>}

      {/* Latency Matrix */}
      <div className="card overflow-x-auto !p-0">
        <table className="w-full text-center">
          <thead>
            <tr>
              <th className="text-caption-mono text-mute uppercase p-md text-left bg-canvas-soft">From ↓ / To →</th>
              {nodes.map(n => (
                <th key={n.id} className="text-caption-mono text-mute p-md bg-canvas-soft font-medium max-w-[120px] truncate" title={n.name}>
                  {n.name}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {nodes.map(from => (
              <tr key={from.id} className="border-t border-hairline">
                <td className="text-body-sm font-medium p-md text-left bg-canvas-soft text-ink">{from.name}</td>
                {nodes.map(to => {
                  const ms = matrix.get(from.id)?.get(to.id);
                  const isSelf = from.id === to.id;
                  return (
                    <td key={to.id} className={cn('p-md text-body-sm font-mono', isSelf ? 'text-mute bg-canvas-soft' : '')}>
                      {isSelf ? '—' : ms !== undefined ? (
                        <span className={cn('rounded-sm px-xs py-0.5', latencyColor(ms))}>
                          {ms.toFixed(1)}ms
                        </span>
                      ) : (
                        <span className="text-mute">—</span>
                      )}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Recent Probes Table */}
      <div className="card overflow-hidden !p-0">
        <table className="data-table w-full">
          <thead>
            <tr>
              <th>From</th>
              <th>To</th>
              <th>Avg</th>
              <th>Min</th>
              <th>Max</th>
              <th>Loss</th>
              <th>Time</th>
            </tr>
          </thead>
          <tbody>
            {probes.slice(0, 50).map(p => (
              <tr key={p.id}>
                <td className="text-body-sm">{nodes.find(n => n.id === p.fromNodeId)?.name ?? p.fromNodeId.slice(0, 8)}</td>
                <td className="text-body-sm">{nodes.find(n => n.id === p.toNodeId)?.name ?? p.toNodeId.slice(0, 8)}</td>
                <td className={cn('text-body-sm font-medium', p.avgLatencyMs < 5 ? 'text-success' : p.avgLatencyMs < 50 ? 'text-warning-deep' : 'text-error')}>
                  {p.avgLatencyMs.toFixed(1)}ms
                </td>
                <td className="text-body-sm text-mute">{p.minLatencyMs.toFixed(1)}ms</td>
                <td className="text-body-sm text-mute">{p.maxLatencyMs.toFixed(1)}ms</td>
                <td className={cn('text-body-sm', p.packetLossPct > 0 ? 'text-error' : 'text-mute')}>
                  {p.packetLossPct.toFixed(1)}%
                </td>
                <td className="text-caption-mono text-mute">{p.probedAt?.slice(11, 19) ?? '—'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
