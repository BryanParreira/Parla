import { AlertTriangle, ArrowRight, Clock, Gauge, Mic, Type } from "lucide-react";
import { motion } from "motion/react";
import type { ReactNode } from "react";
import { api, type Entry, type Hotkey, type ModelStatus, type Permissions } from "../lib/api";
import type { Phase } from "../lib/hooks";
import { computeStats, cx, formatLatency, formatNumber, formatSaved, formatTime, greeting } from "../lib/utils";
import type { Page } from "./App";
import { Button, Card, CopyButton, Keys } from "./ui";

export default function HomePage({
  hotkey,
  model,
  permissions,
  entries,
  phase,
  error,
  onNavigate,
}: {
  hotkey: Hotkey;
  model: ModelStatus | null;
  permissions: Permissions | null;
  entries: Entry[];
  phase: Phase;
  error: string | null;
  onNavigate: (page: Page) => void;
}) {
  const stats = computeStats(entries);
  const recent = entries.slice(0, 4);
  const lastLatency = entries[0]?.latencyMs;

  return (
    <div className="flex flex-col gap-5">
      <header>
        <p className="text-[13px] text-muted">{greeting()}</p>
        <h1 className="mt-0.5 text-[28px] font-semibold tracking-tight">Talk, don't type.</h1>
      </header>

      {permissions?.microphone === "denied" && (
        <Banner
          message="Microphone access is off, so Parla can't hear you."
          action="Open settings"
          onAction={() => api.openPrivacySettings("microphone")}
        />
      )}
      {permissions && !permissions.accessibility && (
        <Banner
          message="Without Accessibility permission, dictations are copied to the clipboard instead of typed."
          action="Enable"
          onAction={() => api.grantAccessibility()}
        />
      )}
      {model?.state === "error" && (
        <Banner message={model.message ?? "The speech model failed to load."} action="Retry" onAction={api.prepareModel} />
      )}

      <Card className="relative overflow-hidden px-8 py-10 text-center">
        <motion.div
          className="pointer-events-none absolute left-1/2 top-1/2 size-72 -translate-x-1/2 -translate-y-1/2 rounded-full blur-3xl"
          animate={{
            backgroundColor: phase === "recording" ? "rgb(255 255 255 / 0.08)" : "rgb(255 255 255 / 0.03)",
            scale: phase === "recording" ? 1.15 : 1,
          }}
          transition={{ duration: 0.4 }}
        />
        <div className="relative">
          <Keys hotkey={hotkey} size="lg" active={phase === "recording"} />
          <p className="mt-5 text-[15px] font-medium">
            {phase === "recording" ? "Listening…" : phase === "processing" ? "Transcribing…" : "Hold to dictate in any app"}
          </p>
          <p className={cx("mt-1 text-[13px]", error ? "text-danger" : "text-muted")}>
            {error ?? "Release when you're done and your words are typed where your cursor is."}
          </p>
          {!error && phase === "idle" && lastLatency != null && (
            <p className="mt-3 text-[12px] tabular-nums text-muted">Last dictation typed {formatLatency(lastLatency)} after you let go</p>
          )}
        </div>
      </Card>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <Stat icon={<Type className="size-4" />} label="Words dictated" value={formatNumber(stats.words)} />
        <Stat icon={<Mic className="size-4" />} label="This week" value={formatNumber(stats.weekWords)} />
        <Stat icon={<Gauge className="size-4" />} label="Speaking speed" value={stats.wpm ? `${stats.wpm} wpm` : "—"} />
        <Stat icon={<Clock className="size-4" />} label="Time saved" value={formatSaved(stats.savedMs)} />
      </div>

      <section>
        <div className="mb-2.5 flex items-center justify-between">
          <h2 className="text-[13px] font-semibold">Recent</h2>
          {entries.length > 0 && (
            <Button variant="ghost" className="h-7 px-2 text-[12px]" onClick={() => onNavigate("history")}>
              View all <ArrowRight className="size-3.5" />
            </Button>
          )}
        </div>
        {recent.length === 0 ? (
          <Card className="px-6 py-10 text-center text-[13px] text-muted">
            Your dictations will show up here.
          </Card>
        ) : (
          <Card className="divide-y divide-line">
            {recent.map((entry) => (
              <div key={entry.id} className="group flex items-start gap-4 px-4 py-3">
                <span className="w-16 shrink-0 pt-0.5 text-[12px] tabular-nums text-muted">{formatTime(entry.createdAt)}</span>
                <p className="selectable line-clamp-2 flex-1 text-[13px] leading-relaxed">{entry.text}</p>
                <div className="opacity-0 transition-opacity group-hover:opacity-100">
                  <CopyButton text={entry.text} />
                </div>
              </div>
            ))}
          </Card>
        )}
      </section>
    </div>
  );
}

function Stat({ icon, label, value }: { icon: ReactNode; label: string; value: string }) {
  return (
    <Card className="p-4">
      <div className="flex items-center gap-2 text-muted">
        {icon}
        <span className="text-[12px]">{label}</span>
      </div>
      <p className="mt-2 text-[22px] font-semibold tracking-tight tabular-nums">{value}</p>
    </Card>
  );
}

function Banner({ message, action, onAction }: { message: string; action: string; onAction: () => void }) {
  return (
    <div className="flex items-center gap-3 rounded-xl border border-line bg-surface px-4 py-2.5">
      <AlertTriangle className="size-4 shrink-0 text-muted" />
      <p className="flex-1 text-[13px]">{message}</p>
      <Button variant="secondary" className="h-7 px-2.5 text-[12px]" onClick={onAction}>
        {action}
      </Button>
    </div>
  );
}
