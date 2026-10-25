import { create } from "zustand";
import {
  saveVoiceSettings,
  transcribe,
  voiceState,
  type CommandError,
  type VoiceSettings,
} from "../../lib/tauri";
import { guessLanguage, pickVoice, speechText, toPcm16 } from "./model";
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
  loadSettings: () => Promise<void>;
  saveSettings: (
    patch: Partial<Pick<VoiceSettings, "model" | "language" | "read_aloud">>,
  ) => Promise<void>;
  setSettings: (settings: VoiceSettings) => void;
  /** Starts recording; a second call while listening is ignored. */
  startListening: () => Promise<void>;
  /** Stops recording and returns the words, or null for silence, a cancel or an error. */
  finishListening: () => Promise<string | null>;
  cancelListening: () => Promise<void>;
  speak: (key: string, markdown: string) => void;
  stopSpeaking: () => void;
}

let recorder: Recorder | null = null;
// When the minute is up, whoever started the recording finishes it.
let onLimit: (() => void) | null = null;

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
      set({ settings: await saveVoiceSettings(next.model, next.language, next.read_aloud) });
    } catch (err) {
      set({ error: message(err) });
    }
  },

  setSettings(settings) {
    set({ settings });
  },

  async startListening() {
    if (get().phase !== "idle") return;
    get().stopSpeaking();
    set({ phase: "listening", level: 0, error: null });
    recorder = new Recorder();
    try {
      await recorder.start(
        (level) => set({ level }),
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
      set({ phase: "idle", heard: transcript.language });
      if (!transcript.text.trim()) {
        set({ error: "Didn't catch that. Hold the key while you speak." });
        return null;
      }
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
    if (typeof speechSynthesis === "undefined") return;
    speechSynthesis.cancel();
    const text = speechText(markdown);
    if (!text) return;
    const utterance = new SpeechSynthesisUtterance(text);
    const language = guessLanguage(text);
    const voice = pickVoice(speechSynthesis.getVoices(), language);
    if (voice) {
      utterance.voice = voice;
      utterance.lang = voice.lang;
    }
    utterance.onend = () => {
      if (get().speaking === key) set({ speaking: null });
    };
    utterance.onerror = utterance.onend;
    set({ speaking: key });
    speechSynthesis.speak(utterance);
  },

  stopSpeaking() {
    if (typeof speechSynthesis !== "undefined") speechSynthesis.cancel();
    set({ speaking: null });
  },
}));
