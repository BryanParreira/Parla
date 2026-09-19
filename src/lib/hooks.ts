import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, type DictationEvent, type Entry, type Settings } from "./api";

export type Phase = "idle" | "recording" | "processing";

export function usePolling<T>(load: () => Promise<T>, intervalMs: number) {
  const [value, setValue] = useState<T | null>(null);

  useEffect(() => {
    let alive = true;
    const tick = () =>
      load()
        .then((next) => alive && setValue(next))
        .catch(() => {});
    tick();
    const timer = setInterval(tick, intervalMs);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [load, intervalMs]);

  return value;
}

export const useModelStatus = () => usePolling(api.modelStatus, 1000);
export const usePermissions = () => usePolling(api.permissions, 1500);

export function useDictation(onEvent?: (event: DictationEvent) => void) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [error, setError] = useState<string | null>(null);
  const handler = useRef(onEvent);

  useEffect(() => {
    handler.current = onEvent;
  });

  useEffect(() => {
    const unlisten = listen<DictationEvent>("dictation", ({ payload }) => {
      handler.current?.(payload);
      switch (payload.phase) {
        case "recording":
          setError(null);
          setPhase("recording");
          break;
        case "processing":
          setPhase("processing");
          break;
        case "error":
          setError(payload.message);
          setPhase("idle");
          break;
        default:
          setPhase("idle");
      }
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  return { phase, error };
}

export function useHistory() {
  const [entries, setEntries] = useState<Entry[]>([]);

  const refresh = useCallback(() => {
    api.history().then(setEntries).catch(() => {});
  }, []);

  useEffect(() => {
    refresh();
    const unlisten = listen("history-changed", refresh);
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [refresh]);

  return entries;
}

export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);

  useEffect(() => {
    api.getSettings().then(setSettings).catch(() => {});
  }, []);

  const update = useCallback(
    async (patch: Partial<Settings>) => {
      if (!settings) return;
      setSettings(await api.updateSettings({ ...settings, ...patch }));
    },
    [settings],
  );

  return { settings, update };
}
