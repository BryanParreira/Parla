import { getVersion } from "@tauri-apps/api/app";
import { Check } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { api, LANGUAGE_NAMES, MAX_DICTIONARY_TERMS, MAX_SNIPPETS, type EnhanceStatus, type InputDevice, type Entry, type Hotkey, type ModelState, type ModelStatus, type Permissions, type Settings } from "../lib/api";
import { Button, Card, HotkeyField, HotkeyPicker, PageHeader, SnippetList, StatusDot, TermList, Toggle } from "./ui";

const LANGUAGE_COUNT = Object.keys(LANGUAGE_NAMES).length;
const LANGUAGE_EXAMPLES = ["en", "es", "pt", "fr", "de", "it"].map((code) => LANGUAGE_NAMES[code]).join(", ");

const ENHANCE_HINTS: Record<EnhanceStatus, string> = {
  available: "Removes filler words, fixes grammar and keeps only your corrections. Runs on-device with Apple Intelligence.",
  appleIntelligenceNotEnabled: "Turn on Apple Intelligence in System Settings to use Enhance.",
  modelNotReady: "Apple Intelligence is still downloading its model. Enhance turns on once it's ready.",
  deviceNotEligible: "This Mac doesn't support Apple Intelligence, so text is typed with punctuation only.",
  unsupportedOS: "Enhance needs macOS 26 or later. Text is still typed with punctuation.",
  unavailable: "Apple Intelligence isn't available right now. Text is still typed with punctuation.",
};

export default function SettingsPage({
  settings,
  update,
  model,
  permissions,
  entries,
}: {
  settings: Settings;
  update: (patch: Partial<Settings>) => Promise<void>;
  model: ModelStatus | null;
  permissions: Permissions | null;
  entries: Entry[];
}) {
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);
  const [shortcutError, setShortcutError] = useState<string | null>(null);
  const [version, setVersion] = useState("");
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [devices, setDevices] = useState<InputDevice[]>([]);

  const loadDevices = () => api.inputDevices().then(setDevices).catch(() => {});

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
    api.launchAtLogin().then(setLaunchAtLogin).catch(() => {});
    loadDevices();
  }, []);

  // A saved microphone that is unplugged right now still shows, so the choice isn't
  // silently lost; recording falls back to the system default until it returns.
  const missingDevice = settings.inputDevice && !devices.some((device) => device.uid === settings.inputDevice);

  const enhanceStatus = model?.enhance ?? "available";
  const enhanceAvailable = enhanceStatus === "available";
  const preview = entries.find((entry) => entry.raw && entry.enhanceMs != null);

  const chooseHotkey = async (hotkey: Hotkey) => {
    setHotkeyError(null);
    try {
      await update({ hotkey });
    } catch (error) {
      setHotkeyError(String(error));
    }
  };

  // The backend has the final say on a shortcut, so its refusal is what gets shown.
  const save = async (patch: Partial<Settings>) => {
    setShortcutError(null);
    try {
      await update(patch);
    } catch (error) {
      setShortcutError(String(error));
    }
  };

  return (
    <div>
      <PageHeader title="Settings" />

      <Section
        title="Dictation key"
        description="Hold it to talk, release it to type. Pick one of these or record your own — using it in a shortcut with other keys won't start dictation."
      >
        <div className="p-4">
          <HotkeyPicker value={settings.hotkey} onChange={chooseHotkey} />
        </div>
        {hotkeyError && <p className="px-4 pb-4 text-[12px] text-danger">{hotkeyError}</p>}
      </Section>

      <Section title="Shortcuts">
        <Row
          label="Hands-free"
          description="Tap the dictation key twice to keep recording after you let go. Press it again to stop."
        >
          <Toggle checked={settings.handsFree} onChange={(handsFree) => save({ handsFree })} />
        </Row>
        <Row
          label="Stop when I go quiet"
          description="Ends a hands-free recording after 8 seconds of silence, so a forgotten one doesn't keep listening."
        >
          <Toggle
            checked={settings.autoStopSilence && settings.handsFree}
            disabled={!settings.handsFree}
            onChange={(autoStopSilence) => save({ autoStopSilence })}
          />
        </Row>
        <Row
          label="Paste last dictation again"
          description="For when a transcript landed in the wrong window. Off until you pick a key."
        >
          <HotkeyField
            value={settings.repeatHotkey}
            emptyLabel="Not set"
            onChange={(repeatHotkey) => save({ repeatHotkey })}
          />
        </Row>
        <Row
          label="Command Mode"
          description={
            enhanceAvailable
              ? "Select text, hold this key and say what to change — “make it shorter”, “turn this into a list”."
              : "Needs Apple Intelligence, which isn't available on this Mac."
          }
        >
          <HotkeyField
            value={settings.commandHotkey}
            emptyLabel="Not set"
            onChange={(commandHotkey) => save({ commandHotkey })}
          />
        </Row>
        {shortcutError && <p className="px-4 pb-4 text-[12px] text-danger">{shortcutError}</p>}
      </Section>

      <Section title="General">
        <Row label="Microphone" description="Which input Parla listens to. System default follows whatever macOS is using.">
          <select
            value={settings.inputDevice ?? ""}
            onFocus={loadDevices}
            onChange={(event) => update({ inputDevice: event.target.value || null })}
            className="h-8 max-w-[220px] rounded-lg border border-line bg-raised/60 px-2.5 text-[12px] outline-none transition-colors focus:border-accent"
          >
            <option value="">System default</option>
            {devices.map((device) => (
              <option key={device.uid} value={device.uid}>
                {device.name}
              </option>
            ))}
            {missingDevice && <option value={settings.inputDevice!}>Unplugged microphone</option>}
          </select>
        </Row>
        <Row label="Open at login" description="Start Parla quietly in the menu bar when you log in.">
          <Toggle
            checked={launchAtLogin}
            onChange={(enabled) =>
              api
                .setLaunchAtLogin(enabled)
                .then(() => setLaunchAtLogin(enabled))
                .catch(() => {})
            }
          />
        </Row>
        <Row label="Sound effects" description="Play a soft sound when dictation starts and stops.">
          <Toggle checked={settings.sounds} onChange={(sounds) => update({ sounds })} />
        </Row>
        <Row
          label="Mute other audio"
          description="Silence music and video while you hold the key, then put the volume back exactly as it was."
        >
          <Toggle
            checked={settings.muteOtherAudio}
            onChange={(muteOtherAudio) => update({ muteOtherAudio })}
          />
        </Row>
      </Section>

      <Section title="Permissions">
        <Row label="Microphone" description="Needed to hear you while the shortcut is held.">
          <PermissionControl
            granted={permissions?.microphone === "granted"}
            onFix={() =>
              permissions?.microphone === "undetermined" ? api.requestMicrophone() : api.openPrivacySettings("microphone")
            }
          />
        </Row>
        <Row label="Accessibility" description="Lets Parla paste text into the app you're using.">
          <PermissionControl granted={!!permissions?.accessibility} onFix={api.grantAccessibility} />
        </Row>
      </Section>

      <Section title="Writing" description="Polish what you say before it's typed.">
        <Row label="Enhance" description={ENHANCE_HINTS[enhanceStatus]}>
          <Toggle
            checked={settings.enhance && enhanceAvailable}
            disabled={!enhanceAvailable}
            onChange={(enhance) => update({ enhance })}
          />
        </Row>
        <Row
          label="Skip when it's already clean"
          description="Paste right away when there are no filler words or restarts to fix. About half a second faster. English only; other languages and your words always get the full pass."
        >
          <Toggle
            checked={settings.quickEnhance && settings.enhance && enhanceAvailable}
            disabled={!settings.enhance || !enhanceAvailable}
            onChange={(quickEnhance) => update({ quickEnhance })}
          />
        </Row>
        <Row
          label="Match the app"
          description="Casual in chat, polished in email, hands-off in code editors. Only the app's name is used — never what's on screen."
        >
          <Toggle
            checked={settings.appAwareTone && settings.enhance && enhanceAvailable}
            disabled={!settings.enhance || !enhanceAvailable}
            onChange={(appAwareTone) => update({ appAwareTone })}
          />
        </Row>
        <Row
          label="Your words"
          description={
            enhanceAvailable
              ? "Names, jargon and product spellings Parla keeps getting wrong. Enhance spells these your way."
              : "Needs Enhance, which isn't available on this Mac."
          }
        >
          <span className="text-[12px] tabular-nums text-muted">
            {settings.dictionary.length}/{MAX_DICTIONARY_TERMS}
          </span>
        </Row>
        {enhanceAvailable && (
          <TermList
            terms={settings.dictionary}
            limit={MAX_DICTIONARY_TERMS}
            placeholder="Add a name or word, then press Enter"
            onChange={(dictionary) => save({ dictionary })}
          />
        )}
        {preview && (
          <div className="px-4 py-3.5">
            <p className="text-[11.5px] font-medium uppercase tracking-wide text-muted">
              Last enhancement · {preview.enhanceMs} ms
            </p>
            <p className="selectable mt-2 text-[13px] leading-relaxed text-muted line-through decoration-white/25">
              {preview.raw}
            </p>
            <p className="selectable mt-1.5 text-[13px] leading-relaxed">{preview.text}</p>
          </div>
        )}
      </Section>

      <Section
        title="Snippets"
        description="Say a short phrase and Parla types the saved text instead — your email, an address, a sign-off."
      >
        <SnippetList snippets={settings.snippets} limit={MAX_SNIPPETS} onChange={(snippets) => save({ snippets })} />
      </Section>

      <Section title="Speech models" description="Everything runs on this Mac's Neural Engine. Audio never leaves it.">
        <Row label="Parakeet TDT v3" description="Accurate transcription with punctuation when you release the key.">
          <ModelBadge state={model?.punctuation} progress={model?.punctuationProgress} />
        </Row>
        <Row label="Parakeet streaming" description="Lightweight fallback while the main model loads.">
          <ModelBadge state={model?.streaming} progress={model?.state === "loading" ? model.progress : null} />
        </Row>
        <Row
          label="Live preview"
          description="Show your words in the pill as you speak. Uses more of the Neural Engine while recording, so text can take a moment longer to land."
        >
          <Toggle checked={settings.livePreview} onChange={(livePreview) => update({ livePreview })} />
        </Row>
        <Row
          label="Languages"
          description={`Speak any of ${LANGUAGE_COUNT} European languages — ${LANGUAGE_EXAMPLES} and more. Parla works out which one you used, so there is nothing to switch.`}
        >
          <span className="text-[12px] tabular-nums text-muted">Automatic</span>
        </Row>
      </Section>

      <Section title="About">
        <Row label="Parla" description="Fast, private, on-device dictation.">
          <span className="text-[12px] tabular-nums text-muted">v{version}</span>
        </Row>
        <Row label="Setup guide" description="Walk through permissions and the model again.">
          <Button variant="secondary" className="h-8 text-[12px]" onClick={() => update({ onboarded: false })}>
            Run again
          </Button>
        </Row>
      </Section>
    </div>
  );
}

function Section({ title, description, children }: { title: string; description?: string; children: ReactNode }) {
  return (
    <section className="mb-6">
      <div className="mb-2 px-1">
        <h2 className="text-[13px] font-semibold">{title}</h2>
        {description && <p className="text-[12px] text-muted">{description}</p>}
      </div>
      <Card className="divide-y divide-line">{children}</Card>
    </section>
  );
}

function Row({ label, description, children }: { label: string; description: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 px-4 py-3.5">
      <div>
        <p className="text-[13px] font-medium">{label}</p>
        <p className="mt-0.5 text-[12px] text-muted">{description}</p>
      </div>
      {children}
    </div>
  );
}

function PermissionControl({ granted, onFix }: { granted: boolean; onFix: () => void }) {
  return granted ? (
    <span className="flex items-center gap-1.5 text-[12px] font-medium text-ok">
      <Check className="size-3.5" /> Granted
    </span>
  ) : (
    <Button variant="secondary" className="h-8 text-[12px]" onClick={onFix}>
      Grant access
    </Button>
  );
}

function ModelBadge({ state, progress }: { state?: ModelState; progress?: number | null }) {
  const label =
    state === "ready"
      ? "Ready"
      : state === "loading"
        ? `${Math.round((progress ?? 0) * 100)}%`
        : state === "error"
          ? "Error"
          : "Idle";
  return (
    <span className="flex items-center gap-2 text-[12px] text-muted">
      <StatusDot tone={state === "ready" ? "ok" : "warn"} />
      {label}
    </span>
  );
}
