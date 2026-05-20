import { useCallback, useEffect, useState } from 'react';
import type { ProbeResult } from '../lib/peerman_pb';
import { clusterClient } from '../lib/grpc';

export function useProbes(fromNodeId?: string, toNodeId?: string) {
  const [probes, setProbes] = useState<ProbeResult[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchProbes = useCallback(async () => {
    try {
      setLoading(true);
      const res = await clusterClient.listProbeResults({
        fromNodeId: fromNodeId ?? '',
        toNodeId: toNodeId ?? '',
        limit: 100,
      });
      setProbes(res.results);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [fromNodeId, toNodeId]);

  useEffect(() => {
    fetchProbes();
  }, [fetchProbes]);

  return { probes, loading, error, refetch: fetchProbes };
}

export function useRunProbe() {
  const [loading, setLoading] = useState(false);

  const run = useCallback(async (fromNodeId: string, toNodeId: string) => {
    setLoading(true);
    try {
      const res = await clusterClient.runProbe({ fromNodeId, toNodeId });
      return res.result;
    } finally {
      setLoading(false);
    }
  }, []);

  return { run, loading };
}
