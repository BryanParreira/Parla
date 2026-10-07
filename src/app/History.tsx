import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ChevronDown, Download, FileAudio, Info, Pause, Play, RotateCcw, Search, Trash2 } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { api, IS_MAC, languageName, type Entry, type Settings, THIS_DEVICE } from "../lib/api";
import { cx, dayLabel, entryWpm, formatLatency, formatTime } from "../lib/utils";
import { Button, Card, CopyButton, PageHeader } from "./ui";

const CONTEXT_NAMES: Record<string, string> = {
  selection: "selected text",
  clipboard: "copied text",
  app: "app and date",
};

// Oldest first reads like a journal; each day gets a heading and each entry its time.
function toMarkdown(entries: Entry[]) {
  const lines = ["# Parla history", ""];
  let day = "";
  for (const entry of [...entries].reverse()) {
    const label = new Date(entry.createdAt).toLocaleDateString([], { weekday: "long", year: "numeric", month: "long", day: "numeric" });
    if (label !== day) {
      lines.push(`## ${label}`, "");
      day = label;
    }
    const from = entry.source ? ` · from ${entry.source}` : "";
    lines.push(`**${formatTime(entry.createdAt)}** · ${entry.words} words${from}`, "", entry.text, "");
  }
  return lines.join("\n");
}

export default function HistoryPage({ entries, settings }: { entries: Entry[]; settings: Settings }) {
  const [query, setQuery] = useState("");
  const [confirmClear, setConfirmClear] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const [dropping, setDropping] = useState(false);
  const [open, setOpen] = useState<Set<number>>(() => new Set());

  // Audio and video dropped anywhere on the window are transcribed.
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") setDropping(true);
      else if (event.payload.type === "leave") setDropping(false);
      else if (event.payload.type === "drop") {
        setDropping(false);
        for (const path of event.payload.paths.slice(0, 1)) api.transcribeFile(path).catch(() => {});
      }
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  const exportHistory = async () => {
    setExportError(null);
    const stamp = new Date().toISOString().slice(0, 10);
    try {
      await api.saveExport(`Parla history ${stamp}.md`, toMarkdown(entries));
    } catch (error) {
      setExportError(String(error));
    }
  };

  const toggle = (id: number) =>
    setOpen((current) => {
      const next = new Set(current);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  const groups = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const matches = needle
      ? entries.filter((e) => `${e.text} ${e.raw ?? ""} ${e.source ?? ""}`.toLowerCase().includes(needle))
      : entries;
    const byDay = new Map<string, Entry[]>();
    for (const entry of matches) {
      const label = dayLabel(entry.createdAt);
      byDay.set(label, [...(byDay.get(label) ?? []), entry]);
    }
    return [...byDay.entries()];
  }, [entries, query]);

  return (
    <div className={cx("relative", dropping && "after:pointer-events-none after:absolute after:-inset-3 after:rounded-2xl after:border-2 after:border-dashed after:border-fg/40")}>
      <PageHeader
        title="History"
        subtitle={`Everything you've dictated, stored only on ${THIS_DEVICE}.`}
        action={
          <div className="flex gap-1.5">
            {IS_MAC && (
              <Button variant="ghost" className="h-8 text-[12px]" onClick={() => api.transcribeFile()} title="Or drop an audio or video file on this window">
                <FileAudio className="size-3.5" />
                Transcribe a file
              </Button>
            )}
            {entries.length > 0 && (
              <>
                <Button variant="ghost" className="h-8 text-[12px]" onClick={exportHistory} title="Save as Markdown in Downloads">
                  <Download className="size-3.5" />
                  Export
                </Button>
                <Button
                  variant={confirmClear ? "danger" : "ghost"}
                  className="h-8 text-[12px]"
                  onBlur={() => setConfirmClear(false)}
                  onClick={async () => {
                    if (!confirmClear) return setConfirmClear(true);
                    await api.clearHistory();
                    setConfirmClear(false);
                  }}
                >
                  <Trash2 className="size-3.5" />
                  {confirmClear ? "Click again to clear all" : "Clear all"}
                </Button>
              </>
            )}
          </div>
        }
      />

      {exportError && <p className="-mt-2 mb-4 text-[12px] text-danger">{exportError}</p>}

      <div className="relative mb-6">
        <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted" />
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search dictations"
          className="selectable h-10 w-full rounded-xl border border-line bg-surface pl-9 pr-3 text-[13px] outline-none transition-colors placeholder:text-muted focus:border-accent"
        />
      </div>

      {groups.length === 0 ? (
        <Card className="px-6 py-14 text-center text-[13px] text-muted">
          {entries.length === 0
            ? `Nothing here yet. Hold your shortcut and start talking${IS_MAC ? ", or drop a recording here to transcribe it" : ""}.`
            : "No dictations match your search."}
        </Card>
      ) : (
        <div className="flex flex-col gap-6">
          {groups.map(([label, items]) => (
            <section key={label}>
              <h2 className="mb-2 px-1 text-[12px] font-semibold uppercase tracking-wide text-muted">{label}</h2>
              <Card className="divide-y divide-line overflow-hidden">
                <AnimatePresence initial={false}>
                  {items.map((entry) => (
                    <EntryRow key={entry.id} entry={entry} settings={settings} open={open.has(entry.id)} onToggle={() => toggle(entry.id)} />
                  ))}
                </AnimatePresence>
              </Card>
            </section>
          ))}
        </div>
      )}
    </div>
  );
}

function EntryRow({ entry, settings, open, onToggle }: { entry: Entry; settings: Settings; open: boolean; onToggle: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reprocess = async (mode: string) => {
    setBusy(true);
    setError(null);
    try {
      await api.reprocess(entry.id, mode);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <motion.div layout exit={{ opacity: 0, height: 0 }} className="group flex items-start gap-4 px-4 py-3.5">
      <div className="min-w-0 flex-1">
        <p className={cx("selectable whitespace-pre-wrap text-[13.5px] leading-relaxed", busy && "opacity-40")}>{entry.text}</p>
        <p className="mt-1.5 text-[11.5px] tabular-nums text-muted">
          {formatTime(entry.createdAt)} · {entry.words} words
          {entry.source && ` · from ${entry.source}`}
          {!entry.source && entryWpm(entry) > 0 && ` · ${entryWpm(entry)} wpm`}
          {entry.latencyMs != null && ` · typed in ${formatLatency(entry.latencyMs)}`}
          {entry.mode && entry.mode !== "Default" && ` · ${entry.mode}`}
          {entry.style && ` · ${entry.style[0].toUpperCase()}${entry.style.slice(1)} style`}
          {entry.language && entry.language !== "en" && ` · ${languageName(entry.language)}`}
          <button onClick={onToggle} className="ml-2 inline-flex items-center gap-0.5 rounded px-1 text-fg/70 transition-colors hover:text-fg">
            {open ? "Less" : "Details"}
            <ChevronDown className={cx("size-3 transition-transform", open && "rotate-180")} />
          </button>
        </p>
        {error && <p className="mt-1.5 text-[12px] text-danger">{error}</p>}
        {open && <Details entry={entry} settings={settings} busy={busy} onReprocess={reprocess} />}
      </div>
      <div className="flex opacity-0 transition-opacity group-hover:opacity-100">
        {entry.audio && <AudioButton id={entry.id} />}
        <CopyButton text={entry.text} />
        <button
          title="Delete"
          onClick={() => api.deleteEntry(entry.id)}
          className="grid size-8 place-items-center rounded-lg text-muted transition-colors hover:bg-raised hover:text-danger"
        >
          <Trash2 className="size-4" />
        </button>
      </div>
    </motion.div>
  );
}

/** Where the time went, what the cleanup was told, and running it again. */
function Details({
  entry,
  settings,
  busy,
  onReprocess,
}: {
  entry: Entry;
  settings: Settings;
  busy: boolean;
  onReprocess: (mode: string) => void;
}) {
  const [mode, setMode] = useState(settings.activeMode);
  const timings = [
    entry.transcribeMs != null && `transcribed in ${formatLatency(entry.transcribeMs)}`,
    entry.enhanceMs != null && `cleaned up in ${formatLatency(entry.enhanceMs)}`,
    entry.durationMs > 0 && !entry.source && `${(entry.durationMs / 1000).toFixed(1)} s of talking`,
  ].filter(Boolean);

  return (
    <div className="mt-3 space-y-2.5 rounded-xl border border-line bg-raised/40 p-3 text-[12px]">
      {entry.raw && (
        <div>
          <p className="font-medium text-muted">Before cleanup</p>
          <p className="selectable mt-1 whitespace-pre-wrap leading-relaxed">{entry.raw}</p>
        </div>
      )}
      {timings.length > 0 && (
        <p className="text-muted">
          <Info className="mr-1 inline size-3" />
          {timings.join(" · ")}
          {entry.app && ` · into ${entry.app}`}
        </p>
      )}
      {entry.context && entry.context.length > 0 && (
        <p className="text-muted">Used {entry.context.map((c) => CONTEXT_NAMES[c] ?? c).join(", ")}.</p>
      )}
      {entry.prompt && (
        <div>
          <p className="font-medium text-muted">What the cleanup was asked</p>
          <pre className="selectable mt-1 max-h-48 overflow-auto whitespace-pre-wrap rounded-lg bg-bg/60 p-2 font-sans text-[11.5px] leading-relaxed text-fg/80">
            {entry.prompt}
          </pre>
        </div>
      )}
      <div className="flex items-center gap-2 pt-0.5">
        <span className="text-muted">{entry.audio ? "Transcribe again with" : "Clean up again with"}</span>
        <select
          value={mode}
          onChange={(event) => setMode(event.target.value)}
          className="h-7 rounded-md border border-line bg-raised px-2 text-[12px] outline-none"
        >
          {settings.modes.map((m) => (
            <option key={m.id} value={m.id}>
              {m.name}
            </option>
          ))}
        </select>
        <Button variant="secondary" className="h-7 px-2.5 text-[12px]" disabled={busy} onClick={() => onReprocess(mode)}>
          <RotateCcw className={cx("size-3.5", busy && "animate-spin")} />
          {busy ? "Working…" : "Run"}
        </Button>
      </div>
    </div>
  );
}

/** Plays a kept recording, loaded only when asked for. */
function AudioButton({ id }: { id: number }) {
  const audio = useRef<HTMLAudioElement | null>(null);
  const [playing, setPlaying] = useState(false);

  useEffect(() => () => audio.current?.pause(), []);

  const toggle = async () => {
    if (playing) {
      audio.current?.pause();
      return;
    }
    if (!audio.current) {
      const url = await api.entryAudio(id).catch(() => null);
      if (!url) return;
      audio.current = new Audio(url);
      audio.current.onpause = () => setPlaying(false);
      audio.current.onended = () => setPlaying(false);
      audio.current.onplay = () => setPlaying(true);
    }
    audio.current.play().catch(() => setPlaying(false));
  };

  return (
    <button
      title={playing ? "Pause" : "Play the recording"}
      onClick={toggle}
      className="grid size-8 place-items-center rounded-lg text-muted transition-colors hover:bg-raised hover:text-fg"
    >
      {playing ? <Pause className="size-4" /> : <Play className="size-4" />}
    </button>
  );
}
