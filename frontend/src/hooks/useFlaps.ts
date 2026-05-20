import { useState, useCallback, useEffect } from 'react';
import { create } from '@bufbuild/protobuf';
import {
  ListFlapEventsRequestSchema,
  GetFlapStatsRequestSchema,
  GetFlapStatsResponseSchema,
} from '../lib/peerman_pb';
import type { FlapEvent, GetFlapStatsResponse } from '../lib/peerman_pb';
import { flapClient } from '../lib/grpc';

function defaultStats(): GetFlapStatsResponse {
  return create(GetFlapStatsResponseSchema, {
    activeCount: 0,
    totalToday: 0,
    avgChangesPerHour: 0,
  });
}

export function useFlapEvents(activeOnly: boolean = true) {
  const [events, setEvents] = useState<FlapEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await flapClient.listFlapEvents(
        create(ListFlapEventsRequestSchema, { activeOnly, limit: 50 })
      );
      setEvents(res.events);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [activeOnly]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { events, loading, error, refetch: fetch };
}

export function useFlapStats() {
  const [stats, setStats] = useState<GetFlapStatsResponse>(defaultStats());
  const [loading, setLoading] = useState(true);

  const fetch = useCallback(async () => {
    setLoading(true);
    try {
      const res = await flapClient.getFlapStats(
        create(GetFlapStatsRequestSchema, {})
      );
      setStats(res);
    } catch {
      // stats unavailable — keep defaults
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { stats, loading, refetch: fetch };
}
