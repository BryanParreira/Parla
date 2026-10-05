import { ArrowDownToLine, History as HistoryIcon, House, Settings as SettingsIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { useDictation, useHistory, useModelStatus, usePermissions, useSettings, useUpdateCheck, useUpdateInstall } from "../lib/hooks";
import { api } from "../lib/api";
import { cx } from "../lib/utils";
import HistoryPage from "./History";
import HomePage from "./Home";
import Onboarding from "./Onboarding";
import SettingsPage from "./Settings";
import { Keys, Logo, StatusDot } from "./ui";

export type Page = "home" | "history" | "settings";

const NAV = [
  { id: "home", label: "Home", icon: House },
  { id: "history", label: "History", icon: HistoryIcon },
  { id: "settings", label: "Settings", icon: SettingsIcon },
] as const;

export default function App() {
  const { settings, update } = useSettings();
  const model = useModelStatus();
  const permissions = usePermissions();
  const entries = useHistory();
  const { phase, error } = useDictation();
  const [page, setPage] = useState<Page>("home");
  // The menu bar's Settings… and Check for Updates… open a page directly.
  useEffect(() => {
    const unlisten = listen<Page>("navigate", (event) => setPage(event.payload));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);
  const release = useUpdateCheck(!!settings?.checkUpdates && !!settings?.onboarded);
  const installer = useUpdateInstall();

  if (!settings) {
    return <div className="h-full" data-tauri-drag-region />;
  }

  if (!settings.onboarded) {
    return (
      <Onboarding
        hotkey={settings.hotkey}
        model={model}
        permissions={permissions}
        onHotkeyChange={(hotkey) => update({ hotkey })}
        onFinish={() => {
          api.setLaunchAtLogin(true).catch(() => {});
          update({ onboarded: true });
        }}
      />
    );
  }

  const modelLabel =
    model?.state === "ready"
      ? "On-device model ready"
      : model?.state === "loading"
        ? `Preparing model ${Math.round((model.progress ?? 0) * 100)}%`
        : model?.state === "error"
          ? "Model failed to load"
          : "Starting…";

  return (
    <div className="flex h-full">
      <aside className="flex w-[228px] shrink-0 flex-col border-r border-line bg-sidebar px-3 pb-3 pt-12" data-tauri-drag-region>
        <div className="mb-7 flex items-center gap-2.5 px-2" data-tauri-drag-region>
          <Logo />
          <span className="text-[15px] font-semibold tracking-tight">Parla</span>
        </div>

        <nav className="flex flex-col gap-0.5">
          {NAV.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setPage(id)}
              className={cx(
                "relative flex h-9 items-center gap-2.5 rounded-lg px-2.5 text-[13px] font-medium transition-colors",
                page === id ? "text-fg" : "text-muted hover:text-fg",
              )}
            >
              {page === id && (
                <motion.span
                  layoutId="nav-active"
                  className="absolute inset-0 rounded-lg bg-raised ring-1 ring-white/[0.04]"
                  transition={{ type: "spring", stiffness: 500, damping: 38 }}
                />
              )}
              <Icon className="relative size-4" />
              <span className="relative">{label}</span>
            </button>
          ))}
        </nav>

        {release?.available && (
          <button
            onClick={() => installer.install(release.url)}
            disabled={installer.busy}
            title={installer.error ?? undefined}
            className="mt-auto mb-2 flex items-center gap-2.5 rounded-xl border border-line bg-surface px-3 py-2.5 text-left transition-colors hover:bg-raised disabled:cursor-default"
          >
            <ArrowDownToLine className="size-4 shrink-0" />
            <span className="text-[12px] leading-snug">
              <span className="block font-medium">Parla {release.version} is out</span>
              <span className={installer.error ? "text-danger" : "text-muted"}>
                {installer.label ?? (installer.error ? "Couldn't update. Try from Settings" : "Install and restart")}
              </span>
            </span>
          </button>
        )}

        <div className={cx("rounded-xl border border-line bg-surface p-3", !release?.available && "mt-auto")}>
          <div className="flex items-center gap-2 text-[12px] text-muted">
            <StatusDot
              tone={
                phase === "recording" ? "live" : model?.state === "ready" ? "ok" : "warn"
              }
            />
            <span className="truncate">{phase === "recording" ? "Listening…" : phase === "processing" ? "Transcribing…" : modelLabel}</span>
          </div>
          <div className="mt-2.5 flex items-center justify-between">
            <span className="text-[12px] text-muted">Hold to talk</span>
            <Keys hotkey={settings.hotkey} size="sm" active={phase === "recording"} />
          </div>
        </div>
      </aside>

      <main className="relative flex-1 overflow-y-auto">
        <div className="sticky top-0 z-10 h-11 bg-bg/80 backdrop-blur" data-tauri-drag-region />
        <AnimatePresence mode="wait">
          <motion.div
            key={page}
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.18, ease: "easeOut" }}
            className="mx-auto max-w-[760px] px-10 pb-14"
          >
            {page === "home" && (
              <HomePage
                hotkey={settings.hotkey}
                model={model}
                permissions={permissions}
                entries={entries}
                phase={phase}
                error={error}
                onNavigate={setPage}
              />
            )}
            {page === "history" && <HistoryPage entries={entries} />}
            {page === "settings" && (
              <SettingsPage settings={settings} update={update} model={model} permissions={permissions} entries={entries} />
            )}
          </motion.div>
        </AnimatePresence>
      </main>
    </div>
  );
}
