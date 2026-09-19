import type { Entry } from "./api";

const TYPING_WPM = 40;
const DAY_MS = 86_400_000;

export const cx = (...classes: (string | false | null | undefined)[]) =>
  classes.filter(Boolean).join(" ");

export function computeStats(entries: Entry[]) {
  const words = entries.reduce((sum, e) => sum + e.words, 0);
  const speakingMs = entries.reduce((sum, e) => sum + e.durationMs, 0);
  const weekAgo = Date.now() - 7 * DAY_MS;
  const weekWords = entries.filter((e) => e.createdAt >= weekAgo).reduce((sum, e) => sum + e.words, 0);
  const wpm = speakingMs > 0 ? Math.round(words / (speakingMs / 60_000)) : 0;
  const savedMs = Math.max(0, (words / TYPING_WPM) * 60_000 - speakingMs);
  return { words, weekWords, wpm, savedMs };
}

export function entryWpm(entry: Entry) {
  return entry.durationMs > 0 ? Math.round(entry.words / (entry.durationMs / 60_000)) : 0;
}

export const formatLatency = (ms: number) => (ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`);

export function formatSaved(ms: number) {
  const minutes = Math.round(ms / 60_000);
  if (minutes < 1) return "0m";
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

export function formatNumber(value: number) {
  return new Intl.NumberFormat().format(value);
}

export function formatTime(timestamp: number) {
  return new Date(timestamp).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

export function dayLabel(timestamp: number) {
  const date = new Date(timestamp);
  const today = new Date();
  const startOfToday = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  if (timestamp >= startOfToday) return "Today";
  if (timestamp >= startOfToday - DAY_MS) return "Yesterday";
  return date.toLocaleDateString([], { weekday: "long", month: "short", day: "numeric" });
}

export function greeting() {
  const hour = new Date().getHours();
  if (hour < 5) return "Working late";
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}
