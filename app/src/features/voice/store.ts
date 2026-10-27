import { create } from "zustand";
import {
  saveVoiceSettings,
  speakNatural,
  transcribe,
  voiceState,
  warmNaturalVoice,
  type CommandError,
  type VoiceSettings,
} from "../../lib/tauri";
import {
  SpeechEnd,
  guessLanguage,
  pickVoice,
  plausibleAfterWake,
  speechText,
  splitSentences,
  toPcm16,
} from "./model";
import { Recorder, micError } from "./recorder";

export type VoicePhase = "idle" | "listening" | "transcribing";

interface VoiceState {
  settings: VoiceSettings | null;
  phase: VoicePhase;
  /** 0–1, while listening. */
  level: number;
  error: string | null;
  /** The language Whisper heard last, shown next to the transcript. */
  heard: string | null;
  /** What is being read aloud: a turn's task id, "briefing", or null. */
  speaking: string | null;
  /** Listening after the wake word: it sends by itself when the Master pauses. */
  auto: boolean;
  loadSettings: () => Promise<void>;
  saveSettings: (
    patch: Partial<Pick<VoiceSettings, "model" | "language" | "read_aloud" | "answer_spoken">>,
  ) => Promise<void>;
  setSettings: (settings: VoiceSettings) => void;
  /** Starts recording; a second call while listening is ignored. */
  /**
   * Starts recording; a second call while listening is ignored. With `onAutoText` (after the wake
   * word) it also stops by itself when the Master pauses, and hands over the words.
   */
  startListening: (onAutoText?: (text: string) => void) => Promise<void>;
  /** Stops recording and returns the words, or null for silence, a cancel or an error. */
  finishListening: () => Promise<string | null>;
  cancelListening: () => Promise<void>;
  speak: (key: string, markdown: string) => void;
  stopSpeaking: () => void;
}

let recorder: Recorder | null = null;

// Each answer read aloud is one "run"; stopping or a newer answer moves the counter on, and the
// older run's pieces are dropped.
let speechRun = 0;
let playing: { audio: HTMLAudioElement; finish: () => void } | null = null;

function stopAudio() {
  const current = playing;
  playing = null;
  if (current) {
    current.audio.pause();
    current.finish();
  }
}

/** Plays one WAV and resolves when it ends or is stopped. */
function play(wav: ArrayBuffer): Promise<void> {
  return new Promise((resolve) => {
    const url = URL.createObjectURL(new Blob([wav], { type: "audio/wav" }));
    const audio = new Audio(url);
    const finish = () => {
      URL.revokeObjectURL(url);
      if (playing?.audio === audio) playing = null;
      resolve();
    };
    playing = { audio, finish };
    audio.onended = finish;
    audio.onerror = finish;
    void audio.play().catch(finish);
  });
}

/** The natural voice, a sentence at a time; the next is made while the current one plays. */
async function speakNaturally(text: string, run: number): Promise<void> {
  const pieces = splitSentences(text);
  let next = pieces.length > 0 ? speakNatural(pieces[0]) : null;
  for (let i = 0; i < pieces.length && next; i++) {
    const wav = await next;
    if (run !== speechRun) return;
    next = i + 1 < pieces.length ? speakNatural(pieces[i + 1]) : null;
    await play(wav);
    if (run !== speechRun) return;
  }
}

function speakWithWindows(text: string, done: () => void) {
  if (typeof speechSynthesis === "undefined") return done();
  speechSynthesis.cancel();
  const utterance = new SpeechSynthesisUtterance(text);
  const voice = pickVoice(speechSynthesis.getVoices(), guessLanguage(text));
  if (voice) {
    utterance.voice = voice;
    utterance.lang = voice.lang;
  }
  utterance.onend = done;
  utterance.onerror = done;
  speechSynthesis.speak(utterance);
}
// When the minute is up, whoever started the recording finishes it.
let onLimit: (() => void) | null = null;

// Requests that were spoken, by their text: their answers are spoken back (M9: asked by voice,
// answered by voice). The chat turn only knows its text, so that is the link.
const spokenRequests = new Set<string>();

/** True once for a request that came from the microphone. */
export function wasSpoken(text: string): boolean {
  return spokenRequests.delete(text.trim());
}

export function whenRecordingLimitReached(handler: (() => void) | null) {
  onLimit = handler;
}

function message(err: unknown): string {
  if (typeof err === "object" && err !== null && "message" in err) {
    return String((err as CommandError).message);
  }
  return String(err);
}

export const useVoice = create<VoiceState>((set, get) => ({
  settings: null,
  phase: "idle",
  level: 0,
  error: null,
  heard: null,
  speaking: null,
  auto: false,

  async loadSettings() {
    try {
      set({ settings: await voiceState(), error: null });
    } catch (err) {
      set({ error: message(err) });
    }
  },

  async saveSettings(patch) {
    const current = get().settings;
    if (!current) return;
    const next = { ...current, ...patch };
    try {
      set({
        settings: await saveVoiceSettings(
          next.model,
          next.language,
          next.read_aloud,
          next.answer_spoken,
        ),
      });
    } catch (err) {
      set({ error: message(err) });
    }
  },

  setSettings(settings) {
    set({ settings });
  },

  async startListening(onAutoText) {
    if (get().phase !== "idle") return;
    get().stopSpeaking();
    set({ phase: "listening", level: 0, error: null, auto: Boolean(onAutoText) });
    // The natural voice takes seconds to load; it does so while the user speaks.
    if (get().settings?.engine === "natural") void warmNaturalVoice().catch(() => {});
    recorder = new Recorder();
    const end = onAutoText ? new SpeechEnd() : null;
    const started = performance.now();
    let ended = false;
    try {
      await recorder.start(
        (level) => {
          set({ level });
          if (!end || ended) return;
          const state = end.feed(level, performance.now() - started);
          if (state === "done") {
            ended = true;
            void get()
              .finishListening()
              .then((text) => text && onAutoText?.(text));
          } else if (state === "nothing") {
            ended = true;
            void get().cancelListening();
          }
        },
        () => onLimit?.(),
      );
    } catch (err) {
      recorder = null;
      set({ phase: "idle", error: micError(err) });
    }
  },

  async finishListening() {
    if (get().phase !== "listening" || !recorder) return null;
    const current = recorder;
    recorder = null;
    set({ phase: "transcribing", level: 0 });
    try {
      const recording = await current.stop();
      const transcript = await transcribe(
        toPcm16(recording.samples),
        get().settings?.language ?? null,
      );
      const auto = get().auto;
      set({ phase: "idle", heard: transcript.language, auto: false });
      if (auto && !plausibleAfterWake(transcript.language)) {
        set({ error: "Didn't catch that." });
        return null;
      }
      if (!transcript.text.trim()) {
        set({ error: "Didn't catch that. Hold the key while you speak." });
        return null;
      }
      spokenRequests.add(transcript.text.trim());
      return transcript.text.trim();
    } catch (err) {
      set({ phase: "idle", error: message(err) });
      return null;
    }
  },

  async cancelListening() {
    const current = recorder;
    recorder = null;
    set({ phase: "idle", level: 0 });
    await current?.cancel();
  },

  speak(key, markdown) {
    get().stopSpeaking();
    const text = speechText(markdown);
    if (!text) return;
    const run = ++speechRun;
    const done = () => {
      if (run === speechRun) set({ speaking: null });
    };
    set({ speaking: key });
    if (get().settings?.engine === "natural") {
      void speakNaturally(text, run).then(done, (err: unknown) => {
        if (run !== speechRun) return;
        // Once, then the Windows voice reads it, so the answer is never silent (D-025).
        set({ error: `The natural voice failed (${message(err)}); using the Windows voice.` });
        speakWithWindows(text, done);
      });
    } else {
      speakWithWindows(text, done);
    }
  },

  stopSpeaking() {
    speechRun++;
    stopAudio();
    if (typeof speechSynthesis !== "undefined") speechSynthesis.cancel();
    set({ speaking: null });
  },
}));
