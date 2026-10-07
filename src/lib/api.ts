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

/** What a mode starts from. Default is Parla as it always worked; Custom runs your own instructions. */
export type ModePreset = "default" | "voice" | "message" | "email" | "note" | "custom";

export type ModeExample = { input: string; output: string };

/** Context a mode lets its cleanup see, all read on-device. */
export type ModeContext = { selection: boolean; clipboard: boolean; app: boolean };

export type Mode = {
  id: string;
  name: string;
  preset: ModePreset;
  instructions: string;
  examples: ModeExample[];
  context: ModeContext;
  /** App names, bundle id fragments or website domains that switch to this mode. */
  apps: string[];
  hotkey: Hotkey | null;
  /** Language code to write the result in; null keeps the spoken language. */
  translate: string | null;
  /** Overrides the Writing settings' cleanup level. */
  level: CleanupLevel | null;
};

export const MODE_PRESETS: { value: ModePreset; label: string; hint: string }[] = [
  { value: "default", label: "Default", hint: "Cleanup that matches the app you're in." },
  { value: "voice", label: "Voice to Text", hint: "Exactly what you said, no cleanup. The fastest." },
  { value: "message", label: "Message", hint: "Casual and brief, like a chat message." },
  { value: "email", label: "Email", hint: "Properly punctuated, laid out with greeting and sign-off." },
  { value: "note", label: "Note", hint: "Paragraphs and bullet lists for items and action points." },
  { value: "custom", label: "Custom", hint: "Your own instructions, run on-device." },
];

export const MAX_MODES = 16;

/** What other audio does while Parla listens. */
export type OtherAudio = "keep" | "lower" | "mute";

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
  modes: Mode[];
  activeMode: string;
  modeHotkey: Hotkey | null;
  shiftToSend: boolean;
  escCancels: boolean;
  preferredMics: string[];
  hiddenMics: string[];
  boostInput: boolean;
  otherAudio: OtherAudio;
  trimSilence: boolean;
  unloadAfterMinutes: number;
  keepAudioDays: number;
  trayClickRecords: boolean;
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
  /** Windows and Linux: the local Enhance model's download, which the Mac doesn't have. */
  enhanceProgress?: number | null;
  enhanceMessage?: string | null;
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
  /** Name of the mode that wrote it. */
  mode?: string;
  /** What the cleanup model was asked, when a mode added instructions or context. */
  prompt?: string;
  /** Context that went with it: "selection", "clipboard", "app". */
  context?: string[];
  /** The file it was transcribed from. */
  source?: string;
  /** Whether its audio is kept. */
  audio?: boolean;
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
  | { phase: "notice"; text: string }
  | { phase: "working"; label: string }
  | { phase: "error"; message: string };

/** Which keyboard the app is running on. Keycodes are the platform's own. */
export const PLATFORM: "mac" | "windows" | "linux" = (() => {
  const agent = typeof navigator === "undefined" ? "" : navigator.userAgent;
  if (/Windows/i.test(agent)) return "windows";
  if (/Mac/i.test(agent)) return "mac";
  return "linux";
})();

export const IS_MAC = PLATFORM === "mac";
/** How the interface names things that differ between platforms. */
export const THIS_DEVICE = IS_MAC ? "this Mac" : "this computer";
export const TRAY_NAME = IS_MAC ? "menu bar" : "system tray";
export const MOD_KEY = IS_MAC ? "⌘" : "Ctrl+";
export const PASTE_KEYS = IS_MAC ? "⌘V" : "Ctrl+V";

type KeyTable = {
  labels: Record<number, string>;
  pairs: { codes: [number, number]; label: string }[];
  capsLock: number;
  modifiers: number[];
  presets: { hotkey: Hotkey; label: string; hint: string }[];
  modifierHint: string;
};

const range = (from: number, labels: string[]) =>
  Object.fromEntries(labels.map((label, i) => [from + i, label]));
const LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("");

/** macOS virtual keycodes, which is what the key watcher polls on a Mac. */
const MAC_KEYS: KeyTable = {
  labels: {
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
  },
  pairs: [
    { codes: [55, 54], label: "⌘ command" },
    { codes: [56, 60], label: "⇧ shift" },
    { codes: [58, 61], label: "⌥ option" },
    { codes: [59, 62], label: "⌃ control" },
  ],
  capsLock: 57,
  modifiers: [54, 55, 56, 58, 59, 60, 61, 62, 63],
  presets: [
    { hotkey: { groups: [[58, 61]] }, label: "Option", hint: "Either Option key" },
    { hotkey: { groups: [[61]] }, label: "Right Option", hint: "Keeps left Option free" },
    { hotkey: { groups: [[54]] }, label: "Right Command", hint: "Rarely used on its own" },
    { hotkey: { groups: [[59, 62]] }, label: "Control", hint: "Either Control key" },
    { hotkey: { groups: [[63]] }, label: "Globe / Fn", hint: "Set Globe to “Do Nothing” first" },
    { hotkey: { groups: [[59, 62], [58, 61]] }, label: "Control + Option", hint: "Free on most Macs" },
  ],
  modifierHint: "Include a modifier like ⌘, ⌥, ⌃, ⇧ or fn.",
};

/** Windows virtual-key codes. */
const WINDOWS_KEYS: KeyTable = {
  labels: {
    ...range(0x41, LETTERS),
    ...range(0x30, "0123456789".split("")),
    ...range(0x70, Array.from({ length: 24 }, (_, i) => `F${i + 1}`)),
    ...range(0x60, Array.from({ length: 10 }, (_, i) => `num ${i}`)),
    0x08: "backspace", 0x09: "tab", 0x0d: "enter", 0x13: "pause", 0x14: "caps lock",
    0x1b: "esc", 0x20: "space", 0x21: "page up", 0x22: "page down", 0x23: "end", 0x24: "home",
    0x25: "←", 0x26: "↑", 0x27: "→", 0x28: "↓", 0x2c: "print screen", 0x2d: "insert",
    0x2e: "delete", 0x5b: "Win", 0x5c: "right Win", 0x5d: "menu",
    0xa0: "Shift", 0xa1: "right Shift", 0xa2: "Ctrl", 0xa3: "right Ctrl", 0xa4: "Alt",
    0xa5: "right Alt", 0xba: ";", 0xbb: "=", 0xbc: ",", 0xbd: "-", 0xbe: ".", 0xbf: "/",
    0xc0: "`", 0xdb: "[", 0xdc: "\\", 0xdd: "]", 0xde: "'",
  },
  pairs: [
    { codes: [0xa2, 0xa3], label: "Ctrl" },
    { codes: [0x5b, 0x5c], label: "Win" },
    { codes: [0xa0, 0xa1], label: "Shift" },
    { codes: [0xa4, 0xa5], label: "Alt" },
  ],
  capsLock: 0x14,
  modifiers: [0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0x5b, 0x5c],
  presets: [
    { hotkey: { groups: [[0xa2, 0xa3], [0x5b, 0x5c]] }, label: "Ctrl + Win", hint: "Free in almost every app" },
    { hotkey: { groups: [[0xa3]] }, label: "Right Ctrl", hint: "Keeps left Ctrl free" },
    { hotkey: { groups: [[0xa2, 0xa3], [0xa0, 0xa1]] }, label: "Ctrl + Shift", hint: "Either side" },
    { hotkey: { groups: [[0xa2, 0xa3], [0xa4, 0xa5]] }, label: "Ctrl + Alt", hint: "Either side" },
  ],
  modifierHint: "Include a modifier like Ctrl, Alt, Shift or Win.",
};

/** X11 keycodes on a standard layout. */
const LINUX_KEYS: KeyTable = {
  labels: {
    ...range(10, "1234567890".split("")),
    ...range(24, "QWERTYUIOP".split("")),
    ...range(38, "ASDFGHJKL".split("")),
    ...range(52, "ZXCVBNM".split("")),
    ...range(67, Array.from({ length: 10 }, (_, i) => `F${i + 1}`)),
    9: "esc", 20: "-", 21: "=", 22: "backspace", 23: "tab", 34: "[", 35: "]", 36: "enter",
    37: "Ctrl", 47: ";", 48: "'", 49: "`", 50: "Shift", 51: "\\", 59: ",", 60: ".", 61: "/",
    62: "right Shift", 64: "Alt", 65: "space", 66: "caps lock", 92: "AltGr", 95: "F11",
    96: "F12", 105: "right Ctrl", 108: "right Alt", 110: "home", 111: "↑", 112: "page up",
    113: "←", 114: "→", 115: "end", 116: "↓", 117: "page down", 118: "insert", 119: "delete",
    133: "Super", 134: "right Super", 135: "menu",
  },
  pairs: [
    { codes: [37, 105], label: "Ctrl" },
    { codes: [133, 134], label: "Super" },
    { codes: [50, 62], label: "Shift" },
    { codes: [64, 108], label: "Alt" },
  ],
  capsLock: 66,
  modifiers: [50, 62, 37, 105, 64, 108, 92, 133, 134],
  presets: [
    { hotkey: { groups: [[37, 105], [133, 134]] }, label: "Ctrl + Super", hint: "Free in most apps" },
    { hotkey: { groups: [[105]] }, label: "Right Ctrl", hint: "Keeps left Ctrl free" },
    { hotkey: { groups: [[108]] }, label: "Right Alt", hint: "Unless your layout uses AltGr" },
    { hotkey: { groups: [[37, 105], [50, 62]] }, label: "Ctrl + Shift", hint: "Either side" },
  ],
  modifierHint: "Include a modifier like Ctrl, Alt, Shift or Super.",
};

const KEYS = PLATFORM === "windows" ? WINDOWS_KEYS : PLATFORM === "linux" ? LINUX_KEYS : MAC_KEYS;

export const KEY_LABELS = KEYS.labels;
/** Left and right halves of the same modifier, and the label for either of them. */
export const MODIFIER_PAIRS = KEYS.pairs;
export const CAPS_LOCK = KEYS.capsLock;
export const MODIFIER_CODES = KEYS.modifiers;
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
    return KEYS.modifierHint;
  return null;
}

export const HOTKEY_PRESETS = KEYS.presets;

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
  /** Transcribes a file; with no path, asks for one. */
  transcribeFile: (path?: string) => invoke<void>("transcribe_path", { path: path ?? null }),
  reprocess: (id: number, mode: string) => invoke<Entry>("history_reprocess", { id, mode }),
  /** A kept recording as a playable data URL. */
  entryAudio: (id: number) => invoke<string | null>("history_audio", { id }),
  exportBackup: (includeHistory: boolean) => invoke<string>("export_backup", { includeHistory }),
  /** Asks for a backup file and restores it; false when the user cancelled. */
  importBackup: () => invoke<boolean>("import_backup"),
  openDataFolder: () => invoke<void>("open_data_folder"),
  installCli: () => invoke<string>("install_cli"),
  claudeCodeSetup: () => invoke<{ folder: string; commands: string[] }>("claude_code_setup"),
};

/** A fresh custom mode, with an id that won't clash with the others. */
export function newMode(preset: ModePreset, existing: Mode[]): Mode {
  const base = MODE_PRESETS.find((p) => p.value === preset)?.label ?? "Mode";
  let name = base;
  for (let n = 2; existing.some((m) => m.name.toLowerCase() === name.toLowerCase()); n++) name = `${base} ${n}`;
  return {
    id: `mode-${Date.now().toString(36)}`,
    name,
    preset,
    instructions: "",
    examples: [],
    context: { selection: false, clipboard: false, app: false },
    apps: [],
    hotkey: null,
    translate: null,
    level: null,
  };
}
