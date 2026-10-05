import { invoke } from "@tauri-apps/api/core";

/** Each group is satisfied by any of its keycodes; every group has to be held at once. */
export type Hotkey = { groups: number[][] };

export type Snippet = { trigger: string; text: string };

export type InputDevice = { uid: string; name: string };

/** A style for one app, matched on its name or bundle id. "off" types the plain transcript. */
export type AppStyle = "casual" | "formal" | "email" | "code" | "notes" | "off";
export type AppRule = { app: string; style: AppStyle };

/** A saved Command Mode instruction, run by saying its name or from the menu bar. */
export type Transform = { name: string; instruction: string };

export type SpeechModel = "parakeet" | "whisper";

/** A word the user keeps fixing after a paste, offered for the dictionary. */
export type Suggestion = { word: string; heard: string; count: number };

export type UpdateInfo = { available: boolean; version: string; url: string };

export type UpdateProgress =
  | { phase: "downloading"; progress: number }
  | { phase: "verifying" }
  | { phase: "restarting" };

/** How far Enhance may go. Standard is what Parla has always done. */
export type CleanupLevel = "light" | "standard" | "polished";

export type Settings = {
  hotkey: Hotkey;
  handsFree: boolean;
  repeatHotkey: Hotkey | null;
  commandHotkey: Hotkey | null;
  undoHotkey: Hotkey | null;
  transformHotkey: Hotkey | null;
  dictionary: string[];
  appAwareTone: boolean;
  livePreview: boolean;
  autoStopSilence: boolean;
  inputDevice: string | null;
  softVoice: boolean;
  snippets: Snippet[];
  sounds: boolean;
  muteOtherAudio: boolean;
  pauseMedia: boolean;
  enhance: boolean;
  quickEnhance: boolean;
  cleanupLevel: CleanupLevel;
  pauseInSensitiveApps: boolean;
  useContext: boolean;
  spokenFormatting: boolean;
  appRules: AppRule[];
  transforms: Transform[];
  learnWords: boolean;
  checkUpdates: boolean;
  speechModel: SpeechModel;
  /** Language Whisper is told to expect; null detects it. */
  language: string | null;
  onboarded: boolean;
};

export const APP_STYLES: { value: AppStyle; label: string }[] = [
  { value: "casual", label: "Casual" },
  { value: "formal", label: "Formal" },
  { value: "email", label: "Email" },
  { value: "code", label: "Code" },
  { value: "notes", label: "Notes" },
  { value: "off", label: "No cleanup" },
];

export const MAX_APP_RULES = 30;
export const MAX_TRANSFORMS = 12;

export const CLEANUP_LEVELS: { value: CleanupLevel; label: string; hint: string }[] = [
  { value: "light", label: "Light", hint: "Takes out “um” and false starts. Nothing else is touched." },
  { value: "standard", label: "Standard", hint: "Fillers, punctuation and obvious slips. Your words stay yours." },
  { value: "polished", label: "Polished", hint: "Also tidies grammar and joins fragments into proper sentences." },
];

export const MAX_DICTIONARY_TERMS = 40;
export const MAX_SNIPPETS = 50;

export type ModelState = "idle" | "loading" | "ready" | "error";

export type EnhanceStatus =
  | "available"
  | "appleIntelligenceNotEnabled"
  | "deviceNotEligible"
  | "modelNotReady"
  | "unsupportedOS"
  | "unavailable";

export type ModelStatus = {
  state: ModelState;
  progress: number | null;
  message: string | null;
  streaming: ModelState;
  punctuation: ModelState;
  punctuationProgress: number | null;
  enhance: EnhanceStatus;
  whisper: ModelState;
  whisperProgress: number | null;
  whisperMessage: string | null;
};

export type Permissions = {
  microphone: "granted" | "denied" | "undetermined";
  accessibility: boolean;
};

export type Entry = {
  id: number;
  text: string;
  language?: string;
  raw?: string;
  createdAt: number;
  durationMs: number;
  words: number;
  transcribeMs?: number;
  enhanceMs?: number;
  latencyMs?: number;
  /** Name of the app this was dictated into, for the stats on Home. */
  app?: string;
  /** Style used there, such as "email". */
  style?: string;
};

export type DictationEvent =
  | { phase: "recording"; command: boolean }
  | { phase: "processing" }
  | { phase: "done"; text: string }
  | { phase: "copied"; text: string }
  | { phase: "empty" }
  | { phase: "cancelled" }
  | { phase: "undone" }
  | { phase: "blocked"; reason: string }
  | { phase: "error"; message: string };

/** macOS virtual keycodes, which is what the key watcher polls. */
export const KEY_LABELS: Record<number, string> = {
  0: "A", 1: "S", 2: "D", 3: "F", 4: "H", 5: "G", 6: "Z", 7: "X", 8: "C", 9: "V", 11: "B",
  12: "Q", 13: "W", 14: "E", 15: "R", 16: "Y", 17: "T", 18: "1", 19: "2", 20: "3", 21: "4",
  22: "6", 23: "5", 24: "=", 25: "9", 26: "7", 27: "-", 28: "8", 29: "0", 30: "]", 31: "O",
  32: "U", 33: "[", 34: "I", 35: "P", 36: "return", 37: "L", 38: "J", 39: "'", 40: "K",
  41: ";", 42: "\\", 43: ",", 44: "/", 45: "N", 46: "M", 47: ".", 48: "tab", 49: "space",
  50: "`", 51: "delete", 53: "esc", 54: "right ⌘", 55: "⌘", 56: "⇧", 57: "caps lock",
  58: "⌥", 59: "⌃", 60: "right ⇧", 61: "right ⌥", 62: "right ⌃", 63: "fn",
  65: "num .", 67: "num *", 69: "num +", 71: "clear", 75: "num /", 76: "num enter",
  78: "num -", 81: "num =", 82: "num 0", 83: "num 1", 84: "num 2", 85: "num 3", 86: "num 4",
  87: "num 5", 88: "num 6", 89: "num 7", 91: "num 8", 92: "num 9",
  96: "F5", 97: "F6", 98: "F7", 99: "F3", 100: "F8", 101: "F9", 103: "F11", 105: "F13",
  107: "F14", 109: "F10", 111: "F12", 113: "F15", 114: "help", 115: "home", 116: "page up",
  117: "fwd delete", 118: "F4", 119: "end", 120: "F2", 121: "page down", 122: "F1",
  123: "←", 124: "→", 125: "↓", 126: "↑",
};

/** Left and right halves of the same modifier, and the label for either of them. */
export const MODIFIER_PAIRS: { codes: [number, number]; label: string }[] = [
  { codes: [55, 54], label: "⌘ command" },
  { codes: [56, 60], label: "⇧ shift" },
  { codes: [58, 61], label: "⌥ option" },
  { codes: [59, 62], label: "⌃ control" },
];

export const CAPS_LOCK = 57;
export const MODIFIER_CODES = [54, 55, 56, 58, 59, 60, 61, 62, 63];
export const MAX_HOTKEY_KEYS = 3;

const sorted = (codes: number[]) => [...codes].sort((a, b) => a - b);
const same = (a: number[], b: number[]) => a.length === b.length && sorted(a).every((code, i) => code === sorted(b)[i]);

export function keyLabel(code: number) {
  return KEY_LABELS[code] ?? `key ${code}`;
}

function groupLabel(group: number[]) {
  const pair = MODIFIER_PAIRS.find(({ codes }) => same(codes, group));
  if (pair) return pair.label;
  if (group.length === 1) return keyLabel(group[0]);
  return group.map(keyLabel).join(" / ");
}

export const hotkeyLabels = (hotkey: Hotkey) => hotkey.groups.map(groupLabel);

export const hotkeyText = (hotkey: Hotkey) => hotkeyLabels(hotkey).join(" + ");

export const sameHotkey = (a: Hotkey, b: Hotkey) =>
  a.groups.length === b.groups.length && a.groups.every((group, i) => same(group, b.groups[i]));

/**
 * Turns recorded keycodes into a shortcut. Pressing one side of a modifier normally
 * means "this modifier", so both sides are accepted unless the user asked for the exact
 * key they pressed.
 */
export function hotkeyFromCodes(codes: number[], eitherSide: boolean): Hotkey {
  const groups: number[][] = [];
  for (const code of codes) {
    const pair = eitherSide ? MODIFIER_PAIRS.find(({ codes: sides }) => sides.includes(code)) : undefined;
    const group = pair ? sorted(pair.codes) : [code];
    if (!groups.some((existing) => same(existing, group))) groups.push(group);
  }
  return { groups };
}

/** Mirrors the checks the backend runs, so the recorder can explain a bad choice first. */
export function hotkeyProblem(hotkey: Hotkey): string | null {
  const codes = hotkey.groups.flat();
  if (!hotkey.groups.length) return "Hold the keys you want to use.";
  if (hotkey.groups.length > MAX_HOTKEY_KEYS) return `Use at most ${MAX_HOTKEY_KEYS} keys.`;
  if (codes.includes(CAPS_LOCK)) return "Caps Lock stays on when pressed, so it can't be held to talk.";
  if (!hotkey.groups.some((group) => group.every((code) => MODIFIER_CODES.includes(code))))
    return "Include a modifier like ⌘, ⌥, ⌃, ⇧ or fn.";
  return null;
}

export const HOTKEY_PRESETS: { hotkey: Hotkey; label: string; hint: string }[] = [
  { hotkey: { groups: [[58, 61]] }, label: "Option", hint: "Either Option key" },
  { hotkey: { groups: [[61]] }, label: "Right Option", hint: "Keeps left Option free" },
  { hotkey: { groups: [[54]] }, label: "Right Command", hint: "Rarely used on its own" },
  { hotkey: { groups: [[59, 62]] }, label: "Control", hint: "Either Control key" },
  { hotkey: { groups: [[63]] }, label: "Globe / Fn", hint: "Set Globe to “Do Nothing” first" },
  { hotkey: { groups: [[59, 62], [58, 61]] }, label: "Control + Option", hint: "Free on most Macs" },
];

/** BCP-47 codes Parakeet TDT v3 recognises without being told which one is spoken. */
export const LANGUAGE_NAMES: Record<string, string> = {
  bg: "Bulgarian", cs: "Czech", da: "Danish", de: "German", el: "Greek", en: "English",
  es: "Spanish", et: "Estonian", fi: "Finnish", fr: "French", hr: "Croatian", hu: "Hungarian",
  it: "Italian", lt: "Lithuanian", lv: "Latvian", mt: "Maltese", nl: "Dutch", pl: "Polish",
  pt: "Portuguese", ro: "Romanian", ru: "Russian", sk: "Slovak", sl: "Slovenian",
  sv: "Swedish", uk: "Ukrainian",
};

/** Languages Whisper can be locked to, most asked-for first. It detects the rest itself. */
export const WHISPER_LANGUAGES = [
  "en", "pt", "es", "fr", "de", "it", "nl", "pl", "ru", "uk", "tr", "ar", "he", "hi", "zh", "ja",
  "ko", "vi", "th", "id", "ms", "fil", "sv", "da", "no", "fi", "el", "cs", "ro", "hu",
];

const displayNames = (() => {
  try {
    return new Intl.DisplayNames(["en"], { type: "language" });
  } catch {
    return null;
  }
})();

export const languageName = (code?: string | null) =>
  code ? LANGUAGE_NAMES[code] ?? displayNames?.of(code) ?? code.toUpperCase() : null;

export const api = {
  modelStatus: () => invoke<ModelStatus>("model_status"),
  prepareModel: () => invoke<void>("prepare_model"),
  permissions: () => invoke<Permissions>("permissions"),
  requestMicrophone: () => invoke<boolean>("request_microphone"),
  openPrivacySettings: (pane: "microphone" | "accessibility") =>
    invoke<void>("open_privacy_settings", { pane }),
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  setHotkeyCapture: (active: boolean) => invoke<void>("set_hotkey_capture", { active }),
  history: () => invoke<Entry[]>("history_list"),
  deleteEntry: (id: number) => invoke<void>("history_delete", { id }),
  clearHistory: () => invoke<void>("history_clear"),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  inputDevices: () => invoke<InputDevice[]>("input_devices"),
  /** Saves to Downloads, reveals it in Finder and returns the full path. */
  saveExport: (name: string, contents: string) => invoke<string>("save_export", { name, contents }),
  grantAccessibility: async () => {
    const trusted = await invoke<boolean>("request_accessibility");
    if (!trusted) await invoke<void>("open_privacy_settings", { pane: "accessibility" });
  },
  launchAtLogin: () => invoke<boolean>("launch_at_login"),
  setLaunchAtLogin: (enabled: boolean) => invoke<void>("set_launch_at_login", { enabled }),
  suggestions: () => invoke<Suggestion[]>("suggestions_list"),
  resolveSuggestion: (word: string, accepted: boolean) =>
    invoke<void>("suggestion_resolve", { word, accepted }),
  checkForUpdate: () => invoke<UpdateInfo>("check_for_update"),
  openRelease: (url: string) => invoke<void>("open_release", { url }),
  /** Downloads, checks and installs the update, then Parla quits and reopens. */
  installUpdate: (url: string) => invoke<void>("install_update", { url }),
};
