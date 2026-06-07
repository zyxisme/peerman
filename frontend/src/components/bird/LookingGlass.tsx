import { useState } from 'react';
import { Search, RotateCw } from 'lucide-react';
import { cn } from '../../lib/utils';
import { useExecuteCommand, useTraceroute } from '../../hooks/useBird';
import { useNodes } from '../../hooks/useNodes';

const PRESETS = [
  { label: 'show protocols', command: 'show protocols' },
  { label: 'show status', command: 'show status' },
  { label: 'show memory', command: 'show memory' },
  { label: 'show route all', command: 'show route all' },
];

export default function LookingGlass() {
  const { nodes } = useNodes();
  const { results, loading, error, execute } = useExecuteCommand();
  const { results: traceResults, loading: traceLoading, run: runTrace } = useTraceroute();

  const [command, setCommand] = useState('');
  const [targetNodeId, setTargetNodeId] = useState('');
  const [traceTarget, setTraceTarget] = useState('');

  const handleExecute = () => {
    if (command.trim()) {
      execute(command.trim(), targetNodeId);
    }
  };

  const handlePreset = (cmd: string) => {
    setCommand(cmd);
    execute(cmd, targetNodeId);
  };

  const handleTrace = (e: React.FormEvent) => {
    e.preventDefault();
    if (traceTarget.trim()) {
      runTrace(traceTarget.trim(), targetNodeId);
    }
  };

  return (
    <div className="space-y-xl animate-fade-in">
      <div>
        <h1 className="text-display-md text-ink">Looking Glass</h1>
        <p className="text-body-md text-body mt-1">
          Query BIRD routing daemon across cluster nodes.
        </p>
      </div>

      {/* Controls */}
      <div className="card space-y-lg">
        <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-sm sm:gap-md">
          <select
            value={targetNodeId}
            onChange={(e) => setTargetNodeId(e.target.value)}
            className="form-input sm:w-48"
          >
            <option value="">All Nodes</option>
            {nodes.map((n) => (
              <option key={n.id} value={n.id}>
                {n.name}
              </option>
            ))}
          </select>

          <input
            type="text"
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleExecute()}
            placeholder="show route for 8.8.8.8..."
            className="form-input flex-1"
          />

          <button
            onClick={handleExecute}
            disabled={loading || !command.trim()}
            className="btn-primary"
          >
            <Search className="w-4 h-4" />
            Execute
          </button>
        </div>

        {/* Presets */}
        <div className="flex flex-wrap gap-2">
          {PRESETS.map((p) => (
            <button
              key={p.command}
              onClick={() => handlePreset(p.command)}
              disabled={loading}
              className={cn(
                'btn-secondary-sm',
                command === p.command && 'bg-primary text-primary-foreground border-primary'
              )}
            >
              {p.label}
            </button>
          ))}
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="card border border-error bg-error-soft">
          <p className="text-body-sm text-error">{error}</p>
        </div>
      )}

      {/* Loading */}
      {loading && (
        <div className="card flex items-center gap-3 text-body">
          <RotateCw className="w-4 h-4 animate-spin" />
          <span className="text-body-sm">Querying BIRD...</span>
        </div>
      )}

      {/* Results */}
      {!loading && !error && results.length > 0 && (
        <div className="space-y-md">
          {results.map((r) => (
            <div key={r.nodeId} className="card">
              <div className="flex items-center gap-2 mb-md">
                <span className="badge">{r.nodeName || r.nodeId}</span>
                {r.statusCode !== 0 && (
                  <span className="badge bg-error-soft text-error">{r.statusCode}</span>
                )}
              </div>
              {r.error ? (
                <p className="text-body-sm text-error-soft">{r.error}</p>
              ) : r.output ? (
                <pre className="code-block">{r.output}</pre>
              ) : (
                <p className="text-body-sm text-mute">No output.</p>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Empty state */}
      {!loading && !error && results.length === 0 && (
        <div className="card-soft text-center py-3xl">
          <p className="text-body-md text-mute">
            Select a command and click Execute to query BIRD.
          </p>
        </div>
      )}

      {/* Traceroute section */}
      <div className="card space-y-md">
        <h2 className="text-display-sm text-ink">Traceroute</h2>
        <form onSubmit={handleTrace} className="flex flex-col sm:flex-row items-stretch sm:items-center gap-sm sm:gap-md">
          <input
            type="text"
            value={traceTarget}
            onChange={(e) => setTraceTarget(e.target.value)}
            placeholder="8.8.8.8 or fd00::1"
            className="form-input flex-1"
          />
          <button
            type="submit"
            disabled={traceLoading || !traceTarget.trim()}
            className="btn-primary"
          >
            <Search className="w-4 h-4" />
            Run Traceroute
          </button>
        </form>

        {traceLoading && (
          <div className="flex items-center gap-3 text-body">
            <RotateCw className="w-4 h-4 animate-spin" />
            <span className="text-body-sm">Running traceroute...</span>
          </div>
        )}

        {traceResults.map((r) => (
          <div key={r.nodeId} className="card-soft">
            <span className="badge mb-2">{r.nodeName || r.nodeId}</span>
            <pre className="code-block mt-2">{r.output}</pre>
          </div>
        ))}
      </div>
    </div>
  );
}
