import { listen } from "@tauri-apps/api/event";
import { AppWindow, ClipboardPaste, TextSelect } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { PASTE_KEYS, type DictationEvent } from "../lib/api";
import { cx } from "../lib/utils";
import Orb from "./Orb";

type View =
  | { kind: "hidden" }
  | { kind: "recording"; command: boolean }
  | { kind: "processing"; command: boolean; label?: string }
  | { kind: "message"; text: string; tone: "error" | "muted" };

const CONTEXT_ICONS: Record<string, typeof AppWindow> = {
  selection: TextSelect,
  clipboard: ClipboardPaste,
  app: AppWindow,
};

export default function Overlay() {
  const [view, setView] = useState<View>({ kind: "hidden" });
  const [level, setLevel] = useState(0);
  const [partial, setPartial] = useState("");
  const [mode, setMode] = useState<string | null>(null);
  const [context, setContext] = useState<string[]>([]);
  const [notice, setNotice] = useState<string | null>(null);
  const hideTimer = useRef<number | undefined>(undefined);
  const noticeTimer = useRef<number | undefined>(undefined);
  const command = useRef(false);

  useEffect(() => {
    const flash = (text: string, tone: "error" | "muted", ms: number) => {
      setView({ kind: "message", text, tone });
      hideTimer.current = window.setTimeout(() => setView({ kind: "hidden" }), ms);
    };

    const offDictation = listen<DictationEvent>("dictation", ({ payload }) => {
      window.clearTimeout(hideTimer.current);
      switch (payload.phase) {
        case "recording":
          command.current = payload.command;
          setLevel(0);
          setPartial("");
          setMode(null);
          setContext([]);
          setNotice(null);
          setView({ kind: "recording", command: payload.command });
          break;
        case "processing":
          setView({ kind: "processing", command: command.current });
          break;
        case "working":
          command.current = false;
          setView({ kind: "processing", command: false, label: payload.label });
          break;
        case "notice":
          flash(payload.text, "muted", 3200);
          break;
        case "done":
        case "cancelled":
          setView({ kind: "hidden" });
          break;
        case "copied":
          flash(`Copied — press ${PASTE_KEYS} to paste`, "muted", 2200);
          break;
        case "empty":
          flash("Didn't catch that", "muted", 1400);
          break;
        case "undone":
          flash("Took that back", "muted", 1400);
          break;
        case "blocked":
          flash(`Not here — ${payload.reason}`, "muted", 2600);
          break;
        case "error":
          flash(payload.message, "error", 3500);
          break;
      }
    });

    // Speech RMS mostly sits well under 0.1, so a square-root curve keeps quiet
    // voices visible without letting loud ones clip.
    const offLevel = listen<number>("level", ({ payload }) => {
      setLevel(Math.min(1, Math.sqrt(payload) * 3));
    });

    const offPartial = listen<string>("partial", ({ payload }) => setPartial(payload));
    const offMode = listen<string>("mode", ({ payload }) => setMode(payload));
    const offContext = listen<string[]>("context", ({ payload }) => setContext(payload));
    // A note while recording, like "press Esc again", shows in place of "Listening".
    const offNotice = listen<string>("notice", ({ payload }) => {
      window.clearTimeout(noticeTimer.current);
      setNotice(payload);
      noticeTimer.current = window.setTimeout(() => setNotice(null), 3000);
    });

    return () => {
      window.clearTimeout(hideTimer.current);
      offDictation.then((stop) => stop());
      offLevel.then((stop) => stop());
      offPartial.then((stop) => stop());
      offMode.then((stop) => stop());
      offContext.then((stop) => stop());
      offNotice.then((stop) => stop());
      window.clearTimeout(noticeTimer.current);
    };
  }, []);

  return (
    <div className="flex h-screen items-end justify-center pb-2">
      <AnimatePresence>
        {view.kind !== "hidden" && (
          <motion.div
            key="pill"
            layout
            initial={{ opacity: 0, y: 14, scale: 0.85 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 10, scale: 0.9 }}
            transition={{ type: "spring", stiffness: 480, damping: 34 }}
            className="flex h-11 items-center gap-2.5 rounded-full bg-[#141415]/95 pl-2 pr-4 text-white shadow-xl shadow-black/40 ring-1 ring-white/[0.08]"
          >
            {view.kind === "recording" && (
              <>
                <Orb level={level} command={view.command} />
                {view.command && <span className="text-xs font-medium text-white/75">Command</span>}
                {mode && !view.command && (
                  <span className="rounded-full bg-white/10 px-2 py-0.5 text-[11px] font-medium text-white/85">{mode}</span>
                )}
                {context.map((kind) => {
                  const Icon = CONTEXT_ICONS[kind];
                  return Icon ? <Icon key={kind} className="size-3.5 text-white/60" aria-label={`Using ${kind}`} /> : null;
                })}
                {/* The newest words matter most, so a long preview keeps its end. */}
                {notice ? (
                  <span className="text-xs font-medium text-[#f0c27a]">{notice}</span>
                ) : partial ? (
                  <span className="max-w-[230px] truncate text-xs text-white/85">
                    {partial.length > 42 ? `…${partial.slice(-42)}` : partial}
                  </span>
                ) : (
                  !view.command && <span className="text-xs font-medium text-white/60">Listening</span>
                )}
              </>
            )}

            {view.kind === "processing" && (
              <>
                <Orb level={0} busy command={view.command} />
                <span className="max-w-[260px] truncate text-xs font-medium text-white/75">
                  {view.label ?? (view.command ? "Rewriting" : mode ? `Writing as ${mode}` : "Transcribing")}
                </span>
              </>
            )}

            {view.kind === "message" && (
              <span className={cx("max-w-[310px] truncate text-xs font-medium", view.tone === "error" ? "text-[#f0a39d]" : "text-white/75")}>
                {view.text}
              </span>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
