import { useCallback, useEffect, useState } from 'react';
import type { Node } from '../lib/peerman_pb';
import { clusterClient } from '../lib/grpc';

export function useNodes() {
  const [nodes, setNodes] = useState<Node[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchNodes = useCallback(async () => {
    try {
      setLoading(true);
      const res = await clusterClient.listNodes({});
      setNodes(res.nodes);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchNodes();
  }, [fetchNodes]);

  return { nodes, loading, error, refetch: fetchNodes };
}

export function useNode(id: string | undefined) {
  // getNode doesn't exist as an RPC — find from list
  const { nodes, loading, error } = useNodes();
  const node = nodes.find(n => n.id === id) ?? null;
  return { node, loading, error };
}
