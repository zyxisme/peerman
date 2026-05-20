import { useParams, Link } from 'react-router-dom';
import { Edit, Trash2, ArrowLeft } from 'lucide-react';
import { useNode } from '../../hooks/useNodes';
import { useProbes } from '../../hooks/useProbes';
import { usePeers } from '../../hooks/usePeers';
import { clusterClient } from '../../lib/grpc';
import { useNavigate } from 'react-router-dom';
import { cn } from '../../lib/utils';

export default function NodeDetail() {
  const { id } = useParams<{ id: string }>();
  const { node, loading, error } = useNode(id);
  const { probes } = useProbes(id);
  const { peers } = usePeers();
  const navigate = useNavigate();

  if (loading) return <div className="text-mute p-lg">Loading...</div>;
  if (error) return <div className="text-error p-lg">{error}</div>;
  if (!node) return <div className="text-mute p-lg">Node not found.</div>;

  const nodePeers = peers.filter(p => p.originNodeId === node.id);

  const handleDelete = async () => {
    if (!confirm(`Delete node "${node.name}"?`)) return;
    await clusterClient.deleteNode({ id: node.id });
    navigate('/nodes');
  };

  return (
    <div className="space-y-lg animate-fade-in">
      <Link to="/nodes" className="inline-flex items-center gap-1 text-body-sm text-mute hover:text-body no-underline">
        <ArrowLeft className="w-3.5 h-3.5" /> Back to Nodes
      </Link>

      <div className="flex items-start justify-between">
        <div>
          <h1 className="text-display-md text-ink">{node.name}</h1>
          <p className="text-body-sm text-mute mt-xxs">{node.description || 'No description'}</p>
        </div>
        <div className="flex items-center gap-sm">
          <Link to={`/nodes/${node.id}/edit`} className="btn-secondary inline-flex items-center gap-1.5 no-underline text-body-sm">
            <Edit className="w-3.5 h-3.5" /> Edit
          </Link>
          <button onClick={handleDelete} className="btn-secondary inline-flex items-center gap-1.5 text-body-sm !text-error hover:!bg-error-soft">
            <Trash2 className="w-3.5 h-3.5" /> Delete
          </button>
        </div>
      </div>

      {/* Status */}
      <div className="card">
        <h2 className="text-body-md-strong text-ink mb-md">Status</h2>
        <dl className="grid grid-cols-2 gap-md">
          <div>
            <dt className="text-caption-mono text-mute">Listen Address</dt>
            <dd className="text-body-sm"><code className="text-code">{node.listenAddr}</code></dd>
          </div>
          <div>
            <dt className="text-caption-mono text-mute">Local ASN</dt>
            <dd className="text-body-sm">AS{node.localAsn > 0n ? String(node.localAsn) : '—'}</dd>
          </div>
          <div>
            <dt className="text-caption-mono text-mute">Status</dt>
            <dd>
              <span className={cn(
                'inline-flex items-center gap-1.5 rounded-full px-xs py-0.5 text-caption font-medium',
                node.online ? 'bg-link-bg-soft text-link-deep' : 'bg-canvas-soft text-mute'
              )}>
                <span className={cn('w-1.5 h-1.5 rounded-full', node.online ? 'bg-success' : 'bg-hairline-strong')} />
                {node.online ? 'Online' : 'Offline'}
              </span>
            </dd>
          </div>
          <div>
            <dt className="text-caption-mono text-mute">Last Seen</dt>
            <dd className="text-body-sm">{node.lastSeenAt?.slice(0, 19).replace('T', ' ') ?? '—'}</dd>
          </div>
        </dl>
      </div>

      {/* Hosted Peers */}
      <div className="card">
        <h2 className="text-body-md-strong text-ink mb-md">Hosted Peers ({nodePeers.length})</h2>
        {nodePeers.length === 0 ? (
          <p className="text-body-sm text-mute">No peers hosted on this node.</p>
        ) : (
          <div className="space-y-sm">
            {nodePeers.map(p => (
              <Link key={p.id} to={`/peers/${p.id}`}
                className="flex items-center justify-between p-sm rounded-sm border border-hairline hover:bg-canvas-soft no-underline">
                <span className="text-body-sm text-link">{p.name}</span>
                <span className="text-caption-mono text-mute">AS{String(p.asn)}</span>
              </Link>
            ))}
          </div>
        )}
      </div>

      {/* Probe History */}
      <div className="card">
        <h2 className="text-body-md-strong text-ink mb-md">Recent Probes ({probes.length})</h2>
        <div className="overflow-x-auto">
          <table className="data-table w-full">
            <thead>
              <tr>
                <th>To Node</th>
                <th>Avg Latency</th>
                <th>Min</th>
                <th>Max</th>
                <th>Loss</th>
                <th>Time</th>
              </tr>
            </thead>
            <tbody>
              {probes.slice(0, 10).map(p => (
                <tr key={p.id}>
                  <td className="text-body-sm">{p.toNodeId.slice(0, 8)}...</td>
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
    </div>
  );
}
