import { useCallback, useEffect, useState } from 'react';
import type { Peer, ListPeersResponse, ConfigResponse } from '../lib/peerman_pb';
import { peerClient } from '../lib/grpc';

export function usePeers() {
  const [peers, setPeers] = useState<Peer[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchPeers = useCallback(async () => {
    try {
      setLoading(true);
      const res: ListPeersResponse = await peerClient.listPeers({});
      setPeers(res.peers);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchPeers();
  }, [fetchPeers]);

  return { peers, loading, error, refetch: fetchPeers };
}

export function usePeer(id: string | undefined) {
  const [peer, setPeer] = useState<Peer | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!id) return;
    (async () => {
      try {
        setLoading(true);
        const p = await peerClient.getPeer({ id });
        setPeer(p);
        setError(null);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, [id]);

  return { peer, loading, error };
}

export function useGenerateKeypair() {
  const [loading, setLoading] = useState(false);

  const generate = useCallback(async () => {
    setLoading(true);
    try {
      const res = await peerClient.generateKeypair({});
      return { privateKey: res.privateKey, publicKey: res.publicKey };
    } finally {
      setLoading(false);
    }
  }, []);

  return { generate, loading };
}

export function useWireGuardConfig(peerId: string | undefined) {
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!peerId) return;
    (async () => {
      setLoading(true);
      try {
        const res: ConfigResponse = await peerClient.getWireGuardConfig({ id: peerId });
        setContent(res.content);
      } finally {
        setLoading(false);
      }
    })();
  }, [peerId]);

  return { content, loading };
}

export function useBirdConfig(peerId: string | undefined) {
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!peerId) return;
    (async () => {
      setLoading(true);
      try {
        const res: ConfigResponse = await peerClient.getBirdConfig({ id: peerId });
        setContent(res.content);
      } finally {
        setLoading(false);
      }
    })();
  }, [peerId]);

  return { content, loading };
}

export function useRestartWireGuard() {
  const [loading, setLoading] = useState(false);

  const restart = useCallback(async () => {
    setLoading(true);
    try {
      await peerClient.restartWireGuard({});
    } finally {
      setLoading(false);
    }
  }, []);

  return { restart, loading };
}
