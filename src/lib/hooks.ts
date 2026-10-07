import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, type DictationEvent, type Entry, type Settings, type Suggestion, type UpdateInfo, type UpdateProgress } from "./api";

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
        case "working":
          setPhase("processing");
          break;
        // A passing note, like a mode switch, says nothing about a dictation.
        case "notice":
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
    const load = () => api.getSettings().then(setSettings).catch(() => {});
    load();
    // Switches flipped in the menu bar show up here too.
    const unlisten = listen("settings-changed", load);
    return () => {
      unlisten.then((stop) => stop());
    };
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

export function useSuggestions() {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);

  const refresh = useCallback(() => {
    api.suggestions().then(setSuggestions).catch(() => {});
  }, []);

  useEffect(() => {
    refresh();
    const unlisten = listen("suggestions-changed", refresh);
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [refresh]);

  return suggestions;
}

const DAY_MS = 24 * 60 * 60 * 1000;

/** Asks GitHub for a newer release once a day, only while the user has it turned on. */
export function useUpdateCheck(enabled: boolean) {
  const [update, setUpdate] = useState<UpdateInfo | null>(null);

  useEffect(() => {
    if (!enabled) {
      setUpdate(null);
      return;
    }
    const check = () => api.checkForUpdate().then(setUpdate).catch(() => {});
    check();
    const timer = setInterval(check, DAY_MS);
    return () => clearInterval(timer);
  }, [enabled]);

  return update;
}

/** Runs an in-app update and follows its progress until Parla restarts. */
export function useUpdateInstall() {
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<UpdateProgress>("update-progress", ({ payload }) => setProgress(payload));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  const install = useCallback((url: string) => {
    setError(null);
    setProgress({ phase: "downloading", progress: 0 });
    api.installUpdate(url).catch((reason) => {
      setProgress(null);
      setError(String(reason));
    });
  }, []);

  const label =
    progress?.phase === "downloading"
      ? `Downloading ${Math.round(progress.progress * 100)}%`
      : progress?.phase === "verifying"
        ? "Checking it's genuine…"
        : progress?.phase === "restarting"
          ? "Restarting…"
          : null;

  return { install, busy: progress !== null, label, error };
}
