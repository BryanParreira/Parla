import { getVersion } from "@tauri-apps/api/app";
import { Check, ChevronDown, ChevronUp, EyeOff, Star } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { cx } from "../lib/utils";
import { api, CLEANUP_LEVELS, IS_MAC, THIS_DEVICE, TRAY_NAME, LANGUAGE_NAMES, languageName, MAX_APP_RULES, MAX_DICTIONARY_TERMS, MAX_SNIPPETS, MAX_TRANSFORMS, WHISPER_LANGUAGES, type EnhanceStatus, type InputDevice, type Entry, type Hotkey, type ModelState, type ModelStatus, type Permissions, type OtherAudio, type Settings, type SpeechModel, type UpdateInfo } from "../lib/api";
import { useSuggestions, useUpdateInstall } from "../lib/hooks";
import { AppRuleList, Button, CopyButton, HotkeyField, HotkeyPicker, PageHeader, Row, Section, Segmented, SnippetList, StatusDot, SuggestionList, TermList, Toggle, TransformList } from "./ui";

const SPEECH_MODELS: { value: SpeechModel; label: string }[] = [
  { value: "parakeet", label: "Parakeet" },
  { value: "whisper", label: "Whisper" },
];

const LANGUAGE_COUNT = Object.keys(LANGUAGE_NAMES).length;

const OTHER_AUDIO: { value: OtherAudio; label: string }[] = [
  { value: "keep", label: "Keep" },
  { value: "lower", label: "Lower" },
  { value: "mute", label: "Mute" },
];

const KEEP_AUDIO = [
  { days: 0, label: "Don't keep" },
  { days: 1, label: "1 day" },
  { days: 7, label: "1 week" },
  { days: 14, label: "2 weeks" },
  { days: 30, label: "1 month" },
  { days: 182, label: "6 months" },
  { days: 365, label: "1 year" },
];

const IDLE_UNLOAD = [
  { minutes: 0, label: "Never" },
  { minutes: 5, label: "After 5 minutes" },
  { minutes: 15, label: "After 15 minutes" },
  { minutes: 30, label: "After 30 minutes" },
  { minutes: 60, label: "After an hour" },
];

const SELECT =
  "h-8 max-w-[220px] rounded-lg border border-line bg-raised/60 px-2.5 text-[12px] outline-none transition-colors focus:border-accent";
const LANGUAGE_EXAMPLES = ["en", "es", "pt", "fr", "de", "it"].map((code) => LANGUAGE_NAMES[code]).join(", ");

const ENHANCE_HINTS: Record<EnhanceStatus, string> = IS_MAC ? {
  available: "Removes filler words, fixes grammar and keeps only your corrections. Runs on-device with Apple Intelligence.",
  appleIntelligenceNotEnabled: "Turn on Apple Intelligence in System Settings to use Enhance.",
  modelNotReady: "Apple Intelligence is still downloading its model. Enhance turns on once it's ready.",
  deviceNotEligible: "This Mac doesn't support Apple Intelligence, so text is typed with punctuation only.",
  unsupportedOS: "Enhance needs macOS 26 or later. Text is still typed with punctuation.",
  unavailable: "Apple Intelligence isn't available right now. Text is still typed with punctuation.",
} : {
  available: "Removes filler words, fixes grammar and keeps only your corrections. Runs on this computer with a small local model.",
  appleIntelligenceNotEnabled: "",
  modelNotReady: "Downloading the Enhance model (about 1.1 GB, once). Until then text is typed with punctuation only.",
  deviceNotEligible: "",
  unsupportedOS: "",
  unavailable: "The Enhance model couldn't load. Text is still typed with punctuation.",
};

const NEEDS_ENHANCE = IS_MAC
  ? "Needs Apple Intelligence, which isn't available on this Mac."
  : "Needs the Enhance model, which is still getting ready.";

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
              : NEEDS_ENHANCE
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
        {IS_MAC && <Row
          label="Transform menu"
          description="Select some text and press this to get your transforms — Fix grammar, Shorter, Formal — right where you're typing. Pick one with its number, the arrow keys or a click; Esc closes it."
        >
          <HotkeyField
            value={settings.transformHotkey}
            emptyLabel="Not set"
            onChange={(transformHotkey) => save({ transformHotkey })}
          />
        </Row>}
        <Row
          label="Hold Shift to send"
          description="Keep Shift down as you let go of the key and Parla presses Return after typing, to send a chat message straight away."
        >
          <Toggle checked={settings.shiftToSend} onChange={(shiftToSend) => save({ shiftToSend })} />
        </Row>
        <Row
          label="Esc throws a recording away"
          description="Press Esc while Parla is listening to discard what you said. Recordings longer than 30 seconds ask for a second press."
        >
          <Toggle checked={settings.escCancels} onChange={(escCancels) => save({ escCancels })} />
        </Row>
        {shortcutError && <p className="px-4 pb-4 text-[12px] text-danger">{shortcutError}</p>}
      </Section>

      <Section title="General">
        <Row label="Microphone" description={`Which input Parla listens to. System default follows whatever ${IS_MAC ? "macOS" : "your system"} is using.`}>
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
        {!settings.inputDevice && devices.length > 1 && (
          <MicRanking
            devices={devices}
            preferred={settings.preferredMics}
            hidden={settings.hiddenMics}
            onChange={(patch) => save(patch)}
          />
        )}
        {IS_MAC && (
          <Row
            label="Turn up the microphone"
            description="Sets the input volume to full while you dictate, so a quiet microphone isn't heard as silence, and puts your level back afterwards."
          >
            <Toggle checked={settings.boostInput} onChange={(boostInput) => update({ boostInput })} />
          </Row>
        )}
        {IS_MAC && (
          <Row
            label="Whisper mode"
            description="Boosts the microphone so you can dictate softly in a shared space or a quiet room. Leave off when speaking normally."
          >
            <Toggle checked={settings.softVoice} onChange={(softVoice) => update({ softVoice })} />
          </Row>
        )}
        <Row
          label={`Click the ${TRAY_NAME} icon to record`}
          description={`A click starts and stops a dictation instead of opening the menu. ${IS_MAC ? "Right-click" : "Right-click"} still opens it. The icon shows what Parla is doing: red while it listens, blue while it writes, green once your text is in.`}
        >
          <Toggle checked={settings.trayClickRecords} onChange={(trayClickRecords) => update({ trayClickRecords })} />
        </Row>
        <Row label="Open at login" description={`Start Parla quietly in the ${TRAY_NAME} when you log in.`}>
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
          label="Other audio while you talk"
          description="Leave music and video playing, turn them down, or silence them. The volume goes back exactly as it was."
        >
          <Segmented
            options={OTHER_AUDIO}
            value={settings.muteOtherAudio ? settings.otherAudio === "keep" ? "keep" : settings.otherAudio : "keep"}
            onChange={(choice) =>
              update(choice === "keep" ? { muteOtherAudio: false } : { muteOtherAudio: true, otherAudio: choice })
            }
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
        {IS_MAC && (
          <Row label="Accessibility" description="Lets Parla paste text into the app you're using.">
            <PermissionControl granted={!!permissions?.accessibility} onFix={api.grantAccessibility} />
          </Row>
        )}
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
              : `Needs Enhance, which isn't available on ${THIS_DEVICE} yet.`
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
              : `Needs Enhance, which isn't available on ${THIS_DEVICE} yet.`
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
            ? `Saved rewrites for selected text. Say the name while holding the Command Mode key, or pick one from Parla's ${TRAY_NAME} icon.`
            : NEEDS_ENHANCE
        }
      >
        <TransformList
          transforms={settings.transforms}
          limit={MAX_TRANSFORMS}
          disabled={!enhanceAvailable}
          onChange={(transforms) => save({ transforms })}
        />
      </Section>

      <Section
        title="Speech models"
        description={IS_MAC ? "Everything runs on this Mac's Neural Engine. Audio never leaves it." : "Everything runs on this computer. Audio never leaves it."}
      >
        <Row
          label="Transcription"
          description={
            settings.speechModel === "whisper"
              ? "Whisper understands 99 languages. A little slower than Parakeet, and a one-time 1.6 GB download."
              : `Parakeet is the fastest, for ${LANGUAGE_COUNT} European languages. Switch to Whisper for Chinese, Japanese, Arabic, Hindi and more.`
          }
        >
          {IS_MAC && <Segmented options={SPEECH_MODELS} value={settings.speechModel} onChange={(speechModel) => update({ speechModel })} />}
        </Row>
        {IS_MAC && settings.speechModel === "whisper" && (
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
        {IS_MAC && (
          <Row
            label="Cut out long silences"
            description="Removes pauses of more than a second before transcribing, which stops Whisper inventing words for them. Short pauses stay as you spoke them."
          >
            <Toggle checked={settings.trimSilence} onChange={(trimSilence) => update({ trimSilence })} />
          </Row>
        )}
        {IS_MAC && (
          <Row
            label="Free memory when idle"
            description="Unloads the big speech models when you haven't dictated for a while. The next dictation still starts at once and the models load again while you talk."
          >
            <select
              value={settings.unloadAfterMinutes}
              onChange={(event) => update({ unloadAfterMinutes: Number(event.target.value) })}
              className={SELECT}
            >
              {IDLE_UNLOAD.map((option) => (
                <option key={option.minutes} value={option.minutes}>
                  {option.label}
                </option>
              ))}
            </select>
          </Row>
        )}
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

      <Section title="History and backup">
        <Row
          label="Keep recordings for"
          description={
            settings.keepAudioDays > 0
              ? "Each dictation's audio is kept on this device so you can play it back and transcribe it again with another mode. Older ones are deleted on their own."
              : "Off: audio is thrown away as soon as it's transcribed. Turning it on keeps it so you can replay a dictation or run it through another mode."
          }
        >
          <select
            value={settings.keepAudioDays}
            onChange={(event) => update({ keepAudioDays: Number(event.target.value) })}
            className={SELECT}
          >
            {KEEP_AUDIO.map((option) => (
              <option key={option.days} value={option.days}>
                {option.label}
              </option>
            ))}
          </select>
        </Row>
        <BackupRows />
      </Section>

      <Integrations />

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

/** Microphones in the order Parla tries them, and the ones it never offers. */
function MicRanking({
  devices,
  preferred,
  hidden,
  onChange,
}: {
  devices: InputDevice[];
  preferred: string[];
  hidden: string[];
  onChange: (patch: Partial<Settings>) => void;
}) {
  const ranked = [
    ...preferred.map((uid) => devices.find((d) => d.uid === uid)).filter((d): d is InputDevice => !!d),
    ...devices.filter((d) => !preferred.includes(d.uid)),
  ];
  const move = (uid: string, by: number) => {
    const order = preferred.filter((kept) => devices.some((d) => d.uid === kept));
    const index = order.indexOf(uid);
    const next = [...order];
    next.splice(index, 1);
    next.splice(Math.max(0, Math.min(next.length, index + by)), 0, uid);
    onChange({ preferredMics: next });
  };
  const star = (uid: string) =>
    onChange({
      preferredMics: preferred.includes(uid) ? preferred.filter((kept) => kept !== uid) : [...preferred, uid],
      hiddenMics: hidden.filter((kept) => kept !== uid),
    });
  const hide = (uid: string) =>
    onChange({
      hiddenMics: hidden.includes(uid) ? hidden.filter((kept) => kept !== uid) : [...hidden, uid],
      preferredMics: preferred.filter((kept) => kept !== uid),
    });

  return (
    <div className="px-4 py-3.5">
      <p className="text-[13px] font-medium">Preferred microphones</p>
      <p className="mt-0.5 text-[12px] text-muted">
        Star the ones you trust. Parla uses the first starred microphone that's plugged in, so headphones joining
        can't take over. Hidden ones never show in the menu.
      </p>
      <div className="mt-2.5 flex flex-col gap-1.5">
        {ranked.map((device) => {
          const starred = preferred.includes(device.uid);
          const position = preferred.indexOf(device.uid);
          const off = hidden.includes(device.uid);
          return (
            <div key={device.uid} className={cx("flex items-center gap-2 rounded-lg border border-line bg-raised/60 py-1.5 pl-3 pr-1.5 text-[12.5px]", off && "opacity-50")}>
              <span className="w-4 text-[11px] tabular-nums text-muted">{starred ? position + 1 : ""}</span>
              <span className="min-w-0 flex-1 truncate">{device.name}</span>
              {starred && (
                <>
                  <IconButton title="Try sooner" disabled={position === 0} onClick={() => move(device.uid, -1)}>
                    <ChevronUp className="size-3.5" />
                  </IconButton>
                  <IconButton title="Try later" disabled={position === preferred.length - 1} onClick={() => move(device.uid, 1)}>
                    <ChevronDown className="size-3.5" />
                  </IconButton>
                </>
              )}
              <IconButton title={starred ? "Don't prefer" : "Prefer this one"} onClick={() => star(device.uid)}>
                <Star className={cx("size-3.5", starred && "fill-current text-fg")} />
              </IconButton>
              <IconButton title={off ? "Show again" : "Hide"} onClick={() => hide(device.uid)}>
                <EyeOff className={cx("size-3.5", off && "text-fg")} />
              </IconButton>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function IconButton({ title, onClick, disabled, children }: { title: string; onClick: () => void; disabled?: boolean; children: ReactNode }) {
  return (
    <button
      title={title}
      disabled={disabled}
      onClick={onClick}
      className="grid size-6 shrink-0 place-items-center rounded text-muted transition-colors hover:text-fg disabled:opacity-30"
    >
      {children}
    </button>
  );
}

function BackupRows() {
  const [note, setNote] = useState<string | null>(null);
  const run = async (work: () => Promise<string | null>) => {
    setNote(null);
    try {
      setNote(await work());
    } catch (error) {
      setNote(String(error));
    }
  };
  return (
    <>
      <Row
        label="Back up"
        description={note ?? "Saves your settings, modes, words and snippets to one file in Downloads, to restore later or on another Mac."}
      >
        <div className="flex shrink-0 gap-2">
          <Button variant="secondary" className="h-8 text-[12px]" onClick={() => run(async () => `Saved to ${await api.exportBackup(false)}`)}>
            Export
          </Button>
          <Button variant="secondary" className="h-8 text-[12px]" title="Includes every dictation" onClick={() => run(async () => `Saved to ${await api.exportBackup(true)}`)}>
            With history
          </Button>
          {IS_MAC && (
            <Button variant="secondary" className="h-8 text-[12px]" onClick={() => run(async () => ((await api.importBackup()) ? "Restored." : null))}>
              Restore…
            </Button>
          )}
        </div>
      </Row>
      <Row label="Data folder" description="Where Parla keeps its settings, history and kept recordings.">
        <Button variant="secondary" className="h-8 text-[12px]" onClick={() => api.openDataFolder().catch(() => {})}>
          Show
        </Button>
      </Row>
    </>
  );
}

/** The command-line tool, and Claude Code. */
function Integrations() {
  const [cli, setCli] = useState<string | null>(null);
  const [setup, setSetup] = useState<{ folder: string; commands: string[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  if (!IS_MAC) return null;
  return (
    <Section title="Integrations" description="For scripts and coding agents. All of it runs on this Mac.">
      <Row
        label="Command-line tool"
        description={
          cli ?? "Adds `parla` to your terminal: search and export your history, see stats, manage words and snippets, start a recording or transcribe a file."
        }
      >
        <Button
          variant="secondary"
          className="h-8 text-[12px]"
          onClick={() =>
            api
              .installCli()
              .then((path) => setCli(`Installed at ${path}. Try \`parla stats\` or \`parla help\`.`))
              .catch((reason) => setCli(String(reason)))
          }
        >
          Install
        </Button>
      </Row>
      <Row
        label="Claude Code"
        description="Parla's pill tells you when Claude Code needs an answer or has finished, so you can reply by voice, and Claude can search your dictations and add words over MCP."
      >
        <Button
          variant="secondary"
          className="h-8 text-[12px]"
          onClick={() =>
            api
              .claudeCodeSetup()
              .then(setSetup)
              .catch((reason) => setError(String(reason)))
          }
        >
          Set up
        </Button>
      </Row>
      {error && <p className="px-4 pb-3 text-[12px] text-danger">{error}</p>}
      {setup && (
        <div className="px-4 py-3.5">
          <p className="text-[12px] text-muted">The plugin is ready. Run these two commands in a terminal, then restart Claude Code:</p>
          {setup.commands.map((command) => (
            <div key={command} className="mt-2 flex items-center gap-2 rounded-lg border border-line bg-bg/60 py-1 pl-3 pr-1">
              <code className="selectable min-w-0 flex-1 truncate text-[12px]">{command}</code>
              <CopyButton text={command} />
            </div>
          ))}
        </div>
      )}
      <Row
        label="MCP for other assistants"
        description="Any MCP client can run Parla's server with the command below. It reads your history, words and snippets; what it reads goes to that assistant."
      >
        <span className="flex shrink-0 items-center gap-1">
          <code className="selectable text-[12px] text-muted">parla mcp</code>
          <CopyButton text="claude mcp add parla -- parla mcp" />
        </span>
      </Row>
    </Section>
  );
}
