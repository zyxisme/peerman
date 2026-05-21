import { useState, useEffect, useCallback } from 'react';
import { create } from '@bufbuild/protobuf';
import { GetWGStatusRequestSchema, GetBirdStatusRequestSchema } from '../lib/peerman_pb';
import type { WGInterface, BirdProtocol } from '../lib/peerman_pb';
import { mgmtClient } from '../lib/grpc';

export function useWireGuardStatus(iface: string = '') {
  const [interfaces, setInterfaces] = useState<WGInterface[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await mgmtClient.getWireGuardStatus(
        create(GetWGStatusRequestSchema, { interface: iface })
      );
      setInterfaces(res.interfaces);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [iface]);

  useEffect(() => { fetch(); }, [fetch]);

  return { interfaces, loading, error, refetch: fetch };
}

export function useBirdStatus() {
  const [protocols, setProtocols] = useState<BirdProtocol[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await mgmtClient.getBirdStatus(
        create(GetBirdStatusRequestSchema, {})
      );
      setProtocols(res.protocols);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);

  return { protocols, loading, error, refetch: fetch };
}
