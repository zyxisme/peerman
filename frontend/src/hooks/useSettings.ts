import { useCallback, useEffect, useState } from 'react';
import type { Settings } from '../lib/peerman_pb';
import { settingsClient } from '../lib/grpc';

export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchSettings = useCallback(async () => {
    try {
      setLoading(true);
      const s = await settingsClient.getSettings({});
      setSettings(s);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchSettings();
  }, [fetchSettings]);

  const saveSettings = useCallback(async (s: Settings) => {
    const updated = await settingsClient.saveSettings({ settings: s });
    setSettings(updated);
    return updated;
  }, []);

  return { settings, loading, error, saveSettings };
}
