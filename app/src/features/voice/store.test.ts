import { beforeEach, describe, expect, it, vi } from "vitest";

const tauri = vi.hoisted(() => ({
  transcribe: vi.fn(),
  voiceState: vi.fn(),
  saveVoiceSettings: vi.fn(),
  speakNatural: vi.fn(),
  warmNaturalVoice: vi.fn(),
  wakeTranscribe: vi.fn(),
  wakeCancel: vi.fn(),
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

const naturalSettings = {
  models: [],
  model: "small" as const,
  language: null,
  read_aloud: false,
  answer_spoken: true,
  ptt_hotkey: "Ctrl+Alt+J",
  downloading: false,
  natural_installed: true,
  natural_installing: false,
  natural_size_bytes: 0,
  engine: "natural" as const,
  speaker: "george" as const,
  wake_installed: false,
  wake_installing: false,
  wake_enabled: false,
  wake_sensitivity: "normal" as const,
  wake_size_bytes: 0,
};

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
        natural_installed: false,
        natural_installing: false,
        natural_size_bytes: 0,
        engine: "windows",
        speaker: "george",
        wake_installed: false,
        wake_installing: false,
        wake_enabled: false,
        wake_sensitivity: "normal",
        wake_size_bytes: 0,
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

  it("after a wake, the request is the one Kairos kept from the start of the sentence", async () => {
    tauri.wakeTranscribe.mockResolvedValue({ text: "open Notepad", language: "en", elapsed_ms: 1 });
    await useVoice.getState().startListening(() => {});
    expect(await useVoice.getState().finishListening()).toBe("open Notepad");
    expect(tauri.wakeTranscribe).toHaveBeenCalledWith(null);
    expect(tauri.transcribe).not.toHaveBeenCalled();
    expect(recorder.cancel).toHaveBeenCalled();
    expect(useVoice.getState().auto).toBe(false);
  });

  it("after a wake, a request in a language the Master doesn't speak is dropped", async () => {
    tauri.wakeTranscribe.mockResolvedValue({ text: "E aí", language: "pt", elapsed_ms: 1 });
    await useVoice.getState().startListening(() => {});
    expect(await useVoice.getState().finishListening()).toBeNull();
  });

  it("a wake that ends in silence lets Kairos drop what it kept", async () => {
    tauri.wakeCancel.mockResolvedValue(null);
    await useVoice.getState().startListening(() => {});
    await useVoice.getState().cancelListening();
    expect(tauri.wakeCancel).toHaveBeenCalled();
    expect(tauri.wakeTranscribe).not.toHaveBeenCalled();
  });

  it("warms the natural voice while the user speaks", async () => {
    useVoice.setState({ settings: naturalSettings });
    tauri.warmNaturalVoice.mockResolvedValue(null);
    await useVoice.getState().startListening();
    expect(tauri.warmNaturalVoice).toHaveBeenCalled();
  });

  it("reads with the natural voice, and says so once when it fails", async () => {
    useVoice.setState({ settings: naturalSettings });
    tauri.speakNatural.mockRejectedValue({ kind: "voice", message: "engine missing" });
    useVoice.getState().speak("t1", "Good evening, Master. All is well.");
    expect(tauri.speakNatural).toHaveBeenCalledWith("Good evening, Master. All is well.");
    await vi.waitFor(() => expect(useVoice.getState().error).toMatch(/natural voice failed/));
    expect(useVoice.getState().speaking).toBeNull();
  });

  it("ignores a second start while already listening", async () => {
    await useVoice.getState().startListening();
    await useVoice.getState().startListening();
    expect(recorder.start).toHaveBeenCalledTimes(1);
  });
});
