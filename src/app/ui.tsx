import { listen } from "@tauri-apps/api/event";
import { Check, Copy, Keyboard, Plus, X } from "lucide-react";
import { motion } from "motion/react";
import { useEffect, useState, type ButtonHTMLAttributes, type ReactNode } from "react";
import {
  api,
  hotkeyFromCodes,
  hotkeyLabels,
  hotkeyProblem,
  HOTKEY_PRESETS,
  keyLabel,
  sameHotkey,
  APP_STYLES,
  type AppRule,
  type AppStyle,
  type Hotkey,
  type Snippet,
  type Suggestion,
  type Transform,
} from "../lib/api";
import { cx } from "../lib/utils";

const BUTTON_VARIANTS = {
  primary: "bg-fg text-bg hover:bg-white",
  secondary: "border border-line bg-raised/60 text-fg hover:bg-raised",
  ghost: "text-muted hover:bg-raised hover:text-fg",
  danger: "border border-danger/30 bg-danger/10 text-danger hover:bg-danger/15",
};

export function Button({
  variant = "primary",
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: keyof typeof BUTTON_VARIANTS }) {
  return (
    <button
      className={cx(
        "inline-flex h-9 items-center justify-center gap-2 rounded-lg px-3.5 text-[13px] font-medium",
        "transition-all duration-150 active:scale-[0.98] disabled:pointer-events-none disabled:opacity-40",
        BUTTON_VARIANTS[variant],
        className,
      )}
      {...props}
    />
  );
}

export function Keys({
  hotkey,
  labels,
  size = "md",
  active = false,
}: {
  hotkey?: Hotkey;
  labels?: string[];
  size?: "sm" | "md" | "lg";
  active?: boolean;
}) {
  const keys = labels ?? (hotkey ? hotkeyLabels(hotkey) : []);
  return (
    <span className="inline-flex items-center gap-1.5">
      {keys.map((key, index) => (
        <kbd
          key={`${key}-${index}`}
          className={cx(
            "inline-flex items-center justify-center rounded-lg border bg-linear-to-b from-raised to-surface font-sans font-medium",
            "border-b-2 transition-colors duration-150",
            size === "sm" && "h-6 min-w-6 rounded-md px-1.5 text-[11px]",
            size === "md" && "h-8 min-w-8 px-2.5 text-[13px]",
            size === "lg" && "h-14 min-w-14 rounded-xl px-5 text-xl",
            active ? "border-fg/70 text-fg shadow-[0_0_24px_-6px_rgb(255_255_255/0.35)]" : "border-line text-fg/90",
          )}
        >
          {key}
        </kbd>
      ))}
    </span>
  );
}

export function Card({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cx("rounded-2xl border border-line bg-surface", className)}>{children}</div>;
}

export function Toggle({
  checked,
  onChange,
  disabled = false,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cx(
        "flex h-6 w-10 shrink-0 items-center rounded-full p-0.5 transition-colors duration-200 disabled:opacity-40",
        checked ? "justify-end bg-fg" : "justify-start bg-line",
      )}
    >
      <motion.span
        layout
        transition={{ type: "spring", stiffness: 600, damping: 35 }}
        className={cx("size-5 rounded-full shadow", checked ? "bg-bg" : "bg-muted")}
      />
    </button>
  );
}

/** A row of choices where exactly one is picked, for settings a toggle can't express. */
export function Segmented<T extends string>({
  options,
  value,
  onChange,
  disabled = false,
}: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (next: T) => void;
  disabled?: boolean;
}) {
  return (
    <div className={cx("flex shrink-0 gap-0.5 rounded-lg bg-raised/60 p-0.5", disabled && "opacity-40")}>
      {options.map((option) => (
        <button
          key={option.value}
          disabled={disabled}
          onClick={() => onChange(option.value)}
          className={cx(
            "relative h-7 rounded-md px-2.5 text-[12px] font-medium transition-colors",
            option.value === value ? "text-fg" : "text-muted hover:text-fg",
          )}
        >
          {option.value === value && (
            <motion.span
              layoutId="segmented-active"
              className="absolute inset-0 rounded-md bg-surface ring-1 ring-white/[0.06]"
              transition={{ type: "spring", stiffness: 500, damping: 38 }}
            />
          )}
          <span className="relative">{option.label}</span>
        </button>
      ))}
    </div>
  );
}

export function Logo({ size = 28 }: { size?: number }) {
  return (
    <div
      className="grid shrink-0 place-items-center rounded-[30%] bg-fg text-bg"
      style={{ width: size, height: size }}
    >
      <svg viewBox="0 0 24 24" width={size * 0.64} height={size * 0.64} fill="none" stroke="currentColor" strokeLinecap="round">
        <circle cx="12" cy="9.75" r="5.5" strokeWidth={2.6} />
        <path d="M6.5 9.75v10" strokeWidth={2.6} />
        <path d="M10 8.65v2.2M12 7.55v4.4M14 8.65v2.2" strokeWidth={1.5} />
      </svg>
    </div>
  );
}

export function StatusDot({ tone }: { tone: "ok" | "warn" | "live" | "muted" }) {
  return (
    <span
      className={cx(
        "inline-block size-2 shrink-0 rounded-full",
        tone === "ok" && "bg-ok",
        tone === "warn" && "bg-warn",
        tone === "live" && "animate-pulse bg-live",
        tone === "muted" && "bg-muted/50",
      )}
    />
  );
}

export function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      title="Copy"
      onClick={async () => {
        await api.copyText(text);
        setCopied(true);
        setTimeout(() => setCopied(false), 1200);
      }}
      className="grid size-8 place-items-center rounded-lg text-muted transition-colors hover:bg-raised hover:text-fg"
    >
      {copied ? <Check className="size-4 text-fg" /> : <Copy className="size-4" />}
    </button>
  );
}

export function PageHeader({ title, subtitle, action }: { title: string; subtitle?: string; action?: ReactNode }) {
  return (
    <header className="mb-6 flex items-end justify-between gap-4">
      <div>
        <h1 className="text-[26px] font-semibold tracking-tight">{title}</h1>
        {subtitle && <p className="mt-1 text-[13px] text-muted">{subtitle}</p>}
      </div>
      {action}
    </header>
  );
}

export function HotkeyPicker({ value, onChange }: { value: Hotkey; onChange: (hotkey: Hotkey) => void }) {
  const [recording, setRecording] = useState(false);
  const custom = !HOTKEY_PRESETS.some((preset) => sameHotkey(preset.hotkey, value));

  return (
    <div className="space-y-2">
      <div className="grid grid-cols-2 gap-2">
        {HOTKEY_PRESETS.map((preset) => {
          const selected = !recording && sameHotkey(preset.hotkey, value);
          return (
            <button
              key={preset.label}
              onClick={() => {
                setRecording(false);
                onChange(preset.hotkey);
              }}
              className={cx(
                "flex items-center gap-3 rounded-xl border px-3 py-2.5 text-left transition-all",
                selected ? "border-fg/50 bg-accent-soft" : "border-line hover:bg-raised",
              )}
            >
              <Keys hotkey={preset.hotkey} size="sm" active={selected} />
              <span className="min-w-0 flex-1">
                <span className="block text-[13px] font-medium">{preset.label}</span>
                <span className="block truncate text-[11.5px] text-muted">{preset.hint}</span>
              </span>
              {selected && <Check className="size-4 shrink-0 text-fg" />}
            </button>
          );
        })}
      </div>

      {recording ? (
        <HotkeyRecorder
          onCancel={() => setRecording(false)}
          onPick={(hotkey) => {
            setRecording(false);
            onChange(hotkey);
          }}
        />
      ) : (
        <button
          onClick={() => setRecording(true)}
          className={cx(
            "flex w-full items-center gap-3 rounded-xl border px-3 py-2.5 text-left transition-all",
            custom ? "border-fg/50 bg-accent-soft" : "border-line hover:bg-raised",
          )}
        >
          {custom ? <Keys hotkey={value} size="sm" active /> : <Keyboard className="size-5 shrink-0 text-muted" />}
          <span className="min-w-0 flex-1">
            <span className="block text-[13px] font-medium">{custom ? "Your shortcut" : "Record your own"}</span>
            <span className="block truncate text-[11.5px] text-muted">
              {custom ? "Tap to record a different one" : "Hold any keys and Parla will use them"}
            </span>
          </span>
          {custom && <Check className="size-4 shrink-0 text-fg" />}
        </button>
      )}
    </div>
  );
}

/**
 * The key watcher reports what is held instead of the webview reading key events, so a
 * lone Option or the Globe key records the same way as a combination.
 */
function HotkeyRecorder({ onPick, onCancel }: { onPick: (hotkey: Hotkey) => void; onCancel: () => void }) {
  const [codes, setCodes] = useState<number[]>([]);
  const [settled, setSettled] = useState(false);
  const [eitherSide, setEitherSide] = useState(true);

  useEffect(() => {
    api.setHotkeyCapture(true).catch(() => {});
    const unlisten = listen<{ codes: number[]; done: boolean }>("hotkey-capture", ({ payload }) => {
      setCodes(payload.codes);
      setSettled(payload.done);
    });
    return () => {
      api.setHotkeyCapture(false).catch(() => {});
      unlisten.then((stop) => stop());
    };
  }, []);

  const hotkey = hotkeyFromCodes(codes, eitherSide);
  const problem = codes.length ? hotkeyProblem(hotkey) : null;
  const sided = codes.some((code) => [54, 55, 56, 58, 59, 60, 61, 62].includes(code));

  return (
    <div className="rounded-xl border border-fg/50 bg-accent-soft p-3">
      <div className="flex min-h-9 items-center gap-3">
        {codes.length ? (
          <Keys labels={codes.map(keyLabel)} size="sm" active={!settled} />
        ) : (
          <span className="text-[13px] font-medium">Hold the keys you want…</span>
        )}
        {codes.length > 0 && (
          <span className="text-[11.5px] text-muted">{settled ? "Release recorded" : "Listening…"}</span>
        )}
      </div>

      {problem && <p className="mt-2 text-[12px] text-danger">{problem}</p>}

      {sided && (
        <label className="mt-2.5 flex items-center gap-2 text-[12px] text-muted">
          <input
            type="checkbox"
            checked={eitherSide}
            onChange={(event) => setEitherSide(event.target.checked)}
            className="size-3.5 accent-white"
          />
          Accept either side of the modifier
        </label>
      )}

      <div className="mt-3 flex gap-2">
        <Button
          className="h-8 flex-1 text-[12px]"
          disabled={!settled || !!problem || !codes.length}
          onClick={() => onPick(hotkey)}
        >
          Use this shortcut
        </Button>
        <Button variant="secondary" className="h-8 text-[12px]" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  );
}

/** A single optional shortcut, for actions that are off until the user binds a key. */
export function HotkeyField({
  value,
  onChange,
  emptyLabel,
}: {
  value: Hotkey | null;
  onChange: (hotkey: Hotkey | null) => void;
  emptyLabel: string;
}) {
  const [recording, setRecording] = useState(false);

  if (recording) {
    return (
      <div className="w-[268px]">
        <HotkeyRecorder
          onCancel={() => setRecording(false)}
          onPick={(hotkey) => {
            setRecording(false);
            onChange(hotkey);
          }}
        />
      </div>
    );
  }

  return (
    <div className="flex shrink-0 items-center gap-2">
      {value ? <Keys hotkey={value} size="sm" /> : <span className="text-[12px] text-muted">{emptyLabel}</span>}
      <Button variant="secondary" className="h-8 text-[12px]" onClick={() => setRecording(true)}>
        {value ? "Change" : "Set key"}
      </Button>
      {value && (
        <button
          title="Remove"
          onClick={() => onChange(null)}
          className="grid size-8 place-items-center rounded-lg text-muted transition-colors hover:bg-raised hover:text-danger"
        >
          <X className="size-4" />
        </button>
      )}
    </div>
  );
}

/** Free-text list, used for the words Parla should spell the user's way. */
export function TermList({
  terms,
  onChange,
  placeholder,
  limit,
}: {
  terms: string[];
  onChange: (terms: string[]) => void;
  placeholder: string;
  limit: number;
}) {
  const [draft, setDraft] = useState("");
  const full = terms.length >= limit;

  const add = () => {
    const term = draft.trim();
    if (!term || full) return;
    setDraft("");
    if (!terms.some((kept) => kept.toLowerCase() === term.toLowerCase())) onChange([...terms, term]);
  };

  return (
    <div className="px-4 py-3.5">
      <div className="flex gap-2">
        <input
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && add()}
          placeholder={full ? `That's the ${limit}-word limit` : placeholder}
          disabled={full}
          className="selectable h-9 flex-1 rounded-lg border border-line bg-raised/60 px-3 text-[13px] outline-none transition-colors placeholder:text-muted focus:border-accent disabled:opacity-40"
        />
        <Button variant="secondary" className="h-9 px-3" disabled={!draft.trim() || full} onClick={add}>
          <Plus className="size-4" />
        </Button>
      </div>

      {terms.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-1.5">
          {terms.map((term) => (
            <span
              key={term}
              className="group inline-flex items-center gap-1 rounded-lg border border-line bg-raised/60 py-1 pl-2.5 pr-1 text-[12px]"
            >
              <span className="selectable">{term}</span>
              <button
                title="Remove"
                onClick={() => onChange(terms.filter((kept) => kept !== term))}
                className="grid size-5 place-items-center rounded text-muted transition-colors hover:text-danger"
              >
                <X className="size-3" />
              </button>
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

/** Spoken phrases paired with the text typed in their place. */
export function SnippetList({
  snippets,
  onChange,
  limit,
}: {
  snippets: Snippet[];
  onChange: (snippets: Snippet[]) => void;
  limit: number;
}) {
  const [trigger, setTrigger] = useState("");
  const [text, setText] = useState("");
  const full = snippets.length >= limit;
  const clash = snippets.some((kept) => kept.trigger.toLowerCase() === trigger.trim().toLowerCase());
  const ready = trigger.trim() && text.trim() && !full && !clash;

  const add = () => {
    if (!ready) return;
    onChange([...snippets, { trigger: trigger.trim(), text }]);
    setTrigger("");
    setText("");
  };

  const field =
    "selectable rounded-lg border border-line bg-raised/60 px-3 text-[13px] outline-none transition-colors placeholder:text-muted focus:border-accent disabled:opacity-40";

  return (
    <div className="px-4 py-3.5">
      <div className="flex items-start gap-2">
        <input
          value={trigger}
          onChange={(event) => setTrigger(event.target.value)}
          placeholder={full ? `That's the ${limit}-snippet limit` : "When I say… (my email)"}
          disabled={full}
          maxLength={48}
          className={cx(field, "h-9 w-44 shrink-0")}
        />
        <textarea
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && (event.metaKey || event.ctrlKey) && add()}
          placeholder="…type this instead"
          disabled={full}
          rows={1}
          maxLength={2000}
          className={cx(field, "min-h-9 flex-1 resize-y py-2 leading-snug")}
        />
        <Button variant="secondary" className="h-9 px-3" disabled={!ready} onClick={add} title="Add snippet (⌘↩)">
          <Plus className="size-4" />
        </Button>
      </div>
      {clash && trigger.trim() && <p className="mt-2 text-[12px] text-danger">You already have a snippet for that phrase.</p>}

      {snippets.length > 0 && (
        <div className="mt-3 flex flex-col gap-1.5">
          {snippets.map((snippet) => (
            <div
              key={snippet.trigger}
              className="flex items-start gap-3 rounded-lg border border-line bg-raised/60 py-2 pl-3 pr-1.5 text-[12.5px]"
            >
              <span className="selectable w-40 shrink-0 truncate font-medium">“{snippet.trigger}”</span>
              <span className="selectable line-clamp-2 flex-1 whitespace-pre-wrap text-muted">{snippet.text}</span>
              <button
                title="Remove"
                onClick={() => onChange(snippets.filter((kept) => kept.trigger !== snippet.trigger))}
                className="grid size-5 shrink-0 place-items-center rounded text-muted transition-colors hover:text-danger"
              >
                <X className="size-3" />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

const FIELD =
  "selectable rounded-lg border border-line bg-raised/60 px-3 text-[13px] outline-none transition-colors placeholder:text-muted focus:border-accent disabled:opacity-40";

function RemoveButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      title="Remove"
      onClick={onClick}
      className="grid size-5 shrink-0 place-items-center rounded text-muted transition-colors hover:text-danger"
    >
      <X className="size-3" />
    </button>
  );
}

/** Per-app writing styles. Recent apps from history are offered as you type. */
export function AppRuleList({
  rules,
  onChange,
  limit,
  recentApps,
  disabled,
}: {
  rules: AppRule[];
  onChange: (rules: AppRule[]) => void;
  limit: number;
  recentApps: string[];
  disabled?: boolean;
}) {
  const [app, setApp] = useState("");
  const [style, setStyle] = useState<AppStyle>("casual");
  const full = rules.length >= limit;
  const clash = rules.some((kept) => kept.app.toLowerCase() === app.trim().toLowerCase());
  const ready = app.trim() && !full && !clash && !disabled;

  const add = () => {
    if (!ready) return;
    onChange([...rules, { app: app.trim(), style }]);
    setApp("");
  };

  return (
    <div className="px-4 py-3.5">
      <div className="flex items-center gap-2">
        <input
          value={app}
          list="parla-recent-apps"
          onChange={(event) => setApp(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && add()}
          placeholder={full ? `That's the ${limit}-app limit` : "App or website (Slack, Mail, mail.google.com…)"}
          disabled={full || disabled}
          maxLength={64}
          className={cx(FIELD, "h-9 flex-1")}
        />
        <datalist id="parla-recent-apps">
          {recentApps.map((name) => (
            <option key={name} value={name} />
          ))}
        </datalist>
        <select
          value={style}
          disabled={disabled}
          onChange={(event) => setStyle(event.target.value as AppStyle)}
          className="h-9 rounded-lg border border-line bg-raised/60 px-2.5 text-[12px] outline-none focus:border-accent disabled:opacity-40"
        >
          {APP_STYLES.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        <Button variant="secondary" className="h-9 px-3" disabled={!ready} onClick={add} title="Add rule">
          <Plus className="size-4" />
        </Button>
      </div>
      {clash && app.trim() && <p className="mt-2 text-[12px] text-danger">That app already has a rule.</p>}

      {rules.length > 0 && (
        <div className="mt-3 flex flex-col gap-1.5">
          {rules.map((rule) => (
            <div
              key={rule.app}
              className="flex items-center gap-3 rounded-lg border border-line bg-raised/60 py-1.5 pl-3 pr-1.5 text-[12.5px]"
            >
              <span className="selectable flex-1 truncate font-medium">{rule.app}</span>
              <select
                value={rule.style}
                disabled={disabled}
                onChange={(event) =>
                  onChange(
                    rules.map((kept) =>
                      kept.app === rule.app ? { ...kept, style: event.target.value as AppStyle } : kept,
                    ),
                  )
                }
                className="h-7 rounded-md border border-line bg-raised px-2 text-[12px] outline-none disabled:opacity-40"
              >
                {APP_STYLES.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
              <RemoveButton onClick={() => onChange(rules.filter((kept) => kept.app !== rule.app))} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** Saved instructions for Command Mode and the menu bar. */
export function TransformList({
  transforms,
  onChange,
  limit,
  disabled,
}: {
  transforms: Transform[];
  onChange: (transforms: Transform[]) => void;
  limit: number;
  disabled?: boolean;
}) {
  const [name, setName] = useState("");
  const [instruction, setInstruction] = useState("");
  const full = transforms.length >= limit;
  const clash = transforms.some((kept) => kept.name.toLowerCase() === name.trim().toLowerCase());
  const ready = name.trim() && instruction.trim() && !full && !clash && !disabled;

  const add = () => {
    if (!ready) return;
    onChange([...transforms, { name: name.trim(), instruction: instruction.trim() }]);
    setName("");
    setInstruction("");
  };

  return (
    <div className="px-4 py-3.5">
      <div className="flex items-start gap-2">
        <input
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder={full ? `That's the ${limit} limit` : "Name (Friendly)"}
          disabled={full || disabled}
          maxLength={32}
          className={cx(FIELD, "h-9 w-40 shrink-0")}
        />
        <textarea
          value={instruction}
          onChange={(event) => setInstruction(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && (event.metaKey || event.ctrlKey) && add()}
          placeholder="What to do (Rewrite this so it sounds warm and friendly.)"
          disabled={full || disabled}
          rows={1}
          maxLength={300}
          className={cx(FIELD, "min-h-9 flex-1 resize-y py-2 leading-snug")}
        />
        <Button variant="secondary" className="h-9 px-3" disabled={!ready} onClick={add} title="Add transform (⌘↩)">
          <Plus className="size-4" />
        </Button>
      </div>
      {clash && name.trim() && <p className="mt-2 text-[12px] text-danger">You already have one with that name.</p>}

      {transforms.length > 0 && (
        <div className="mt-3 flex flex-col gap-1.5">
          {transforms.map((transform) => (
            <div
              key={transform.name}
              className="flex items-start gap-3 rounded-lg border border-line bg-raised/60 py-2 pl-3 pr-1.5 text-[12.5px]"
            >
              <span className="selectable w-36 shrink-0 truncate font-medium">{transform.name}</span>
              <span className="selectable line-clamp-2 flex-1 text-muted">{transform.instruction}</span>
              <RemoveButton onClick={() => onChange(transforms.filter((kept) => kept.name !== transform.name))} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** Words the user keeps fixing after a paste, each one click from the dictionary. */
export function SuggestionList({
  suggestions,
  onAccept,
  onDismiss,
  disabled,
}: {
  suggestions: Suggestion[];
  onAccept: (word: string) => void;
  onDismiss: (word: string) => void;
  disabled?: boolean;
}) {
  if (suggestions.length === 0) return null;
  return (
    <div className="px-4 py-3.5">
      <p className="text-[11.5px] font-medium uppercase tracking-wide text-muted">You keep fixing these</p>
      <div className="mt-2 flex flex-wrap gap-1.5">
        {suggestions.map((suggestion) => (
          <span
            key={suggestion.word}
            title={`Parla typed “${suggestion.heard}” and you changed it${suggestion.count > 1 ? ` ${suggestion.count} times` : ""}.`}
            className="inline-flex items-center gap-1.5 rounded-lg border border-line bg-raised/60 py-1 pl-2.5 pr-1 text-[12px]"
          >
            <span className="selectable font-medium">{suggestion.word}</span>
            <span className="text-muted line-through decoration-white/25">{suggestion.heard}</span>
            <button
              title="Add to your words"
              disabled={disabled}
              onClick={() => onAccept(suggestion.word)}
              className="grid size-5 place-items-center rounded text-muted transition-colors hover:text-ok disabled:opacity-40"
            >
              <Check className="size-3" />
            </button>
            <RemoveButton onClick={() => onDismiss(suggestion.word)} />
          </span>
        ))}
      </div>
    </div>
  );
}
