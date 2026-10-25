import { describe, expect, it } from "vitest";
import {
  MAX_SECONDS,
  downmix,
  guessLanguage,
  joinChunks,
  level,
  missingVoices,
  pickVoice,
  resample,
  speechText,
  toPcm16,
} from "./model";

describe("recording", () => {
  it("averages stereo into mono", () => {
    const mono = downmix([new Float32Array([1, 0]), new Float32Array([0, 1])]);
    expect(Array.from(mono)).toEqual([0.5, 0.5]);
  });

  it("resamples 48 kHz to 16 kHz at a third of the length, keeping the shape", () => {
    const input = Float32Array.from({ length: 4800 }, (_, i) => Math.sin(i / 20));
    const out = resample(input, 48_000);
    expect(out.length).toBe(1600);
    expect(out[100]).toBeCloseTo(input[300], 5);
    expect(resample(input, 16_000)).toBe(input);
  });

  it("stops at a minute however long the key was held", () => {
    const second = new Float32Array(16_000);
    const chunks = Array.from({ length: MAX_SECONDS + 5 }, () => second);
    expect(joinChunks(chunks).length).toBe(MAX_SECONDS * 16_000);
    expect(joinChunks([new Float32Array([1, 2]), new Float32Array([3])])).toEqual(
      new Float32Array([1, 2, 3]),
    );
  });

  it("converts to 16-bit samples, clipping anything louder than full scale", () => {
    expect(Array.from(toPcm16(new Float32Array([0, 1, -1, 2, -2, 0.5])))).toEqual([
      0, 32767, -32768, 32767, -32768, 16384,
    ]);
  });

  it("shows silence as an empty meter and speech as a full one", () => {
    expect(level(new Float32Array(100))).toBe(0);
    expect(level(new Float32Array(100).fill(0.5))).toBe(1);
  });
});

describe("reading answers aloud", () => {
  it("skips code and reads links and Markdown as plain words", () => {
    const answer = [
      "## Done",
      "I opened **Notepad** and ran `dir`. See [the docs](https://example.com/docs).",
      "",
      "```powershell",
      "Get-Process | Sort-Object CPU",
      "```",
      "- first point",
      "> quoted",
      "More at https://example.com.",
    ].join("\n");
    const spoken = speechText(answer);
    expect(spoken).not.toMatch(/[#*`>]|Get-Process|https/);
    expect(spoken).toContain("I opened Notepad and ran dir. See the docs.");
    expect(spoken).toContain("(code block skipped)");
    expect(spoken).toContain("first point");
    expect(spoken).toContain("More at a link");
  });

  it("guesses the language of an answer to pick a voice", () => {
    expect(guessLanguage("J'ai ouvert le Bloc-notes et la calculatrice pour vous.")).toBe("fr");
    expect(guessLanguage("فتحت المفكرة لك")).toBe("ar");
    expect(guessLanguage("I opened Notepad for you.")).toBe("en");
    expect(guessLanguage("")).toBe("en");
  });

  it("uses a voice for the language, else the default one", () => {
    const voices = [
      { name: "Zira", lang: "en-US", default: true },
      { name: "Hortense", lang: "fr-FR" },
    ];
    expect(pickVoice(voices, "fr")?.name).toBe("Hortense");
    expect(pickVoice(voices, "ar")?.name).toBe("Zira");
    expect(pickVoice([], "en")).toBeNull();
  });

  it("names the languages that have no Windows voice yet", () => {
    expect(missingVoices([{ name: "Zira", lang: "en-US" }])).toEqual(["fr", "ar"]);
    expect(
      missingVoices([
        { name: "a", lang: "en-US" },
        { name: "b", lang: "fr-FR" },
        { name: "c", lang: "ar-SA" },
      ]),
    ).toEqual([]);
  });
});
