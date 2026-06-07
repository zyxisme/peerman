import { useState, useEffect, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { create } from '@bufbuild/protobuf';
import { SettingsSchema } from '../../lib/peerman_pb';
import { useSettings } from '../../hooks/useSettings';

const DEFAULT_FORM = {
  localAsn: '4242420000',
  birdTemplateName: 'dnpeers',
  birdRouterId: '172.20.0.1',
  wgDefaultListenPort: '42420',
  dn42Ipv4Prefix: '172.20.0.0/14',
  dn42Ipv6Prefix: 'fd00::/8',
  wgTable: 'off',
  wgMtu: '1420',
  wgFwmark: '0',
  wgPostUp: '',
  wgPostDown: '',
  roaMode: 'none',
  roaStaticV4Url: '',
  roaStaticV6Url: '',
  roaRtrAddress: '',
  roaRtrPort: '323',
  birdImportLimit: '9000',
  birdExportFilter: '',
  birdImportFilter: '',
  enableCommunityFilters: false,
  enableBfd: false,
  bfdIntervalMs: '300',
  bfdMultiplier: '3',
};

export default function SettingsPage() {
  const navigate = useNavigate();
  const { settings, loading, saveSettings } = useSettings();
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [saved, setSaved] = useState(false);
  const [form, setForm] = useState(DEFAULT_FORM);

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
        wgMtu: String(settings.wgMtu),
        wgFwmark: String(settings.wgFwmark),
        wgPostUp: settings.wgPostUp,
        wgPostDown: settings.wgPostDown,
        roaMode: settings.roaMode || 'none',
        roaStaticV4Url: settings.roaStaticV4Url,
        roaStaticV6Url: settings.roaStaticV6Url,
        roaRtrAddress: settings.roaRtrAddress,
        roaRtrPort: String(settings.roaRtrPort || '323'),
        birdImportLimit: String(settings.birdImportLimit || '9000'),
        birdExportFilter: settings.birdExportFilter,
        birdImportFilter: settings.birdImportFilter,
        enableCommunityFilters: settings.enableCommunityFilters,
        enableBfd: settings.enableBfd,
        bfdIntervalMs: String(settings.bfdIntervalMs || '300'),
        bfdMultiplier: String(settings.bfdMultiplier || '3'),
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
      wgMtu: Number(form.wgMtu || '0'),
      wgFwmark: Number(form.wgFwmark || '0'),
      wgPostUp: form.wgPostUp,
      wgPostDown: form.wgPostDown,
      roaMode: form.roaMode,
      roaStaticV4Url: form.roaStaticV4Url,
      roaStaticV6Url: form.roaStaticV6Url,
      roaRtrAddress: form.roaRtrAddress,
      roaRtrPort: Number(form.roaRtrPort || '0'),
      birdImportLimit: Number(form.birdImportLimit || '0'),
      birdExportFilter: form.birdExportFilter,
      birdImportFilter: form.birdImportFilter,
      enableCommunityFilters: form.enableCommunityFilters,
      enableBfd: form.enableBfd,
      bfdIntervalMs: Number(form.bfdIntervalMs || '300'),
      bfdMultiplier: Number(form.bfdMultiplier || '3'),
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

      <form onSubmit={handleSubmit} className="space-y-lg">
        {/* Global Configuration */}
        <div className="card space-y-md">
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
        </div>

        {/* WireGuard Advanced */}
        <div className="card space-y-md">
          <h2 className="text-body-sm-strong text-ink">WireGuard Advanced</h2>
          <p className="text-body-sm text-mute">Tunnel-level settings applied to each peer config.</p>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
            <Input label="MTU" value={f('wgMtu')} onChange={(v) => setForm((p) => ({ ...p, wgMtu: v }))} type="number" />
            <Input label="FwMark (0 = disabled)" value={f('wgFwmark')} onChange={(v) => setForm((p) => ({ ...p, wgFwmark: v }))} type="number" />
          </div>
          <div className="grid grid-cols-1 gap-sm">
            <Textarea label="PostUp Script" value={f('wgPostUp')} onChange={(v) => setForm((p) => ({ ...p, wgPostUp: v }))} placeholder="Additional commands after interface up" />
            <Textarea label="PostDown Script" value={f('wgPostDown')} onChange={(v) => setForm((p) => ({ ...p, wgPostDown: v }))} placeholder="Additional commands before interface down" />
          </div>
        </div>

        {/* ROA/RPKI */}
        <div className="card space-y-md">
          <h2 className="text-body-sm-strong text-ink">ROA / RPKI Filtering</h2>
          <p className="text-body-sm text-mute">Reject routes with invalid or unknown ROA status.</p>
          <div className="flex items-center gap-sm">
            {(['none', 'static_file', 'rtr'] as const).map((mode) => (
              <button
                key={mode}
                type="button"
                onClick={() => setForm((p) => ({ ...p, roaMode: mode }))}
                className={f('roaMode') === mode ? 'tab-active' : 'tab-ghost'}
              >
                {mode === 'none' ? 'None' : mode === 'static_file' ? 'Static File' : 'RTR'}
              </button>
            ))}
          </div>
          {f('roaMode') === 'static_file' && (
            <div className="grid grid-cols-1 gap-sm">
              <Input label="ROA v4 URL" value={f('roaStaticV4Url')} onChange={(v) => setForm((p) => ({ ...p, roaStaticV4Url: v }))} placeholder="https://dn42.burble.com/roa/dn42_roa_bird2_4.conf" />
              <Input label="ROA v6 URL" value={f('roaStaticV6Url')} onChange={(v) => setForm((p) => ({ ...p, roaStaticV6Url: v }))} placeholder="https://dn42.burble.com/roa/dn42_roa_bird2_6.conf" />
            </div>
          )}
          {f('roaMode') === 'rtr' && (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
              <Input label="RTR Address" value={f('roaRtrAddress')} onChange={(v) => setForm((p) => ({ ...p, roaRtrAddress: v }))} placeholder="rpki.akae.re" />
              <Input label="RTR Port" value={f('roaRtrPort')} onChange={(v) => setForm((p) => ({ ...p, roaRtrPort: v }))} type="number" />
            </div>
          )}
        </div>

        {/* BIRD Filter */}
        <div className="card space-y-md">
          <h2 className="text-body-sm-strong text-ink">BIRD Filter Templates</h2>
          <p className="text-body-sm text-mute">Custom filter bodies override auto-generated defaults. Leave empty for DN42 best-practice defaults.</p>
          <div className="grid grid-cols-1 gap-sm">
            <Input label="Import Prefix Limit" value={f('birdImportLimit')} onChange={(v) => setForm((p) => ({ ...p, birdImportLimit: v }))} type="number" />
          </div>
          <Toggle
            label="Enable Community Filters"
            description="Include DN42 standard AS 64511 community functions (latency, bandwidth, crypto) in BIRD config."
            checked={form.enableCommunityFilters}
            onChange={(v) => setForm((p) => ({ ...p, enableCommunityFilters: v }))}
          />
          <div className="grid grid-cols-1 gap-sm">
            <Textarea label="Import Filter Body" value={f('birdImportFilter')} onChange={(v) => setForm((p) => ({ ...p, birdImportFilter: v }))} placeholder="if is_valid_network() && !is_self_net() then { ... }" code />
            <Textarea label="Export Filter Body" value={f('birdExportFilter')} onChange={(v) => setForm((p) => ({ ...p, birdExportFilter: v }))} placeholder="if is_valid_network() && source ~ [RTS_STATIC, RTS_BGP] then accept; else reject;" code />
          </div>
        </div>

        {/* BFD */}
        <div className="card space-y-md">
          <h2 className="text-body-sm-strong text-ink">BFD (Bidirectional Forwarding Detection)</h2>
          <p className="text-body-sm text-mute">Enable fast failure detection for BGP sessions over WireGuard tunnels.</p>
          <Toggle
            label="Enable BFD"
            description="Add a protocol bfd block monitoring all WireGuard interfaces for rapid link failure detection."
            checked={form.enableBfd}
            onChange={(v) => setForm((p) => ({ ...p, enableBfd: v }))}
          />
          {form.enableBfd && (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
              <Input label="BFD Interval (ms)" value={f('bfdIntervalMs')} onChange={(v) => setForm((p) => ({ ...p, bfdIntervalMs: v }))} type="number" placeholder="300" />
              <Input label="BFD Multiplier" value={f('bfdMultiplier')} onChange={(v) => setForm((p) => ({ ...p, bfdMultiplier: v }))} type="number" placeholder="3" />
            </div>
          )}
        </div>

        <button type="submit" disabled={saving} className="btn-primary">
          {saving ? 'Saving...' : 'Save Settings'}
        </button>
      </form>
    </div>
  );
}

function Input({
  label, value, onChange, type = 'text', placeholder,
}: {
  label: string; value: string; onChange: (v: string) => void; type?: string; placeholder?: string;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-caption text-mute">{label}</label>
      <input
        type={type}
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        className="form-input"
      />
    </div>
  );
}

function Textarea({
  label, value, onChange, placeholder, code,
}: {
  label: string; value: string; onChange: (v: string) => void; placeholder?: string; code?: boolean;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-caption text-mute">{label}</label>
      <textarea
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        rows={3}
        className={code ? 'form-input font-mono' : 'form-input'}
        style={code ? { fontFamily: 'Geist Mono, ui-monospace, monospace', fontSize: '13px' } : undefined}
      />
    </div>
  );
}

function Toggle({
  label, description, checked, onChange,
}: {
  label: string; description?: string; checked: boolean; onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-start gap-sm">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${checked ? 'bg-cyan' : 'bg-hairline'}`}
      >
        <span
          className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out ${checked ? 'translate-x-4' : 'translate-x-0'}`}
        />
      </button>
      <div className="flex flex-col gap-0.5">
        <span className="text-body-sm-strong text-ink">{label}</span>
        {description && <span className="text-caption text-mute">{description}</span>}
      </div>
    </div>
  );
}
