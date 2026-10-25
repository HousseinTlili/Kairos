// Records the microphone while push-to-talk is held (M9 §3, D-022). Audio stays in this page's
// memory until it is handed to `transcribe`; nothing is written to disk.
import { MAX_SECONDS, TARGET_RATE, downmix, joinChunks, level, resample } from "./model";

// Copies each block of samples out of the audio thread. Inline, so there's no extra file to serve.
const WORKLET = `
class KairosTap extends AudioWorkletProcessor {
  process(inputs) {
    const input = inputs[0];
    if (input && input.length > 0 && input[0].length > 0) {
      this.port.postMessage(input.map((channel) => channel.slice(0)));
    }
    return true;
  }
}
registerProcessor("kairos-tap", KairosTap);
`;

export interface Recording {
  /** 16 kHz mono floats. */
  samples: Float32Array;
  seconds: number;
}

export class Recorder {
  private stream: MediaStream | null = null;
  private context: AudioContext | null = null;
  private chunks: Float32Array[] = [];
  private limit: ReturnType<typeof setTimeout> | null = null;

  /**
   * `onLevel` gets 0–1 about 30 times a second for the meter; `onLimit` fires when the minute is
   * up and the caller should stop.
   */
  async start(onLevel: (value: number) => void, onLimit: () => void): Promise<void> {
    this.chunks = [];
    this.stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        channelCount: 1,
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
      },
    });
    // Asking for 16 kHz lets the browser resample; some devices refuse, so `resample` backs it up.
    try {
      this.context = new AudioContext({ sampleRate: TARGET_RATE });
    } catch {
      this.context = new AudioContext();
    }
    const url = URL.createObjectURL(new Blob([WORKLET], { type: "text/javascript" }));
    try {
      await this.context.audioWorklet.addModule(url);
    } finally {
      URL.revokeObjectURL(url);
    }
    const source = this.context.createMediaStreamSource(this.stream);
    const tap = new AudioWorkletNode(this.context, "kairos-tap");
    tap.port.onmessage = (event: MessageEvent<Float32Array[]>) => {
      const mono = downmix(event.data);
      this.chunks.push(mono);
      onLevel(level(mono));
    };
    source.connect(tap);
    this.limit = setTimeout(onLimit, MAX_SECONDS * 1000);
  }

  /** Stops and returns what was said, at 16 kHz. */
  async stop(): Promise<Recording> {
    const rate = this.context?.sampleRate ?? TARGET_RATE;
    const raw = joinChunks(this.chunks, rate);
    await this.release();
    const samples = resample(raw, rate);
    return { samples, seconds: samples.length / TARGET_RATE };
  }

  /** Stops and throws the audio away (Esc). */
  async cancel(): Promise<void> {
    await this.release();
  }

  private async release() {
    if (this.limit) clearTimeout(this.limit);
    this.limit = null;
    this.stream?.getTracks().forEach((track) => track.stop());
    this.stream = null;
    const context = this.context;
    this.context = null;
    this.chunks = [];
    if (context && context.state !== "closed") await context.close();
  }
}

/** The browser's error, in words the user can act on. */
export function micError(err: unknown): string {
  const name = err instanceof DOMException ? err.name : "";
  if (name === "NotAllowedError" || name === "SecurityError") {
    return "Kairos isn't allowed to use the microphone. Windows Settings ▸ Privacy & security ▸ Microphone ▸ let desktop apps use it.";
  }
  if (name === "NotFoundError" || name === "OverconstrainedError") {
    return "No microphone found.";
  }
  if (name === "NotReadableError") {
    return "The microphone is busy in another app.";
  }
  return `The microphone didn't start: ${err instanceof Error ? err.message : String(err)}`;
}
