import { useState, useEffect, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { create } from '@bufbuild/protobuf';
import { SettingsSchema } from '../../lib/peerman_pb';
import { useSettings } from '../../hooks/useSettings';

export default function SettingsPage() {
  const navigate = useNavigate();
  const { settings, loading, saveSettings } = useSettings();
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [saved, setSaved] = useState(false);

  const [form, setForm] = useState({
    localAsn: '4242420000',
    birdTemplateName: 'dnpeers',
    birdRouterId: '172.20.0.1',
    wgDefaultListenPort: '42420',
    dn42Ipv4Prefix: '172.20.0.0/14',
    dn42Ipv6Prefix: 'fd00::/8',
    wgTable: 'off',
  });

  useEffect(() => {
    if (settings) {
      setForm({
        localAsn: settings.localAsn.toString(),
        birdTemplateName: settings.birdTemplateName,
        birdRouterId: settings.birdRouterId,
        wgDefaultListenPort: String(settings.wgDefaultListenPort),
        dn42Ipv4Prefix: settings.dn42Ipv4Prefix,
        dn42Ipv6Prefix: settings.dn42Ipv6Prefix,
        wgTable: settings.wgTable,
      });
    }
  }, [settings]);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setError('');
    setSaved(false);

    const s = create(SettingsSchema, {
      localAsn: BigInt(form.localAsn || '0'),
      birdTemplateName: form.birdTemplateName,
      birdRouterId: form.birdRouterId,
      wgDefaultListenPort: Number(form.wgDefaultListenPort || '0'),
      dn42Ipv4Prefix: form.dn42Ipv4Prefix,
      dn42Ipv6Prefix: form.dn42Ipv6Prefix,
      wgTable: form.wgTable,
    });

    try {
      await saveSettings(s);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  if (loading) return <div className="p-xl text-body">Loading settings...</div>;

  const f = <K extends keyof typeof form>(k: K) => form[k];

  return (
    <div className="max-w-2xl mx-auto animate-fade-in">
      <div className="flex items-center gap-md mb-lg">
        <button onClick={() => navigate(-1)} className="btn-ghost">
          <ArrowLeft className="w-4 h-4" />
        </button>
        <h1 className="text-display-md text-ink">Settings</h1>
      </div>

      {error && (
        <div className="bg-error-soft text-error-deep text-body-sm px-md py-sm rounded-sm mb-lg">{error}</div>
      )}
      {saved && (
        <div className="bg-cyan-soft text-cyan-deep text-body-sm px-md py-sm rounded-sm mb-lg">Settings saved.</div>
      )}

      <form onSubmit={handleSubmit} className="card space-y-md">
        <h2 className="text-body-sm-strong text-ink">Global Configuration</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
          <Input label="Local ASN" value={f('localAsn')} onChange={(v) => setForm((p) => ({ ...p, localAsn: v }))} />
          <Input label="BIRD Template Name" value={f('birdTemplateName')} onChange={(v) => setForm((p) => ({ ...p, birdTemplateName: v }))} />
          <Input label="BIRD Router ID" value={f('birdRouterId')} onChange={(v) => setForm((p) => ({ ...p, birdRouterId: v }))} />
          <Input label="Default WG Listen Port" value={f('wgDefaultListenPort')} onChange={(v) => setForm((p) => ({ ...p, wgDefaultListenPort: v }))} type="number" />
          <Input label="DN42 IPv4 Prefix" value={f('dn42Ipv4Prefix')} onChange={(v) => setForm((p) => ({ ...p, dn42Ipv4Prefix: v }))} />
          <Input label="DN42 IPv6 Prefix" value={f('dn42Ipv6Prefix')} onChange={(v) => setForm((p) => ({ ...p, dn42Ipv6Prefix: v }))} />
          <Input label="WG Table" value={f('wgTable')} onChange={(v) => setForm((p) => ({ ...p, wgTable: v }))} />
        </div>
        <button type="submit" disabled={saving} className="btn-primary">
          {saving ? 'Saving...' : 'Save Settings'}
        </button>
      </form>
    </div>
  );
}

function Input({
  label, value, onChange, type = 'text',
}: {
  label: string; value: string; onChange: (v: string) => void; type?: string;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-caption text-mute">{label}</label>
      <input
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="form-input"
      />
    </div>
  );
}
