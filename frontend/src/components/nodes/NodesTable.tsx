import { useState, useCallback } from 'react';
import { Link } from 'react-router-dom';
import { Plus, Server, Trash2, RefreshCw } from 'lucide-react';
import { useNodes } from '../../hooks/useNodes';
import { useRunProbe } from '../../hooks/useProbes';
import { clusterClient } from '../../lib/grpc';
import { cn } from '../../lib/utils';
import { ResponsiveTable } from '../ui/ResponsiveTable';

export default function NodesTable() {
  const { nodes, loading, error, refetch } = useNodes();
  const { run, loading: probeLoading } = useRunProbe();
  const [probeMsg, setProbeMsg] = useState('');

  const handleDelete = useCallback(async (id: string) => {
    if (!confirm('Delete this node?')) return;
    try {
      await clusterClient.deleteNode({ id });
      refetch();
    } catch (e) {
      console.error(e);
    }
  }, [refetch]);

  const handleProbe = useCallback(async (fromId: string, toId: string) => {
    setProbeMsg('');
    try {
      const r = await run(fromId, toId);
      if (r) {
        setProbeMsg(`${r.avgLatencyMs.toFixed(1)}ms avg, ${r.packetLossPct.toFixed(1)}% loss`);
      }
      refetch();
    } catch (e) {
      setProbeMsg(String(e));
    }
  }, [run, refetch]);

  if (loading) return <div className="text-mute text-body-sm p-lg">Loading nodes...</div>;
  if (error) return <div className="text-error text-body-sm p-lg">{error}</div>;

  return (
    <div className="space-y-lg">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-display-md text-ink">Cluster Nodes</h1>
          <p className="text-body-sm text-mute mt-xxs">
            {nodes.length} node{nodes.length !== 1 ? 's' : ''} in the cluster
          </p>
        </div>
        <Link to="/nodes/new" className="btn-primary inline-flex items-center gap-1.5 no-underline">
          <Plus className="w-4 h-4" />
          Register Node
        </Link>
      </div>

      {probeMsg && (
        <div className="card border border-hairline text-body-sm">{probeMsg}</div>
      )}

      <div className="card overflow-hidden !p-0">
        <ResponsiveTable>
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
            {nodes.length === 0 && (
              <tr>
                <td colSpan={6} className="text-center text-mute py-4xl">
                  <Server className="w-8 h-8 mx-auto mb-sm opacity-30" />
                  No nodes registered yet. Start by setting <code className="text-code">node_name</code> in the <code className="text-code">[cluster]</code> section of your config.toml.
                </td>
              </tr>
            )}
          </tbody>
        </ResponsiveTable>
      </div>
    </div>
  );
}
