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

export function useClusterHealth(): 'all-online' | 'partial' | 'isolated' {
  const { nodes } = useNodes();
  if (nodes.length <= 1) return 'isolated';
  const onlineCount = nodes.filter((n) => n.online).length;
  if (onlineCount === nodes.length) return 'all-online';
  if (onlineCount <= 1) return 'isolated';
  return 'partial';
}

export function useNode(id: string | undefined) {
  const [node, setNode] = useState<Node | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchNode = useCallback(async () => {
    if (!id) {
      setLoading(false);
      return;
    }
    try {
      setLoading(true);
      const res = await clusterClient.getNode({ id });
      setNode(res);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => {
    fetchNode();
  }, [fetchNode]);

  return { node, loading, error };
}
