import { Download, Search, Trash2 } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useMemo, useState } from "react";
import { api, languageName, type Entry } from "../lib/api";
import { dayLabel, entryWpm, formatLatency, formatTime } from "../lib/utils";
import { Button, Card, CopyButton, PageHeader } from "./ui";

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
    lines.push(`**${formatTime(entry.createdAt)}** · ${entry.words} words`, "", entry.text, "");
  }
  return lines.join("\n");
}

export default function HistoryPage({ entries }: { entries: Entry[] }) {
  const [query, setQuery] = useState("");
  const [confirmClear, setConfirmClear] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);

  const exportHistory = async () => {
    setExportError(null);
    const stamp = new Date().toISOString().slice(0, 10);
    try {
      await api.saveExport(`Parla history ${stamp}.md`, toMarkdown(entries));
    } catch (error) {
      setExportError(String(error));
    }
  };
  const [showingOriginal, setShowingOriginal] = useState<Set<number>>(() => new Set());

  const toggleOriginal = (id: number) =>
    setShowingOriginal((current) => {
      const next = new Set(current);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  const groups = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const matches = needle ? entries.filter((e) => e.text.toLowerCase().includes(needle)) : entries;
    const byDay = new Map<string, Entry[]>();
    for (const entry of matches) {
      const label = dayLabel(entry.createdAt);
      byDay.set(label, [...(byDay.get(label) ?? []), entry]);
    }
    return [...byDay.entries()];
  }, [entries, query]);

  return (
    <div>
      <PageHeader
        title="History"
        subtitle="Everything you've dictated, stored only on this Mac."
        action={
          entries.length > 0 && (
            <div className="flex gap-1.5">
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
            </div>
          )
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
          {entries.length === 0 ? "Nothing here yet. Hold your shortcut and start talking." : "No dictations match your search."}
        </Card>
      ) : (
        <div className="flex flex-col gap-6">
          {groups.map(([label, items]) => (
            <section key={label}>
              <h2 className="mb-2 px-1 text-[12px] font-semibold uppercase tracking-wide text-muted">{label}</h2>
              <Card className="divide-y divide-line overflow-hidden">
                <AnimatePresence initial={false}>
                  {items.map((entry) => (
                    <motion.div
                      key={entry.id}
                      layout
                      exit={{ opacity: 0, height: 0 }}
                      className="group flex items-start gap-4 px-4 py-3.5"
                    >
                      <div className="flex-1">
                        <p className="selectable text-[13.5px] leading-relaxed">{entry.text}</p>
                        {entry.raw && showingOriginal.has(entry.id) && (
                          <p className="selectable mt-1.5 border-l-2 border-line pl-2.5 text-[12.5px] leading-relaxed text-muted">
                            {entry.raw}
                          </p>
                        )}
                        <p className="mt-1.5 text-[11.5px] tabular-nums text-muted">
                          {formatTime(entry.createdAt)} · {entry.words} words
                          {entryWpm(entry) > 0 && ` · ${entryWpm(entry)} wpm`}
                          {entry.latencyMs != null && ` · typed in ${formatLatency(entry.latencyMs)}`}
                          {entry.style && ` · ${entry.style[0].toUpperCase()}${entry.style.slice(1)} style`}
                          {entry.language && entry.language !== "en" && ` · ${languageName(entry.language)}`}
                          {entry.raw && (
                            <button
                              onClick={() => toggleOriginal(entry.id)}
                              className="ml-2 rounded px-1 text-fg/70 transition-colors hover:text-fg"
                            >
                              {showingOriginal.has(entry.id) ? "Hide original" : "Enhanced · show original"}
                            </button>
                          )}
                        </p>
                      </div>
                      <div className="flex opacity-0 transition-opacity group-hover:opacity-100">
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
