import { useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { ArrowLeft, Pencil, Trash2 } from 'lucide-react';
import { usePeer, useWireGuardConfig, useBirdConfig } from '../../hooks/usePeers';
import { peerClient } from '../../lib/grpc';
import ConfigViewer from './ConfigViewer';

export default function PeerDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { peer, loading, error } = usePeer(id);
  const [tab, setTab] = useState<'info' | 'wg' | 'bird'>('info');

  const wg = useWireGuardConfig(tab === 'wg' ? id : undefined);
  const bird = useBirdConfig(tab === 'bird' ? id : undefined);

  const handleDelete = async () => {
    if (!peer || !confirm(`Delete peer "${peer.name}"?`)) return;
    await peerClient.deletePeer({ id: peer.id });
    navigate('/');
  };

  if (loading) return <div className="p-xl text-body">Loading...</div>;
  if (error) return <div className="p-xl text-error">{error}</div>;
  if (!peer) return <div className="p-xl text-body">Peer not found</div>;

  const asnStr = peer.asn.toString();
  const localAsnStr = peer.localAsn.toString();

  return (
    <div className="space-y-lg animate-fade-in">
      {/* Header */}
      <div className="flex items-center gap-md">
        <button onClick={() => navigate('/')} className="btn-ghost">
          <ArrowLeft className="w-4 h-4" />
        </button>
        <div className="flex-1">
          <h1 className="text-display-md text-ink">{peer.name}</h1>
          {peer.description && (
            <p className="text-body-sm text-body">{peer.description}</p>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => navigate(`/peers/${peer.id}/edit`)}
            className="btn-secondary-sm"
          >
            <Pencil className="w-3.5 h-3.5" />
            Edit
          </button>
          <button onClick={handleDelete} className="btn-secondary-sm text-error border-error/30 hover:bg-error-soft">
            <Trash2 className="w-3.5 h-3.5" />
            Delete
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex items-center gap-1">
        {(['info', 'wg', 'bird'] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={tab === t ? 'tab-active' : 'tab-ghost'}
          >
            {t === 'info' ? 'Info' : t === 'wg' ? 'WireGuard Config' : 'BIRD Config'}
          </button>
        ))}
      </div>

      {/* Content */}
      {tab === 'info' && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-lg">
          <Section title="Identity">
            <Field label="Name" value={peer.name} />
            <Field label="ASN" value={`AS${asnStr}`} mono />
            <Field label="Local ASN" value={`AS${localAsnStr}`} mono />
            <Field label="Enabled" value={peer.enabled ? 'Yes' : 'No'} />
          </Section>

          <Section title="WireGuard">
            <Field label="Public Key" value={peer.wgPublicKey || '—'} mono />
            <Field label="Remote Address" value={peer.wgRemoteAddress} mono />
            <Field label="Remote Port" value={String(peer.wgRemotePort)} mono />
            <Field label="Listen Port" value={String(peer.wgListenPort)} mono />
            <Field label="Interface" value={peer.wgInterfaceName} mono />
          </Section>

          <Section title="Tunnel Addressing">
            <Field label="IPv4 Local" value={peer.ipv4TunnelLocal || '—'} mono />
            <Field label="IPv4 Remote" value={peer.ipv4TunnelRemote || '—'} mono />
            <Field label="IPv6 Local" value={peer.ipv6TunnelLocal || '—'} mono />
            <Field label="IPv6 Remote" value={peer.ipv6TunnelRemote || '—'} mono />
          </Section>

          <Section title="BGP Session">
            <Field label="Multiprotocol" value={peer.multiprotocol ? 'Yes' : 'No'} />
            <Field label="Extended Nexthop" value={peer.extendedNexthop ? 'Yes' : 'No'} />
            <Field label="Sessions" value={peer.sessions === 0 ? 'IPv4' : peer.sessions === 1 ? 'IPv6' : 'Both'} />
            <Field label="Passive" value={peer.passive ? 'Yes' : 'No'} />
            <Field label="Max Prefix (import)" value={String(peer.importMaxPrefix || '—')} mono />
            <Field label="Max Prefix (export)" value={String(peer.exportMaxPrefix || '—')} mono />
          </Section>
        </div>
      )}

      {tab === 'wg' && (
        <div className="card">
          <ConfigViewer content={wg.content} loading={wg.loading} filename={`${peer.name}-wg.conf`} title="WireGuard Configuration" />
        </div>
      )}

      {tab === 'bird' && (
        <div className="card">
          <ConfigViewer content={bird.content} loading={bird.loading} filename={`${peer.name}-bird.conf`} title="BIRD2 Configuration" />
        </div>
      )}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="card-soft">
      <h3 className="text-body-sm-strong text-ink mb-sm">{title}</h3>
      <dl className="space-y-xs">{children}</dl>
    </div>
  );
}

function Field({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex items-center justify-between py-xxs">
      <dt className="text-caption text-mute">{label}</dt>
      <dd className={`text-body-sm text-ink ${mono ? 'font-mono text-caption-mono' : ''}`}>
        {value}
      </dd>
    </div>
  );
}
