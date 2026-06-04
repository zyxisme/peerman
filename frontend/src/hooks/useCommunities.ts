import { useCallback, useEffect, useState } from 'react';
import type { CommunityRule } from '../lib/peerman_pb';
import { clusterClient } from '../lib/grpc';

export function useCommunityRules() {
  const [rules, setRules] = useState<CommunityRule[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchRules = useCallback(async () => {
    try {
      setLoading(true);
      const res = await clusterClient.listCommunityRules({});
      setRules(res.rules);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchRules();
  }, [fetchRules]);

  return { rules, loading, error, refetch: fetchRules };
}

export function useSaveCommunityRule() {
  const [loading, setLoading] = useState(false);

  const save = useCallback(async (rule: CommunityRule) => {
    setLoading(true);
    try {
      const res = await clusterClient.saveCommunityRule({ rule });
      return res;
    } finally {
      setLoading(false);
    }
  }, []);

  return { save, loading };
}

export function useDeleteCommunityRule() {
  const [loading, setLoading] = useState(false);

  const del = useCallback(async (id: string) => {
    setLoading(true);
    try {
      await clusterClient.deleteCommunityRule({ id });
    } finally {
      setLoading(false);
    }
  }, []);

  return { del, loading };
}

export function usePeerCommunities(peerId: string | undefined) {
  const [communities, setCommunities] = useState<{ v4: string[]; v6: string[] }>({ v4: [], v6: [] });
  const [loading, setLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!peerId) return;
    setLoading(true);
    try {
      const res = await clusterClient.getPeerCommunities({ peerId });
      setCommunities({ v4: res.communityIpv4, v6: res.communityIpv6 });
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, [peerId]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { communities, loading, refetch: fetch };
}
