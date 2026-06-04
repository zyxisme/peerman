import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { create } from '@bufbuild/protobuf';
import { RegisterNodeRequestSchema, UpdateNodeRequestSchema } from '../../lib/peerman_pb';
import { useNode, useNodes } from '../../hooks/useNodes';
import { clusterClient } from '../../lib/grpc';

export default function NodeForm() {
  const { id } = useParams<{ id: string }>();
  const { node } = useNode(id);
  const { refetch } = useNodes();
  const navigate = useNavigate();
  const isEdit = Boolean(id && node);

  const [name, setName] = useState(node?.name ?? '');
  const [listenAddr, setListenAddr] = useState(node?.listenAddr ?? '');
  const [localAsn, setLocalAsn] = useState(String(node?.localAsn ?? ''));
  const [description, setDescription] = useState(node?.description ?? '');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setSaving(true);
    try {
      if (isEdit) {
        await clusterClient.updateNode(create(UpdateNodeRequestSchema, {
          id: id!,
          name,
          listenAddr,
          localAsn: BigInt(localAsn || '0'),
          description,
        }));
      } else {
        await clusterClient.registerNode(create(RegisterNodeRequestSchema, {
          name,
          listenAddr,
          localAsn: BigInt(localAsn || '0'),
          description,
        }));
      }
      refetch();
      navigate('/nodes');
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="max-w-2xl space-y-lg animate-fade-in">
      <h1 className="text-display-md text-ink">{isEdit ? 'Edit Node' : 'Register Node'}</h1>

      {error && (
        <div className="bg-error-soft text-error-deep text-body-sm rounded-sm px-md py-sm">{error}</div>
      )}

      <form onSubmit={handleSubmit} className="card space-y-lg">
        <fieldset className="space-y-md">
          <legend className="text-body-md-strong text-ink mb-sm">Identity</legend>
          <div>
            <label className="block text-body-sm text-body mb-xxs">Name</label>
            <input className="form-input w-full" value={name} onChange={e => setName(e.target.value)}
              placeholder="alpha-dn42" required />
          </div>
          <div>
            <label className="block text-body-sm text-body mb-xxs">Listen Address</label>
            <input className="form-input w-full" value={listenAddr} onChange={e => setListenAddr(e.target.value)}
              placeholder="172.20.1.1:3000" required />
          </div>
          <div>
            <label className="block text-body-sm text-body mb-xxs">Local ASN</label>
            <input className="form-input w-full" value={localAsn} onChange={e => setLocalAsn(e.target.value)}
              placeholder="4242420001" />
          </div>
          <div>
            <label className="block text-body-sm text-body mb-xxs">Description</label>
            <input className="form-input w-full" value={description} onChange={e => setDescription(e.target.value)}
              placeholder="Optional description" />
          </div>
        </fieldset>

        <div className="flex items-center gap-sm">
          <button type="submit" disabled={saving} className="btn-primary">
            {saving ? 'Saving...' : isEdit ? 'Update Node' : 'Register Node'}
          </button>
          <button type="button" onClick={() => navigate('/nodes')} className="btn-secondary">
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
