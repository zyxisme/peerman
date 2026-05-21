import { RefreshCw } from 'lucide-react';
import { useWireGuardStatus, useBirdStatus } from '../../hooks/useManagement';

export default function StatusPage() {
  const wg = useWireGuardStatus();
  const bird = useBirdStatus();

  return (
    <div className="space-y-lg animate-fade-in">
      <div className="flex items-center justify-between">
        <h1 className="text-display-md text-ink">System Status</h1>
        <button
          onClick={() => { wg.refetch(); bird.refetch(); }}
          className="btn-ghost text-xs flex items-center gap-1"
        >
          <RefreshCw className="w-3 h-3" />
          Refresh
        </button>
      </div>

      {/* WireGuard */}
      <div className="card">
        <h2 className="text-body-md-strong text-ink mb-md">WireGuard</h2>
        {wg.loading && <div className="text-body-sm text-body">Loading...</div>}
        {wg.error && <div className="text-body-sm text-error">{wg.error}</div>}
        {!wg.loading && !wg.error && wg.interfaces.length === 0 && (
          <div className="text-body-sm text-body">No WireGuard interfaces found</div>
        )}
        {wg.interfaces.map((iface) => (
          <div key={iface.name} className="space-y-sm">
            <div className="text-body-sm text-body">
              {iface.name} — pubkey: <code className="code-block text-xs">{iface.publicKey.substring(0, 12)}...</code>, port: {iface.listenPort}
            </div>
            {iface.peers.length === 0 && (
              <div className="text-caption text-mute ml-md">No peers</div>
            )}
            {iface.peers.map((peer, i) => (
              <div key={i} className="card-soft text-caption">
                <div className="grid grid-cols-2 md:grid-cols-3 gap-xxs">
                  <div><span className="text-mute">Peer:</span> <code>{peer.publicKey.substring(0, 10)}...</code></div>
                  <div><span className="text-mute">Endpoint:</span> {peer.endpoint || '—'}</div>
                  <div><span className="text-mute">Handshake:</span> {peer.latestHandshake || '—'}</div>
                  <div><span className="text-mute">RX:</span> {peer.transferRx || '—'}</div>
                  <div><span className="text-mute">TX:</span> {peer.transferTx || '—'}</div>
                </div>
              </div>
            ))}
          </div>
        ))}
      </div>

      {/* BIRD */}
      <div className="card">
        <h2 className="text-body-md-strong text-ink mb-md">BIRD</h2>
        {bird.loading && <div className="text-body-sm text-body">Loading...</div>}
        {bird.error && <div className="text-body-sm text-error">{bird.error}</div>}
        {!bird.loading && !bird.error && bird.protocols.length === 0 && (
          <div className="text-body-sm text-body">No BIRD protocols found</div>
        )}
        {!bird.loading && !bird.error && bird.protocols.length > 0 && (
          <div className="data-table">
            <table>
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Proto</th>
                  <th>Table</th>
                  <th>State</th>
                  <th>Since</th>
                  <th>Info</th>
                </tr>
              </thead>
              <tbody>
                {bird.protocols.map((p) => (
                  <tr key={p.name}>
                    <td className="font-mono text-caption-mono">{p.name}</td>
                    <td>{p.proto}</td>
                    <td>{p.table}</td>
                    <td>
                      <span className={`badge ${p.state === 'up' ? 'bg-green-500/20 text-green-500' : 'bg-red-500/20 text-red-500'}`}>
                        {p.state}
                      </span>
                    </td>
                    <td className="text-mute">{p.since}</td>
                    <td className="text-mute text-xs">{p.info}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
