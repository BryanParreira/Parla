import { getVersion } from "@tauri-apps/api/app";
import { Check } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { api, CLEANUP_LEVELS, LANGUAGE_NAMES, languageName, MAX_APP_RULES, MAX_DICTIONARY_TERMS, MAX_SNIPPETS, MAX_TRANSFORMS, WHISPER_LANGUAGES, type EnhanceStatus, type InputDevice, type Entry, type Hotkey, type ModelState, type ModelStatus, type Permissions, type Settings, type SpeechModel, type UpdateInfo } from "../lib/api";
import { useSuggestions, useUpdateInstall } from "../lib/hooks";
import { AppRuleList, Button, Card, HotkeyField, HotkeyPicker, PageHeader, Segmented, SnippetList, StatusDot, SuggestionList, TermList, Toggle, TransformList } from "./ui";

const SPEECH_MODELS: { value: SpeechModel; label: string }[] = [
  { value: "parakeet", label: "Parakeet" },
  { value: "whisper", label: "Whisper" },
];

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
  const [release, setRelease] = useState<UpdateInfo | "checking" | "failed" | null>(null);
  const suggestions = useSuggestions();
  const installer = useUpdateInstall();
  // Apps the user has actually dictated into, offered when adding a rule.
  const recentApps = [...new Set(entries.map((entry) => entry.app).filter((app): app is string => !!app))].slice(0, 20);

  const checkNow = () => {
    setRelease("checking");
    api.checkForUpdate().then(setRelease).catch(() => setRelease("failed"));
  };

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
  const acceptSuggestion = async (word: string) => {
    if (!settings.dictionary.some((kept) => kept.toLowerCase() === word.toLowerCase())) {
      await save({ dictionary: [...settings.dictionary, word] });
    }
    api.resolveSuggestion(word, true).catch(() => {});
  };

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
        <Row
          label="Undo last dictation"
          description="Deletes the text Parla just typed, for when it went into the wrong window. Works once, right after — not after you've carried on typing."
        >
          <HotkeyField
            value={settings.undoHotkey}
            emptyLabel="Not set"
            onChange={(undoHotkey) => save({ undoHotkey })}
          />
        </Row>
        <Row
          label="Transform menu"
          description="Select some text and press this to get your transforms — Fix grammar, Shorter, Formal — right where you're typing. Pick one with its number, the arrow keys or a click; Esc closes it."
        >
          <HotkeyField
            value={settings.transformHotkey}
            emptyLabel="Not set"
            onChange={(transformHotkey) => save({ transformHotkey })}
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
        <Row
          label="Whisper mode"
          description="Boosts the microphone so you can dictate softly in a shared space or a quiet room. Leave off when speaking normally."
        >
          <Toggle checked={settings.softVoice} onChange={(softVoice) => update({ softVoice })} />
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
        <Row
          label="Pause music and video"
          description="Pauses Spotify, Music or a YouTube tab while you talk and plays it again when you're done. Only touches something that's actually playing."
        >
          <Toggle checked={settings.pauseMedia} onChange={(pauseMedia) => update({ pauseMedia })} />
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
        <Row
          label="Never in password fields"
          description="Refuses to record while a password field has focus, or while a password manager or banking app is in front, so a stray hold can't type into one."
        >
          <Toggle
            checked={settings.pauseInSensitiveApps}
            onChange={(pauseInSensitiveApps) => update({ pauseInSensitiveApps })}
          />
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
          label="How much to change"
          description={
            enhanceAvailable
              ? CLEANUP_LEVELS.find((level) => level.value === settings.cleanupLevel)?.hint ?? ""
              : "Needs Enhance, which isn't available on this Mac."
          }
        >
          <Segmented
            options={CLEANUP_LEVELS}
            value={settings.cleanupLevel}
            disabled={!settings.enhance || !enhanceAvailable}
            onChange={(cleanupLevel) => update({ cleanupLevel })}
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
          description="Casual in chat, polished in email, hands-off in code editors. In a browser it tells Gmail from WhatsApp Web by the website's domain. Only the app's name and that domain are used — never what's on screen, and nothing leaves your Mac."
        >
          <Toggle
            checked={settings.appAwareTone && settings.enhance && enhanceAvailable}
            disabled={!settings.enhance || !enhanceAvailable}
            onChange={(appAwareTone) => update({ appAwareTone })}
          />
        </Row>
        {settings.appAwareTone && settings.enhance && enhanceAvailable && (
          <AppRuleList
            rules={settings.appRules}
            limit={MAX_APP_RULES}
            recentApps={recentApps}
            onChange={(appRules) => save({ appRules })}
          />
        )}
        <Row
          label="Fit into what's already written"
          description="Reads the few sentences before your cursor and the words right after it, so a dictation carries on your sentence with no stray capital or full stop and the right spaces. Never password fields; nothing is stored."
        >
          <Toggle checked={settings.useContext} onChange={(useContext) => update({ useContext })} />
        </Row>
        <Row
          label="Spoken formatting"
          description="Say “new line”, “bullet point”, “comma” or “question mark” and get the real thing. “Number one … number two …” makes a numbered list, and “camel case”, “snake case”, “kebab case”, “pascal case” or “constant case” before a name types it as code, up to the next pause. Punctuation works in Portuguese too."
        >
          <Toggle checked={settings.spokenFormatting} onChange={(spokenFormatting) => update({ spokenFormatting })} />
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
        <Row
          label="Learn from my fixes"
          description="When you correct a name or a spelling right after a dictation, Parla offers to add it here. Only the word is kept."
        >
          <Toggle checked={settings.learnWords} onChange={(learnWords) => update({ learnWords })} />
        </Row>
        {enhanceAvailable && (
          <SuggestionList
            suggestions={suggestions}
            disabled={settings.dictionary.length >= MAX_DICTIONARY_TERMS}
            onAccept={acceptSuggestion}
            onDismiss={(word) => api.resolveSuggestion(word, false).catch(() => {})}
          />
        )}
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

      <Section
        title="Transforms"
        description={
          enhanceAvailable
            ? "Saved rewrites for selected text. Say the name while holding the Command Mode key, or pick one from Parla's menu bar icon."
            : "Needs Apple Intelligence, which isn't available on this Mac."
        }
      >
        <TransformList
          transforms={settings.transforms}
          limit={MAX_TRANSFORMS}
          disabled={!enhanceAvailable}
          onChange={(transforms) => save({ transforms })}
        />
      </Section>

      <Section title="Speech models" description="Everything runs on this Mac's Neural Engine. Audio never leaves it.">
        <Row
          label="Transcription"
          description={
            settings.speechModel === "whisper"
              ? "Whisper understands 99 languages. A little slower than Parakeet, and a one-time 1.6 GB download."
              : `Parakeet is the fastest, for ${LANGUAGE_COUNT} European languages. Switch to Whisper for Chinese, Japanese, Arabic, Hindi and more.`
          }
        >
          <Segmented options={SPEECH_MODELS} value={settings.speechModel} onChange={(speechModel) => update({ speechModel })} />
        </Row>
        {settings.speechModel === "whisper" && (
          <>
            <Row
              label="Whisper large-v3 turbo"
              description={
                model?.whisper === "ready"
                  ? "Ready. Parakeet steps in if Whisper ever can't load."
                  : model?.whisper === "error"
                    ? model.whisperMessage ?? "Whisper couldn't load. Dictation keeps using Parakeet."
                    : model?.whisperMessage ?? "Downloading once, then it works offline. Parakeet keeps working meanwhile."
              }
            >
              {model?.whisper === "error" ? (
                <Button variant="secondary" className="h-8 text-[12px]" onClick={() => update({ speechModel: "parakeet" }).then(() => update({ speechModel: "whisper" }))}>
                  Retry
                </Button>
              ) : (
                <ModelBadge state={model?.whisper} progress={model?.whisperProgress} />
              )}
            </Row>
            <Row label="Language" description="Leave on Automatic unless Whisper keeps guessing wrong, like on very short clips.">
              <select
                value={settings.language ?? ""}
                onChange={(event) => update({ language: event.target.value || null })}
                className="h-8 max-w-[220px] rounded-lg border border-line bg-raised/60 px-2.5 text-[12px] outline-none transition-colors focus:border-accent"
              >
                <option value="">Automatic</option>
                {WHISPER_LANGUAGES.map((code) => (
                  <option key={code} value={code}>
                    {languageName(code)}
                  </option>
                ))}
              </select>
            </Row>
          </>
        )}
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
          description={
            settings.speechModel === "whisper"
              ? "Whisper works out which of 99 languages you spoke, unless you pick one above."
              : `Speak any of ${LANGUAGE_COUNT} European languages — ${LANGUAGE_EXAMPLES} and more. Parla works out which one you used, so there is nothing to switch.`
          }
        >
          <span className="text-[12px] tabular-nums text-muted">Automatic</span>
        </Row>
      </Section>

      <Section title="About">
        <Row label="Parla" description="Fast, private, on-device dictation.">
          <span className="text-[12px] tabular-nums text-muted">v{version}</span>
        </Row>
        <Row
          label="Check for updates"
          description="Asks GitHub once a day whether a newer Parla is out. Off by default: it's the only request Parla makes on its own, and nothing about you is sent."
        >
          <Toggle checked={settings.checkUpdates} onChange={(checkUpdates) => update({ checkUpdates })} />
        </Row>
        <Row
          label="Latest version"
          description={
            installer.error
              ? installer.error
              : installer.label
                ? installer.label
                : release === "checking"
              ? "Checking…"
              : release === "failed"
                ? "Couldn't reach GitHub. Try again later."
                : release?.available
                  ? `Parla ${release.version} is available.`
                  : release
                    ? "You're on the latest version."
                    : "See whether there's a newer Parla."
          }
        >
          {release && typeof release === "object" && release.available ? (
            <div className="flex shrink-0 gap-2">
              <Button variant="secondary" className="h-8 text-[12px]" onClick={() => api.openRelease(release.url)}>
                Download
              </Button>
              <Button className="h-8 text-[12px]" disabled={installer.busy} onClick={() => installer.install(release.url)}>
                Install and restart
              </Button>
            </div>
          ) : (
            <Button variant="secondary" className="h-8 text-[12px]" disabled={release === "checking"} onClick={checkNow}>
              Check now
            </Button>
          )}
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
