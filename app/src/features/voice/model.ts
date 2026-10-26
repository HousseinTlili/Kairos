// Voice's small rules (M9 §3). Pure, for the tests.
import type { Speaker } from "../../lib/tauri";

export const TARGET_RATE = 16_000;
/** A recording stops by itself after a minute. */
export const MAX_SECONDS = 60;

/** Mono, by averaging the channels the browser hands over. */
export function downmix(channels: Float32Array[]): Float32Array {
  if (channels.length === 1) return channels[0];
  const length = Math.min(...channels.map((c) => c.length));
  const out = new Float32Array(length);
  for (const channel of channels) {
    for (let i = 0; i < length; i++) out[i] += channel[i] / channels.length;
  }
  return out;
}

/**
 * Linear resampling to 16 kHz. The recording context asks for 16 kHz, so this is the fallback for
 * a device the browser won't open at that rate; linear is plenty for speech recognition.
 */
export function resample(
  input: Float32Array,
  fromRate: number,
  toRate = TARGET_RATE,
): Float32Array {
  if (fromRate === toRate || input.length === 0) return input;
  const ratio = fromRate / toRate;
  const length = Math.floor(input.length / ratio);
  const out = new Float32Array(length);
  for (let i = 0; i < length; i++) {
    const at = i * ratio;
    const left = Math.floor(at);
    const right = Math.min(left + 1, input.length - 1);
    const t = at - left;
    out[i] = input[left] * (1 - t) + input[right] * t;
  }
  return out;
}

/** Joins the recorded buffers, cut at `MAX_SECONDS`. */
export function joinChunks(chunks: Float32Array[], rate = TARGET_RATE): Float32Array {
  const limit = MAX_SECONDS * rate;
  const total = Math.min(
    limit,
    chunks.reduce((sum, c) => sum + c.length, 0),
  );
  const out = new Float32Array(total);
  let at = 0;
  for (const chunk of chunks) {
    if (at >= total) break;
    const part = chunk.subarray(0, total - at);
    out.set(part, at);
    at += part.length;
  }
  return out;
}

/** What `transcribe` takes: signed 16-bit, clipped. */
export function toPcm16(samples: Float32Array): Int16Array {
  const out = new Int16Array(samples.length);
  for (let i = 0; i < samples.length; i++) {
    const s = Math.max(-1, Math.min(1, samples[i]));
    out[i] = s < 0 ? Math.round(s * 32768) : Math.round(s * 32767);
  }
  return out;
}

/** 0–1 for the level meter: the buffer's root-mean-square, scaled so normal speech fills it. */
export function level(samples: Float32Array): number {
  if (samples.length === 0) return 0;
  let energy = 0;
  for (const s of samples) energy += s * s;
  return Math.min(1, Math.sqrt(energy / samples.length) * 8);
}

// --- Speech out ----------------------------------------------------------------------------------

/**
 * The answer as it should be heard: Markdown marks dropped, links read as their text, code blocks
 * replaced by a short note (reading code aloud helps nobody).
 */
export function speechText(markdown: string): string {
  return markdown
    .replace(/```[\s\S]*?(```|$)/g, " (code block skipped) ")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/https?:\/\/\S+/g, "a link")
    .replace(/^\s{0,3}#{1,6}\s+/gm, "")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/^\s*>\s?/gm, "")
    .replace(/(\*\*|__|\*|_|~~)(\S[\s\S]*?\S|\S)\1/g, "$2")
    .replace(/\|/g, " ")
    .replace(/[ \t]+/g, " ")
    .replace(/\n{2,}/g, "\n")
    .trim();
}

/** A rough language guess for text with no detected language (an answer), for picking a voice. */
export function guessLanguage(text: string): "ar" | "fr" | "en" {
  const letters = text.replace(/[^\p{L}]/gu, "");
  if (letters.length === 0) return "en";
  const arabic = (letters.match(/[؀-ۿ]/g) ?? []).length;
  if (arabic / letters.length > 0.3) return "ar";
  const lower = ` ${text.toLowerCase()} `;
  const french = [
    " le ",
    " la ",
    " les ",
    " des ",
    " est ",
    " et ",
    " je ",
    " vous ",
    " une ",
    " pour ",
  ].filter((w) => lower.includes(w)).length;
  const accents = (text.match(/[éèêàçùâîôû]/gi) ?? []).length;
  return french >= 2 || accents >= 3 ? "fr" : "en";
}

export interface VoiceLike {
  name: string;
  lang: string;
  default?: boolean;
}

/** A voice for the language ("fr" matches "fr-FR"), else the default, else the first; none = null. */
export function pickVoice<V extends VoiceLike>(voices: V[], language: string): V | null {
  const prefix = language.toLowerCase();
  return (
    voices.find((v) => v.lang.toLowerCase().startsWith(prefix)) ??
    voices.find((v) => v.default) ??
    voices[0] ??
    null
  );
}

export const LANGUAGE_NAMES: Record<string, string> = {
  en: "English",
  fr: "French",
  ar: "Arabic",
};

/** Languages the user may want read aloud that have no Windows voice yet. */
export function missingVoices(voices: VoiceLike[]): string[] {
  return Object.keys(LANGUAGE_NAMES).filter(
    (code) => !voices.some((v) => v.lang.toLowerCase().startsWith(code)),
  );
}

export function megabytes(bytes: number): string {
  return `${Math.round(bytes / 1_000_000)} MB`;
}

/**
 * An answer cut into pieces the natural voice reads one by one, so the first is heard while the
 * rest are still being made: sentences, very short ones joined to the next, long ones cut at a
 * comma or a space.
 */
export function splitSentences(text: string, min = 24, max = 240): string[] {
  const sentences = text
    .replace(/\s+/g, " ")
    .split(/(?<=[.!?…:;])\s+(?=\S)/u)
    .map((s) => s.trim())
    .filter(Boolean);
  const joined: string[] = [];
  for (const sentence of sentences) {
    const last = joined[joined.length - 1];
    if (last !== undefined && last.length < min) joined[joined.length - 1] = `${last} ${sentence}`;
    else joined.push(sentence);
  }
  return joined.flatMap((piece) => cut(piece, max));
}

function cut(piece: string, max: number): string[] {
  if (piece.length <= max) return [piece];
  const window = piece.slice(0, max);
  const at = Math.max(window.lastIndexOf(", "), window.lastIndexOf(" "));
  const split = at > max / 3 ? at + 1 : max;
  return [piece.slice(0, split).trim(), ...cut(piece.slice(split).trim(), max)];
}

export const SPEAKERS: { id: Speaker; label: string }[] = [
  { id: "george", label: "George (British)" },
  { id: "lewis", label: "Lewis (British)" },
  { id: "emma", label: "Emma (British)" },
  { id: "isabella", label: "Isabella (British)" },
  { id: "adam", label: "Adam (American)" },
  { id: "michael", label: "Michael (American)" },
  { id: "bella", label: "Bella (American)" },
  { id: "nicole", label: "Nicole (American)" },
  { id: "sarah", label: "Sarah (American)" },
  { id: "sky", label: "Sky (American)" },
  { id: "heart", label: "Heart (American)" },
];
