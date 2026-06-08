import { useState, useEffect, type FormEvent } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { create } from '@bufbuild/protobuf';
import { CreatePeerRequestSchema, UpdatePeerRequestSchema } from '../../lib/peerman_pb';
import { peerClient } from '../../lib/grpc';
import { usePeer } from '../../hooks/usePeers';
import { useNodes } from '../../hooks/useNodes';
import { useSettings } from '../../hooks/useSettings';
import { Input } from '../ui/Input';
import { Toggle } from '../ui/Toggle';

export default function PeerForm() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const isEdit = Boolean(id);
  const { peer: existingPeer, loading: loadingPeer } = usePeer(id);
  const { nodes } = useNodes();
  const { settings } = useSettings();
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');

  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [asn, setAsn] = useState('0');
  const [localAsn, setLocalAsn] = useState('4242420000');
  const [originNodeId, setOriginNodeId] = useState('');
  const [wgPublicKey, setWgPublicKey] = useState('');
  const [wgRemoteAddress, setWgRemoteAddress] = useState('');
  const [wgRemotePort, setWgRemotePort] = useState('0');
  const [wgListenPort, setWgListenPort] = useState('42420');
  const [wgInterfaceName, setWgInterfaceName] = useState('');
  const [ipv4TunnelLocal, setIpv4TunnelLocal] = useState('');
  const [ipv4TunnelRemote, setIpv4TunnelRemote] = useState('');
  const [ipv6TunnelLocal, setIpv6TunnelLocal] = useState('');
  const [ipv6TunnelRemote, setIpv6TunnelRemote] = useState('');
  const [multiprotocol, setMultiprotocol] = useState(true);
  const [extendedNexthop, setExtendedNexthop] = useState(true);
  const [sessions, setSessions] = useState(2);
  const [passive, setPassive] = useState(false);
  const [importMaxPrefix, setImportMaxPrefix] = useState('0');
  const [exportMaxPrefix, setExportMaxPrefix] = useState('0');

  useEffect(() => {
    if (existingPeer) {
      setName(existingPeer.name);
      setDescription(existingPeer.description);
      setAsn(existingPeer.asn.toString());
      setLocalAsn(existingPeer.localAsn.toString());
      setWgPublicKey(existingPeer.wgPublicKey);
      setWgRemoteAddress(existingPeer.wgRemoteAddress);
      setWgRemotePort(String(existingPeer.wgRemotePort));
      setWgListenPort(String(existingPeer.wgListenPort));
      setWgInterfaceName(existingPeer.wgInterfaceName);
      setIpv4TunnelLocal(existingPeer.ipv4TunnelLocal);
      setIpv4TunnelRemote(existingPeer.ipv4TunnelRemote);
      setIpv6TunnelLocal(existingPeer.ipv6TunnelLocal);
      setIpv6TunnelRemote(existingPeer.ipv6TunnelRemote);
      setMultiprotocol(existingPeer.multiprotocol);
      setExtendedNexthop(existingPeer.extendedNexthop);
      setSessions(existingPeer.sessions);
      setPassive(existingPeer.passive);
      setImportMaxPrefix(String(existingPeer.importMaxPrefix || '0'));
      setExportMaxPrefix(String(existingPeer.exportMaxPrefix || '0'));
      setOriginNodeId(existingPeer.originNodeId);
    }
  }, [existingPeer]);

  useEffect(() => {
    if (!isEdit && settings) {
      setWgListenPort('0'); // will be auto-filled by backend
      setLocalAsn(settings.localAsn.toString());
    }
  }, [isEdit, settings]);

  useEffect(() => {
    if (!isEdit && asn && asn !== '0') {
      setWgRemotePort(String(listenPortFromAsn(asn)));
    }
  }, [isEdit, asn]);

  useEffect(() => {
    if (!isEdit && name) {
      setWgInterfaceName(sanitizeInterfaceName(name));
    }
  }, [isEdit, name]);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setError('');

    const base = {
      name,
      description,
      asn: BigInt(asn || '0'),
      localAsn: BigInt(localAsn || '0'),
      wgPrivateKey: '',
      wgPublicKey,
      wgRemoteAddress,
      wgRemotePort: Number(wgRemotePort || '0'),
      wgListenPort: Number(wgListenPort || '0'),
      wgInterfaceName,
      ipv4TunnelLocal,
      ipv4TunnelRemote,
      ipv6TunnelLocal,
      ipv6TunnelRemote,
      multiprotocol,
      extendedNexthop,
      sessions,
      passive,
      importMaxPrefix: Number(importMaxPrefix || '0'),
      exportMaxPrefix: Number(exportMaxPrefix || '0'),
      originNodeId,
    };

    try {
      if (isEdit && id) {
        await peerClient.updatePeer(create(UpdatePeerRequestSchema, { id, ...base }));
      } else {
        await peerClient.createPeer(create(CreatePeerRequestSchema, base));
      }
      navigate('/');
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  if (isEdit && loadingPeer) {
    return <div className="p-xl text-body">Loading...</div>;
  }

  return (
    <div className="max-w-2xl mx-auto animate-fade-in">
      <div className="flex items-center gap-md mb-lg">
        <button onClick={() => navigate(-1)} className="btn-ghost">
          <ArrowLeft className="w-4 h-4" />
        </button>
        <h1 className="text-display-md text-ink">
          {isEdit ? `Edit ${name}` : 'New Peer'}
        </h1>
      </div>

      {error && (
        <div className="bg-error-soft text-error-deep text-body-sm px-md py-sm rounded-sm mb-lg">{error}</div>
      )}

      <form onSubmit={handleSubmit} className="space-y-lg">
        {/* Identity */}
        <fieldset className="card">
          <legend className="text-body-sm-strong text-ink mb-md">Identity</legend>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
            <div className="flex flex-col gap-1">
              <label className="text-caption text-mute">Target Node</label>
              <select value={originNodeId} onChange={(e) => setOriginNodeId(e.target.value)} className="form-input">
                <option value="">This node (local)</option>
                {nodes.filter(n => n.online).map(n => (
                  <option key={n.id} value={n.id}>{n.name} ({n.listenAddr})</option>
                ))}
              </select>
            </div>
            <div>
              <Input label="Name" value={name} onChange={setName} required />
              {name && <p className="text-caption text-mute mt-1">Interface: {sanitizeInterfaceName(name)}</p>}
            </div>
            <Input label="Description" value={description} onChange={setDescription} />
            <div>
              <Input label="Remote ASN" value={asn} onChange={setAsn} placeholder="424242XXXX" />
              {asn && asn !== '0' && (
                <p className="text-caption text-mute mt-1">
                  Remote link-local: {linkLocalFromAsn(asn)} | Default port: {listenPortFromAsn(asn)}
                </p>
              )}
            </div>
          </div>
          <details className="mt-sm">
            <summary className="text-caption text-mute cursor-pointer select-none">Advanced Identity</summary>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-sm mt-sm">
              <Input label="Local ASN" value={localAsn} onChange={setLocalAsn} />
            </div>
          </details>
        </fieldset>

        {/* WireGuard */}
        <fieldset className="card">
          <legend className="text-body-sm-strong text-ink mb-md">WireGuard</legend>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
            <Input label="Peer Public Key" value={wgPublicKey} onChange={setWgPublicKey} mono required />
            <Input label="Remote Address" value={wgRemoteAddress} onChange={setWgRemoteAddress} required />
            <Input label="Remote Port" value={wgRemotePort} onChange={setWgRemotePort} type="number" />
          </div>
          <details className="mt-sm">
            <summary className="text-caption text-mute cursor-pointer select-none">Advanced WireGuard</summary>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-sm mt-sm">
              <Input label="Listen Port" value={wgListenPort} onChange={setWgListenPort} type="number" placeholder="Auto from ASN" />
              <Input label="Interface Name" value={wgInterfaceName} onChange={setWgInterfaceName} placeholder={name ? sanitizeInterfaceName(name) : 'wg-peer-name'} />
            </div>
          </details>
        </fieldset>

        {/* Tunnel & BGP */}
        <fieldset className="card">
          <legend className="text-body-sm-strong text-ink mb-md">Tunnel & BGP</legend>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
            <Toggle label="Multiprotocol" checked={multiprotocol} onChange={setMultiprotocol} />
            <Toggle label="Extended Nexthop" checked={extendedNexthop} onChange={setExtendedNexthop} />
            <Toggle label="Passive" checked={passive} onChange={setPassive} />
            <div className="flex flex-col gap-1">
              <label className="text-caption text-mute">Sessions</label>
              <select value={sessions} onChange={(e) => setSessions(Number(e.target.value))} className="form-input">
                <option value={2}>Both</option>
                <option value={0}>IPv4</option>
                <option value={1}>IPv6</option>
              </select>
            </div>
          </div>
          <details className="mt-sm">
            <summary className="text-caption text-mute cursor-pointer select-none">Advanced Tunnel</summary>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-sm mt-sm">
              <Input label="IPv4 Local" value={ipv4TunnelLocal} onChange={setIpv4TunnelLocal} />
              <Input label="IPv4 Remote" value={ipv4TunnelRemote} onChange={setIpv4TunnelRemote} />
              <Input label="IPv6 Local (link-local)" value={ipv6TunnelLocal} onChange={setIpv6TunnelLocal} placeholder="fe80::1" />
              <Input label="IPv6 Remote (link-local)" value={ipv6TunnelRemote} onChange={setIpv6TunnelRemote} placeholder="fe80::2" />
              <Input label="Max Prefix (import)" value={importMaxPrefix} onChange={setImportMaxPrefix} type="number" />
              <Input label="Max Prefix (export)" value={exportMaxPrefix} onChange={setExportMaxPrefix} type="number" />
            </div>
          </details>
        </fieldset>

        <div className="flex items-center gap-2">
          <button type="submit" disabled={saving} className="btn-primary">
            {saving ? 'Saving...' : isEdit ? 'Update Peer' : 'Create Peer'}
          </button>
          <button type="button" onClick={() => navigate(-1)} className="btn-secondary">
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}

function sanitizeInterfaceName(name: string): string {
  return 'wg-' + name
    .toLowerCase()
    .replace(/[^a-z0-9]/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
    .substring(0, 15);
}

function listenPortFromAsn(asn: string): number {
  return 20000 + (Number(asn) % 10000);
}

function linkLocalFromAsn(asn: string): string {
  const last4 = Number(asn) % 10000;
  return `fe80::${last4.toString(16)}`;
}
