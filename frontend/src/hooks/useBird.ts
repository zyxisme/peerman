import { useState, useCallback } from 'react';
import { create } from '@bufbuild/protobuf';
import { ExecuteCommandRequestSchema, RunTracerouteRequestSchema } from '../lib/peerman_pb';
import type { NodeBirdResult, NodeTracerouteResult } from '../lib/peerman_pb';
import { birdClient } from '../lib/grpc';

export function useExecuteCommand() {
  const [results, setResults] = useState<NodeBirdResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const execute = useCallback(async (command: string, targetNodeId: string = '') => {
    setLoading(true);
    setError(null);
    try {
      const res = await birdClient.executeCommand(
        create(ExecuteCommandRequestSchema, { command, targetNodeId })
      );
      setResults(res.results);
    } catch (e) {
      setError(String(e));
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, []);

  return { results, loading, error, execute };
}

export function useTraceroute() {
  const [results, setResults] = useState<NodeTracerouteResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async (target: string, targetNodeId: string = '') => {
    setLoading(true);
    setError(null);
    try {
      const res = await birdClient.runTraceroute(
        create(RunTracerouteRequestSchema, { target, targetNodeId })
      );
      setResults(res.results);
    } catch (e) {
      setError(String(e));
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, []);

  return { results, loading, error, run };
}
