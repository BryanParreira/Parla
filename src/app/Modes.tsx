import { Check, Plus, Trash2, X } from "lucide-react";
import { useState } from "react";
import {
  CLEANUP_LEVELS,
  IS_MAC,
  languageName,
  MAX_MODES,
  MODE_PRESETS,
  newMode,
  WHISPER_LANGUAGES,
  type CleanupLevel,
  type Entry,
  type Mode,
  type ModePreset,
  type Settings,
} from "../lib/api";
import { cx } from "../lib/utils";
import { Button, Card, HotkeyField, PageHeader, Row, Section, Toggle } from "./ui";

const FIELD =
  "selectable rounded-lg border border-line bg-raised/60 px-3 text-[13px] outline-none transition-colors placeholder:text-muted focus:border-accent disabled:opacity-40";
const SELECT =
  "h-8 max-w-[220px] rounded-lg border border-line bg-raised/60 px-2.5 text-[12px] outline-none transition-colors focus:border-accent";

/** Starting points for a custom mode's instructions. */
const CUSTOM_IDEAS = [
  "Turn what I say into a short, friendly reply to the message in the clipboard.",
  "Write it as a git commit message: a short summary line, a blank line, then the why.",
  "Translate what I say into Spanish. Keep it natural, not word for word.",
  "Turn my rough outline into a finished paragraph in my own voice.",
];

export default function ModesPage({
  settings,
  update,
  entries,
}: {
  settings: Settings;
  update: (patch: Partial<Settings>) => Promise<void>;
  entries: Entry[];
}) {
  const [selected, setSelected] = useState(settings.activeMode);
  const [error, setError] = useState<string | null>(null);
  const mode = settings.modes.find((m) => m.id === selected) ?? settings.modes[0];
  const used = (name: string) => entries.filter((e) => (e.mode ?? "Default") === name).length;
  const recentApps = [...new Set(entries.map((e) => e.app).filter((a): a is string => !!a))].slice(0, 20);

  const save = async (patch: Partial<Settings>) => {
    setError(null);
    try {
      await update(patch);
    } catch (reason) {
      setError(String(reason));
    }
  };

  const change = (next: Partial<Mode>) =>
    save({ modes: settings.modes.map((m) => (m.id === mode.id ? { ...m, ...next } : m)) });

  const add = async (preset: ModePreset) => {
    const created = newMode(preset, settings.modes);
    await save({ modes: [...settings.modes, created] });
    setSelected(created.id);
  };

  const remove = async () => {
    const modes = settings.modes.filter((m) => m.id !== mode.id);
    await save({ modes, activeMode: settings.activeMode === mode.id ? "default" : settings.activeMode });
    setSelected("default");
  };

  return (
    <div>
      <PageHeader
        title="Modes"
        subtitle="Each mode shapes your words for a job. Pick one here, from the menu bar, with its own shortcut, or let an app or website pick it."
      />

      <div className="mb-6 grid grid-cols-[200px_1fr] gap-4">
        <Card className="flex flex-col gap-0.5 self-start p-1.5">
          {settings.modes.map((m) => (
            <button
              key={m.id}
              onClick={() => setSelected(m.id)}
              className={cx(
                "flex items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[13px] transition-colors",
                m.id === mode.id ? "bg-raised text-fg" : "text-muted hover:text-fg",
              )}
            >
              <span className="min-w-0 flex-1 truncate font-medium">{m.name}</span>
              {m.id === settings.activeMode && <Check className="size-3.5 shrink-0" />}
            </button>
          ))}
          <div className="mt-1 border-t border-line pt-1.5">
            <select
              value=""
              disabled={settings.modes.length >= MAX_MODES}
              onChange={(event) => event.target.value && add(event.target.value as ModePreset)}
              className={cx(SELECT, "w-full max-w-none")}
            >
              <option value="">{settings.modes.length >= MAX_MODES ? "That's the limit" : "+ New mode…"}</option>
              {MODE_PRESETS.filter((p) => p.value !== "default").map((p) => (
                <option key={p.value} value={p.value}>
                  {p.label}
                </option>
              ))}
            </select>
          </div>
        </Card>

        <ModeEditor
          key={mode.id}
          mode={mode}
          active={mode.id === settings.activeMode}
          uses={used(mode.name)}
          recentApps={recentApps}
          onChange={change}
          onActivate={() => save({ activeMode: mode.id })}
          onRemove={mode.id === "default" ? undefined : remove}
        />
      </div>
      {error && <p className="-mt-3 mb-4 text-[12px] text-danger">{error}</p>}

      <Section title="Switching modes">
        <Row
          label="Mode switch key"
          description="Press it to move to the next mode. While you're dictating it switches that dictation, so you can change your mind mid-sentence."
        >
          <HotkeyField value={settings.modeHotkey} emptyLabel="Not set" onChange={(modeHotkey) => save({ modeHotkey })} />
        </Row>
        {IS_MAC && (
          <Row
            label="From scripts and launchers"
            description={
              <>
                Shortcuts, Raycast and Alfred can open <code className="selectable">parla://mode?name=Email</code>,{" "}
                <code className="selectable">parla://record</code>, <code className="selectable">parla://record/start</code> or{" "}
                <code className="selectable">parla://record/stop</code>.
              </>
            }
          />
        )}
      </Section>
    </div>
  );
}

function ModeEditor({
  mode,
  active,
  uses,
  recentApps,
  onChange,
  onActivate,
  onRemove,
}: {
  mode: Mode;
  active: boolean;
  uses: number;
  recentApps: string[];
  onChange: (next: Partial<Mode>) => void;
  onActivate: () => void;
  onRemove?: () => void;
}) {
  const [name, setName] = useState(mode.name);
  const [instructions, setInstructions] = useState(mode.instructions);
  const preset = MODE_PRESETS.find((p) => p.value === mode.preset);
  const isDefault = mode.id === "default";
  const cleans = mode.preset !== "voice";

  return (
    <Card className="divide-y divide-line">
      <div className="flex items-center gap-3 px-4 py-3.5">
        <input
          value={name}
          disabled={isDefault}
          maxLength={32}
          onChange={(event) => setName(event.target.value)}
          onBlur={() => name.trim() && name !== mode.name && onChange({ name: name.trim() })}
          onKeyDown={(event) => event.key === "Enter" && (event.target as HTMLInputElement).blur()}
          className="selectable min-w-0 flex-1 bg-transparent text-[17px] font-semibold outline-none disabled:opacity-100"
        />
        <span className="text-[12px] tabular-nums text-muted">{uses} dictations</span>
        {active ? (
          <span className="flex items-center gap-1.5 text-[12px] font-medium">
            <Check className="size-3.5" /> Active
          </span>
        ) : (
          <Button className="h-8 text-[12px]" onClick={onActivate}>
            Use this mode
          </Button>
        )}
        {onRemove && (
          <button
            title="Delete mode"
            onClick={onRemove}
            className="grid size-8 place-items-center rounded-lg text-muted transition-colors hover:bg-raised hover:text-danger"
          >
            <Trash2 className="size-4" />
          </button>
        )}
      </div>

      <Row label="Kind" description={preset?.hint ?? ""}>
        <select
          value={mode.preset}
          disabled={isDefault}
          onChange={(event) => onChange({ preset: event.target.value as ModePreset })}
          className={SELECT}
        >
          {MODE_PRESETS.filter((p) => isDefault || p.value !== "default").map((p) => (
            <option key={p.value} value={p.value}>
              {p.label}
            </option>
          ))}
        </select>
      </Row>

      {cleans && (
        <div className="px-4 py-3.5">
          <p className="text-[13px] font-medium">{mode.preset === "custom" ? "Instructions" : "Extra instructions"}</p>
          <p className="mt-0.5 text-[12px] text-muted">
            {mode.preset === "custom"
              ? "Say what to do with what you dictate. Apple Intelligence follows them on this Mac."
              : "Optional. Anything this mode should always do, like “use British spelling”."}
          </p>
          <textarea
            value={instructions}
            maxLength={1500}
            rows={mode.preset === "custom" ? 4 : 2}
            onChange={(event) => setInstructions(event.target.value)}
            onBlur={() => instructions !== mode.instructions && onChange({ instructions })}
            placeholder={mode.preset === "custom" ? CUSTOM_IDEAS[0] : "Always use British spelling."}
            className={cx(FIELD, "mt-2.5 w-full resize-y py-2 leading-snug")}
          />
          {mode.preset === "custom" && !instructions.trim() && (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {CUSTOM_IDEAS.map((idea) => (
                <button
                  key={idea}
                  onClick={() => {
                    setInstructions(idea);
                    onChange({ instructions: idea });
                  }}
                  className="rounded-lg border border-line px-2 py-1 text-left text-[11.5px] text-muted transition-colors hover:bg-raised hover:text-fg"
                >
                  {idea}
                </button>
              ))}
            </div>
          )}
        </div>
      )}

      {cleans && <ExampleList examples={mode.examples} onChange={(examples) => onChange({ examples })} />}

      {cleans && (
        <div className="px-4 py-3.5">
          <p className="text-[13px] font-medium">Context</p>
          <p className="mt-0.5 text-[12px] text-muted">
            What else the cleanup may read, on this Mac only. Turn on just what the mode needs: extra context can also
            distract it. Never read from password fields.
          </p>
          <div className="mt-2.5 flex flex-col gap-2">
            <ContextBox
              label="Selected text"
              hint="Whatever is selected when you start, so you can say “reply to this” or “summarize it”."
              checked={mode.context.selection}
              onChange={(selection) => onChange({ context: { ...mode.context, selection } })}
            />
            <ContextBox
              label="Copied text"
              hint="Text you copied up to 3 seconds before you start, or while you talk."
              checked={mode.context.clipboard}
              onChange={(clipboard) => onChange({ context: { ...mode.context, clipboard } })}
            />
            <ContextBox
              label="App, window and date"
              hint="The app and its window title, the text before your cursor, today's date and time, and your name."
              checked={mode.context.app}
              onChange={(app) => onChange({ context: { ...mode.context, app } })}
            />
          </div>
        </div>
      )}

      {cleans && (
        <Row label="Write in" description="Translate into this language, whatever you speak. Leave it to keep the language you used.">
          <select
            value={mode.translate ?? ""}
            onChange={(event) => onChange({ translate: event.target.value || null })}
            className={SELECT}
          >
            <option value="">The language I speak</option>
            {WHISPER_LANGUAGES.map((code) => (
              <option key={code} value={code}>
                {languageName(code)}
              </option>
            ))}
          </select>
        </Row>
      )}

      {cleans && mode.preset !== "custom" && (
        <Row label="How much to change" description="Overrides the level in Settings for this mode only.">
          <select
            value={mode.level ?? ""}
            onChange={(event) => onChange({ level: (event.target.value || null) as CleanupLevel | null })}
            className={SELECT}
          >
            <option value="">Same as Settings</option>
            {CLEANUP_LEVELS.map((level) => (
              <option key={level.value} value={level.value}>
                {level.label}
              </option>
            ))}
          </select>
        </Row>
      )}

      <AppList apps={mode.apps} recentApps={recentApps} onChange={(apps) => onChange({ apps })} />

      <Row label="Shortcut" description="Hold it to dictate straight into this mode, whichever one is active.">
        <HotkeyField value={mode.hotkey} emptyLabel="Not set" onChange={(hotkey) => onChange({ hotkey })} />
      </Row>
    </Card>
  );
}

function ContextBox({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint: string;
  checked: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <label className="flex items-start gap-3">
      <span className="mt-0.5">
        <Toggle checked={checked} onChange={onChange} />
      </span>
      <span>
        <span className="block text-[12.5px] font-medium">{label}</span>
        <span className="block text-[12px] text-muted">{hint}</span>
      </span>
    </label>
  );
}

/** Up to three dictations and what they should become. */
function ExampleList({
  examples,
  onChange,
}: {
  examples: Mode["examples"];
  onChange: (examples: Mode["examples"]) => void;
}) {
  const [input, setInput] = useState("");
  const [output, setOutput] = useState("");
  const full = examples.length >= 3;
  const ready = input.trim() && output.trim() && !full;
  const add = () => {
    if (!ready) return;
    onChange([...examples, { input: input.trim(), output: output.trim() }]);
    setInput("");
    setOutput("");
  };

  return (
    <div className="px-4 py-3.5">
      <p className="text-[13px] font-medium">Examples</p>
      <p className="mt-0.5 text-[12px] text-muted">
        Two or three of what you'd say and what you want back teach the mode faster than any instructions.
      </p>
      {examples.map((example, index) => (
        <div key={index} className="mt-2 flex items-start gap-2 rounded-lg border border-line bg-raised/60 py-2 pl-3 pr-1.5 text-[12.5px]">
          <span className="selectable line-clamp-3 flex-1 text-muted">“{example.input}”</span>
          <span className="text-muted">→</span>
          <span className="selectable line-clamp-3 flex-1 whitespace-pre-wrap">{example.output}</span>
          <button
            title="Remove"
            onClick={() => onChange(examples.filter((_, i) => i !== index))}
            className="grid size-5 shrink-0 place-items-center rounded text-muted transition-colors hover:text-danger"
          >
            <X className="size-3" />
          </button>
        </div>
      ))}
      {!full && (
        <div className="mt-2.5 flex items-start gap-2">
          <textarea
            value={input}
            rows={2}
            maxLength={600}
            onChange={(event) => setInput(event.target.value)}
            placeholder="What you'd say"
            className={cx(FIELD, "flex-1 resize-y py-2 leading-snug")}
          />
          <textarea
            value={output}
            rows={2}
            maxLength={600}
            onChange={(event) => setOutput(event.target.value)}
            placeholder="What it should become"
            className={cx(FIELD, "flex-1 resize-y py-2 leading-snug")}
          />
          <Button variant="secondary" className="h-9 px-3" disabled={!ready} onClick={add} title="Add example">
            <Plus className="size-4" />
          </Button>
        </div>
      )}
    </div>
  );
}

/** Apps and websites that switch to a mode by themselves. */
function AppList({
  apps,
  recentApps,
  onChange,
}: {
  apps: string[];
  recentApps: string[];
  onChange: (apps: string[]) => void;
}) {
  const [draft, setDraft] = useState("");
  const add = () => {
    const app = draft.trim();
    if (!app || apps.length >= 20) return;
    setDraft("");
    if (!apps.some((kept) => kept.toLowerCase() === app.toLowerCase())) onChange([...apps, app]);
  };

  return (
    <div className="px-4 py-3.5">
      <p className="text-[13px] font-medium">Use automatically in</p>
      <p className="mt-0.5 text-[12px] text-muted">
        Apps by name, or websites by domain, like Slack or mail.google.com. Dictating there switches to this mode.
      </p>
      <div className="mt-2.5 flex gap-2">
        <input
          value={draft}
          list="parla-mode-apps"
          maxLength={64}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && add()}
          placeholder="App or website, then press Enter"
          className={cx(FIELD, "h-9 flex-1")}
        />
        <datalist id="parla-mode-apps">
          {recentApps.map((name) => (
            <option key={name} value={name} />
          ))}
        </datalist>
        <Button variant="secondary" className="h-9 px-3" disabled={!draft.trim()} onClick={add}>
          <Plus className="size-4" />
        </Button>
      </div>
      {apps.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-1.5">
          {apps.map((app) => (
            <span key={app} className="inline-flex items-center gap-1 rounded-lg border border-line bg-raised/60 py-1 pl-2.5 pr-1 text-[12px]">
              <span className="selectable">{app}</span>
              <button
                title="Remove"
                onClick={() => onChange(apps.filter((kept) => kept !== app))}
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

