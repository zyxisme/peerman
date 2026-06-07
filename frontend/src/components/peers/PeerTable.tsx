import { useNavigate } from 'react-router-dom';
import { Pencil, Trash2, Eye, Cable } from 'lucide-react';
import { usePeers } from '../../hooks/usePeers';
import { useNodes } from '../../hooks/useNodes';
import { peerClient } from '../../lib/grpc';
import { ResponsiveTable } from '../ui/ResponsiveTable';

export default function PeerTable() {
  const { peers, loading, error, refetch } = usePeers();
  const { nodes } = useNodes();
  const navigate = useNavigate();

  const offlineNodeIds = new Set(nodes.filter(n => !n.online).map(n => n.id));
  const nodeNameById = new Map(nodes.map(n => [n.id, n.name]));

  const nodeName = (id: string) => nodeNameById.get(id) ?? id.slice(0, 8);

  const handleToggle = async (id: string) => {
    await peerClient.togglePeer({ id });
    refetch();
  };

  const handleDelete = async (id: string, name: string) => {
    if (!confirm(`Delete peer "${name}"?`)) return;
    await peerClient.deletePeer({ id });
    refetch();
  };

  if (loading) {
    return <div className="p-xl text-body text-center">Loading peers...</div>;
  }

  if (error) {
    return <div className="p-xl text-error text-center">{error}</div>;
  }

  if (peers.length === 0) {
    return (
      <div className="card-soft text-center py-4xl px-xl">
        <Cable className="w-12 h-12 mx-auto mb-md text-mute" />
        <h2 className="text-display-sm text-ink mb-sm">No peers yet</h2>
        <p className="text-body-sm text-body mb-lg">
          Add your first DN42 peer to start managing WireGuard tunnels and BGP sessions.
        </p>
        <button className="btn-primary" onClick={() => navigate('/peers/new')}>
          Add Peer
        </button>
      </div>
    );
  }

  return (
    <div className="card overflow-hidden">
      <div className="flex items-center justify-between px-lg py-md border-b border-hairline">
        <h2 className="text-display-sm text-ink">Peers</h2>
        <button className="btn-primary-sm" onClick={() => navigate('/peers/new')}>
          Add Peer
        </button>
      </div>
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
    </div>
  );
}
