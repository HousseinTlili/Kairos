import { describe, expect, it } from "vitest";
import {
  MAX_SECONDS,
  SpeechEnd,
  plausibleAfterWake,
  downmix,
  guessLanguage,
  joinChunks,
  level,
  missingVoices,
  pickVoice,
  resample,
  speechText,
  splitSentences,
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

describe("the natural voice's pieces", () => {
  it("reads sentence by sentence, joining very short ones to the next", () => {
    expect(splitSentences("Done, Master. I opened Notepad. Anything else? Yes.")).toEqual([
      "Done, Master. I opened Notepad.",
      "Anything else? Yes.",
    ]);
  });

  it("cuts a long sentence at a comma or a space", () => {
    const long = `${"word ".repeat(80)}end.`;
    const pieces = splitSentences(long, 24, 120);
    expect(pieces.length).toBeGreaterThan(2);
    expect(pieces.every((p) => p.length <= 120)).toBe(true);
    expect(pieces.join(" ")).toBe(long);
  });

  it("gives nothing for nothing", () => {
    expect(splitSentences("   ")).toEqual([]);
  });
});

describe("after the wake word", () => {
  it("a noise too short to be a request sends nothing", () => {
    const end = new SpeechEnd();
    end.feed(0.01, 0);
    end.feed(0.5, 100);
    end.feed(0.5, 200);
    expect(end.feed(0.02, 300)).toBe("speaking");
    expect(end.feed(0.02, 1600)).toBe("nothing");
  });

  it("drops a transcript in a language the Master doesn't speak", () => {
    expect(plausibleAfterWake("en")).toBe(true);
    expect(plausibleAfterWake("ar")).toBe(true);
    expect(plausibleAfterWake("pt")).toBe(false);
    expect(plausibleAfterWake("ru")).toBe(false);
    expect(plausibleAfterWake(null)).toBe(true);
  });

  it("is done after a pause that follows speech", () => {
    const end = new SpeechEnd();
    expect(end.feed(0.02, 100)).toBe("waiting");
    expect(end.feed(0.4, 500)).toBe("speaking");
    expect(end.feed(0.03, 900)).toBe("speaking");
    expect(end.feed(0.02, 1900)).toBe("speaking");
    expect(end.feed(0.02, 2150)).toBe("done");
  });

  it("a short breath between words doesn't end it", () => {
    const end = new SpeechEnd();
    end.feed(0.5, 100);
    end.feed(0.02, 300);
    expect(end.feed(0.5, 900)).toBe("speaking");
    expect(end.feed(0.02, 1500)).toBe("speaking");
  });

  it("gives up when nothing is said", () => {
    const end = new SpeechEnd();
    expect(end.feed(0.01, 3000)).toBe("waiting");
    expect(end.feed(0.01, 6000)).toBe("nothing");
  });
});
