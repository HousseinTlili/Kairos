import { beforeEach, describe, expect, it, vi } from "vitest";

const tauri = vi.hoisted(() => ({
  transcribe: vi.fn(),
  voiceState: vi.fn(),
  saveVoiceSettings: vi.fn(),
}));
vi.mock("../../lib/tauri", () => tauri);

const recorder = vi.hoisted(() => ({
  start: vi.fn(),
  stop: vi.fn(),
  cancel: vi.fn(),
}));
vi.mock("./recorder", () => ({
  Recorder: class {
    start = recorder.start;
    stop = recorder.stop;
    cancel = recorder.cancel;
  },
  micError: () => "Kairos isn't allowed to use the microphone.",
}));

import { useVoice, wasSpoken } from "./store";

const recording = { samples: new Float32Array(16_000), seconds: 1 };

describe("the voice states", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    useVoice.setState({ phase: "idle", error: null, heard: null, level: 0, settings: null });
    recorder.start.mockResolvedValue(undefined);
    recorder.stop.mockResolvedValue(recording);
  });

  it("goes listening, then transcribing, then hands over the words", async () => {
    tauri.transcribe.mockResolvedValue({ text: " Open Notepad ", language: "en", elapsed_ms: 900 });
    await useVoice.getState().startListening();
    expect(useVoice.getState().phase).toBe("listening");

    const finished = useVoice.getState().finishListening();
    expect(useVoice.getState().phase).toBe("transcribing");
    expect(await finished).toBe("Open Notepad");
    expect(useVoice.getState()).toMatchObject({ phase: "idle", heard: "en", error: null });
  });

  it("remembers a spoken request once, so only its answer is spoken back", async () => {
    tauri.transcribe.mockResolvedValue({ text: "What time is it?", language: "en", elapsed_ms: 1 });
    await useVoice.getState().startListening();
    await useVoice.getState().finishListening();
    expect(wasSpoken("Something I typed")).toBe(false);
    expect(wasSpoken("What time is it?")).toBe(true);
    expect(wasSpoken("What time is it?")).toBe(false);
  });

  it("sends the fixed language when one is set", async () => {
    useVoice.setState({
      settings: {
        models: [],
        model: "small",
        language: "fr",
        read_aloud: false,
        answer_spoken: true,
        ptt_hotkey: "Ctrl+Alt+J",
        downloading: false,
      },
    });
    tauri.transcribe.mockResolvedValue({ text: "Bonjour", language: "fr", elapsed_ms: 1 });
    await useVoice.getState().startListening();
    await useVoice.getState().finishListening();
    expect(tauri.transcribe).toHaveBeenCalledWith(expect.any(Int16Array), "fr");
  });

  it("sends nothing for silence and says so", async () => {
    tauri.transcribe.mockResolvedValue({ text: "", language: null, elapsed_ms: 0 });
    await useVoice.getState().startListening();
    expect(await useVoice.getState().finishListening()).toBeNull();
    expect(useVoice.getState().error).toMatch(/Didn't catch that/);
  });

  it("explains a refused microphone and stays idle", async () => {
    recorder.start.mockRejectedValue(new DOMException("denied", "NotAllowedError"));
    await useVoice.getState().startListening();
    expect(useVoice.getState()).toMatchObject({ phase: "idle" });
    expect(useVoice.getState().error).toMatch(/microphone/);
  });

  it("passes on a missing model so the page can point to Settings", async () => {
    tauri.transcribe.mockRejectedValue({
      kind: "voice_model_missing",
      message: "Download a voice model first: Settings ▸ Voice.",
    });
    await useVoice.getState().startListening();
    expect(await useVoice.getState().finishListening()).toBeNull();
    expect(useVoice.getState().error).toMatch(/Download a voice model/);
  });

  it("cancelling throws the audio away without transcribing", async () => {
    await useVoice.getState().startListening();
    await useVoice.getState().cancelListening();
    expect(recorder.cancel).toHaveBeenCalled();
    expect(tauri.transcribe).not.toHaveBeenCalled();
    expect(useVoice.getState().phase).toBe("idle");
    expect(await useVoice.getState().finishListening()).toBeNull();
  });

  it("ignores a second start while already listening", async () => {
    await useVoice.getState().startListening();
    await useVoice.getState().startListening();
    expect(recorder.start).toHaveBeenCalledTimes(1);
  });
});
