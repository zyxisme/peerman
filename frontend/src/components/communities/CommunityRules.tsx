import { useState } from 'react';
import { Plus, Trash2, Check, X } from 'lucide-react';
import { create } from '@bufbuild/protobuf';
import { CommunityRuleSchema } from '../../lib/peerman_pb';
import type { CommunityRule } from '../../lib/peerman_pb';
import { useCommunityRules, useSaveCommunityRule, useDeleteCommunityRule } from '../../hooks/useCommunities';
import { cn } from '../../lib/utils';

export default function CommunityRules() {
  const { rules, loading, error, refetch } = useCommunityRules();
  const { save, loading: saving } = useSaveCommunityRule();
  const { del, loading: deleting } = useDeleteCommunityRule();

  const [editing, setEditing] = useState<CommunityRule | null>(null);
  const [form, setForm] = useState({ description: '', maxLatencyMs: '', maxPacketLossPct: '', communityIpv4: '', communityIpv6: '' });

  const startNew = () => {
    setEditing(create(CommunityRuleSchema, {
      id: '',
      description: '',
      maxLatencyMs: 0,
      maxPacketLossPct: 100,
      communityIpv4: '',
      communityIpv6: '',
      enabled: true,
    }));
    setForm({ description: '', maxLatencyMs: '', maxPacketLossPct: '100', communityIpv4: '', communityIpv6: '' });
  };

  const startEdit = (rule: CommunityRule) => {
    setEditing(rule);
    setForm({
      description: rule.description,
      maxLatencyMs: String(rule.maxLatencyMs),
      maxPacketLossPct: String(rule.maxPacketLossPct),
      communityIpv4: rule.communityIpv4,
      communityIpv6: rule.communityIpv6,
    });
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

  return (
    <div className="space-y-lg animate-fade-in max-w-3xl">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-display-md text-ink">Community Rules</h1>
          <p className="text-body-sm text-mute mt-xxs">
            Auto-tag BGP communities based on network latency
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
              <label className="block text-caption-mono text-mute mb-xxs">Max Latency (ms, 0=inf)</label>
              <input className="form-input w-full" type="number" value={form.maxLatencyMs}
                onChange={e => setForm({ ...form, maxLatencyMs: e.target.value })} />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">Max Packet Loss (%)</label>
              <input className="form-input w-full" type="number" value={form.maxPacketLossPct}
                onChange={e => setForm({ ...form, maxPacketLossPct: e.target.value })} />
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
        <table className="data-table w-full">
          <thead>
            <tr>
              <th>Description</th>
              <th>Max Latency</th>
              <th>Max Loss</th>
              <th>IPv4 Community</th>
              <th>IPv6 Community</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {rules.map(r => (
              <tr key={r.id}>
                <td className="text-body-sm font-medium">{r.description}</td>
                <td className="text-body-sm text-mute">{r.maxLatencyMs <= 0 ? '∞' : `${r.maxLatencyMs}ms`}</td>
                <td className="text-body-sm text-mute">{r.maxPacketLossPct}%</td>
                <td><code className="text-code text-body-sm">{r.communityIpv4}</code></td>
                <td><code className="text-code text-body-sm">{r.communityIpv6}</code></td>
                <td>
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
        </table>
      </div>
    </div>
  );
}
