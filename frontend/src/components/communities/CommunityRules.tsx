import { useState } from 'react';
import { Plus, Trash2, Check, X } from 'lucide-react';
import { create } from '@bufbuild/protobuf';
import { CommunityRuleSchema } from '../../lib/peerman_pb';
import type { CommunityRule } from '../../lib/peerman_pb';
import { useCommunityRules, useSaveCommunityRule, useDeleteCommunityRule } from '../../hooks/useCommunities';
import { ResponsiveTable } from '../ui/ResponsiveTable';

type Form = {
  description: string;
  maxLatencyMs: string;
  maxPacketLossPct: string;
  communityIpv4: string;
  communityIpv6: string;
  minBandwidthMbps: string;
  cryptoWeight: string;
  medPenalty: string;
};

const emptyForm: Form = {
  description: '', maxLatencyMs: '', maxPacketLossPct: '100',
  communityIpv4: '', communityIpv6: '',
  minBandwidthMbps: '0', cryptoWeight: '0', medPenalty: '0',
};

function ruleToForm(r: CommunityRule): Form {
  return {
    description: r.description,
    maxLatencyMs: String(r.maxLatencyMs),
    maxPacketLossPct: String(r.maxPacketLossPct),
    communityIpv4: r.communityIpv4,
    communityIpv6: r.communityIpv6,
    minBandwidthMbps: String(r.minBandwidthMbps),
    cryptoWeight: String(r.cryptoWeight),
    medPenalty: String(r.medPenalty),
  };
}

export default function CommunityRules() {
  const { rules, loading, error, refetch } = useCommunityRules();
  const { save, loading: saving } = useSaveCommunityRule();
  const { del, loading: deleting } = useDeleteCommunityRule();

  const [editing, setEditing] = useState<CommunityRule | null>(null);
  const [form, setForm] = useState<Form>(emptyForm);

  const startNew = () => {
    setEditing(create(CommunityRuleSchema, {
      id: '', description: '', maxLatencyMs: 0, maxPacketLossPct: 100,
      communityIpv4: '', communityIpv6: '', enabled: true,
      minBandwidthMbps: 0, cryptoWeight: 0, medPenalty: 0,
    }));
    setForm(emptyForm);
  };

  const startEdit = (rule: CommunityRule) => {
    setEditing(rule);
    setForm(ruleToForm(rule));
  };

  const handleSave = async () => {
    if (!editing) return;
    const rule = create(CommunityRuleSchema, {
      id: editing.id,
      description: form.description,
      maxLatencyMs: parseFloat(form.maxLatencyMs) || 100000,
      maxPacketLossPct: parseFloat(form.maxPacketLossPct) ?? 100,
      communityIpv4: form.communityIpv4,
      communityIpv6: form.communityIpv6,
      enabled: true,
      minBandwidthMbps: parseFloat(form.minBandwidthMbps) || 0,
      cryptoWeight: parseInt(form.cryptoWeight) || 0,
      medPenalty: parseInt(form.medPenalty) || 0,
    });
    await save(rule);
    setEditing(null);
    refetch();
  };

  const handleDelete = async (id: string) => {
    if (!confirm('Delete this rule?')) return;
    await del(id);
    refetch();
  };

  if (loading) return <div className="text-mute p-lg">Loading...</div>;
  if (error) return <div className="text-error p-lg">{error}</div>;

  const fmtInf = (n: number, suffix: string) => n <= 0 ? '∞' : `${n}${suffix}`;

  return (
    <div className="space-y-lg animate-fade-in max-w-3xl">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-display-md text-ink">Community Rules</h1>
          <p className="text-body-sm text-mute mt-xxs">
            Auto-tag BGP communities based on latency, bandwidth, and crypto weight
          </p>
        </div>
        <button onClick={startNew} className="btn-primary inline-flex items-center gap-1.5">
          <Plus className="w-4 h-4" /> Add Rule
        </button>
      </div>

      {/* Inline Editor */}
      {editing && (
        <div className="card border border-link/20 bg-canvas-soft">
          <h3 className="text-body-md-strong text-ink mb-md">{editing.id ? 'Edit Rule' : 'New Rule'}</h3>
          <div className="grid grid-cols-2 gap-md mb-md">
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">Description</label>
              <input className="form-input w-full" value={form.description}
                onChange={e => setForm({ ...form, description: e.target.value })}
                placeholder="e.g., Metro (<5ms)" />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">Max Latency (ms, 0=∞)</label>
              <input className="form-input w-full" type="number" value={form.maxLatencyMs}
                onChange={e => setForm({ ...form, maxLatencyMs: e.target.value })} />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">Max Packet Loss (%)</label>
              <input className="form-input w-full" type="number" value={form.maxPacketLossPct}
                onChange={e => setForm({ ...form, maxPacketLossPct: e.target.value })} />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">Min Bandwidth (Mbps, 0=∞)</label>
              <input className="form-input w-full" type="number" value={form.minBandwidthMbps}
                onChange={e => setForm({ ...form, minBandwidthMbps: e.target.value })} />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">Crypto Weight</label>
              <input className="form-input w-full" type="number" value={form.cryptoWeight}
                onChange={e => setForm({ ...form, cryptoWeight: e.target.value })} />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">MED Penalty</label>
              <input className="form-input w-full" type="number" value={form.medPenalty}
                onChange={e => setForm({ ...form, medPenalty: e.target.value })} />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">IPv4 Community</label>
              <input className="form-input w-full" value={form.communityIpv4}
                onChange={e => setForm({ ...form, communityIpv4: e.target.value })}
                placeholder="<asn>,10" />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">IPv6 Community</label>
              <input className="form-input w-full" value={form.communityIpv6}
                onChange={e => setForm({ ...form, communityIpv6: e.target.value })}
                placeholder="<asn>,610" />
            </div>
          </div>
          <div className="flex items-center gap-sm">
            <button onClick={handleSave} disabled={saving} className="btn-primary text-body-sm inline-flex items-center gap-1">
              <Check className="w-3.5 h-3.5" /> Save
            </button>
            <button onClick={() => setEditing(null)} className="btn-secondary text-body-sm inline-flex items-center gap-1">
              <X className="w-3.5 h-3.5" /> Cancel
            </button>
          </div>
        </div>
      )}

      {/* Rules Table */}
      <div className="card overflow-hidden !p-0">
        <ResponsiveTable>
          <thead>
            <tr>
              <th>Description</th>
              <th>Latency</th>
              <th>Loss</th>
              <th>Bandwidth</th>
              <th>Crypto</th>
              <th>MED</th>
              <th>IPv4</th>
              <th>IPv6</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {rules.map(r => (
              <tr key={r.id}>
                <td data-label="Description" className="text-body-sm font-medium">{r.description}</td>
                <td data-label="Latency" className="text-body-sm text-mute">{fmtInf(r.maxLatencyMs, 'ms')}</td>
                <td data-label="Loss" className="text-body-sm text-mute">{r.maxPacketLossPct}%</td>
                <td data-label="Bandwidth" className="text-body-sm text-mute">{fmtInf(r.minBandwidthMbps, ' Mbps')}</td>
                <td data-label="Crypto" className="text-body-sm text-mute">{r.cryptoWeight || '-'}</td>
                <td data-label="MED" className="text-body-sm text-mute">{r.medPenalty || '-'}</td>
                <td data-label="IPv4"><code className="text-code text-body-sm">{r.communityIpv4}</code></td>
                <td data-label="IPv6"><code className="text-code text-body-sm">{r.communityIpv6}</code></td>
                <td data-label="Actions">
                  <div className="flex items-center gap-1">
                    <button onClick={() => startEdit(r)} className="btn-secondary text-caption px-xs py-0.5">Edit</button>
                    <button onClick={() => handleDelete(r.id)} disabled={deleting} className="p-1 rounded-sm hover:bg-error-soft text-mute hover:text-error">
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </ResponsiveTable>
      </div>
    </div>
  );
}
