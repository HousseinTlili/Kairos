# M9 — Voice: acceptance

**Status (2026-10-25):** built on `m9/voice`; checks run by the agent on the PC are below. The
product owner's checklist at the end is not run yet. Not signed off.

## Checks run on the PC by the agent (2026-10-25)

Windows 11, the product owner's laptop (i7-13650HX, 24 GB).

| Check | Result |
|---|---|
| `whisper-rs 0.16.0` builds from a normal shell | Yes, with `.cargo/config.toml` pointing at VS Build Tools' CMake and LLVM's libclang (24–35 s) |
| Rust tests (`cargo test`) | 173 passed, 2 ignored (the two below) |
| `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` | Clean |
| Real download: `the_small_model_downloads_and_checks_out` (ignored test) | Passed: 190 MB from huggingface.co in 33 s through Kairos's own download code, SHA-256 matched, now in `%APPDATA%\kairos\models` |
| Real transcription: `the_fixture_sentence_is_transcribed_quickly` (ignored test, Small model) | Passed: `fixtures/voice/open-notepad.wav` (Windows's Zira voice) → "Please open notepad for me.", language `en`, **2.1 s** including the model load |
| TS tests (`pnpm -C app test`), `pnpm -C app lint`, `tsc` | 118 passed (16 new); clean |
| Dev app starts; Ctrl+Alt+K and Ctrl+Alt+J both registered | Yes: no "Windows refused" line |
| The Voice page, mic, push-to-talk and speech in the real app | **Not run by the agent**: screen access to the app was declined; the product owner's checklist below covers them |

**Found and fixed while building:** whisper.cpp was compiled **without optimisation** under MSVC.
The cmake crate replaces CMake's Release flags and drops `/O2` (the generated project had an empty
`<Optimization>`), so the first real transcription took 34 s. `.cargo/config.toml` now sets
`CMAKE_C_FLAGS_RELEASE` / `CMAKE_CXX_FLAGS_RELEASE`, and the audio window is cut to the
recording's length (`stt::audio_ctx`): 34 s → 4.2 s → 2.1 s, same text. GPU (CUDA on the RTX
4060) would be faster still; it is not in M9 (M9 §7).

## Product owner's checklist (about 15 minutes)

Before: the dev app running (`pnpm -C app tauri dev`), Hermes running, a microphone that works in
other apps. The Small model is already downloaded.

| # | Do | Look for | Result |
|---|---|---|---|
| 1 | Settings ▸ **Voice** | Small shows **Ready**; Base shows Download; Language "Detect each time"; push-to-talk key Ctrl+Alt+J | |
| 2 | Click the mic in "Test", say "Open Notepad", click again | Windows/WebView2 may ask for the microphone once (allow). Then "Transcribing…" and your words in quotes, "Heard in English" | |
| 3 | Same in **French** ("Ouvre le bloc-notes") and **Arabic** ("افتح المفكرة") | Correct text; "Heard in French" / "Heard in Arabic" | |
| 4 | In a chat, click the mic, ask a short question, click again | The question is sent as if typed and answered | |
| 5 | Over another app, **hold Ctrl+Alt+J**, speak, let go | The quick box appears listening, then sends what you said and answers | |
| 6 | Click the mic, then **Esc** | Recording cancelled; nothing sent | |
| 7 | Click the mic and say nothing for 3 s, click again | "Didn't catch that"; nothing sent | |
| 8 | **Test the voice** in Settings ▸ Voice | Kairos speaks the test sentence | |
| 9 | Speaker button under an answer; then Esc | The answer is read (code skipped); Esc stops it | |
| 10 | Ask by voice (mic or Ctrl+Alt+J) | The answer is **spoken without any click** ("Answer out loud when I speak", on by default); a typed question stays silent | |
| 10b | Turn on **Read typed questions' answers aloud too**, type a question | The new answer is read once; reopening the conversation doesn't read old ones | |
| 11 | Settings ▸ Briefing ▸ Brief me now, then **Read it to me** | The welcome-back facts are read | |
| 12 | Optional: add a French voice (Windows Settings ▸ Time & language ▸ Speech), ask in French | The answer is read with the French voice | |
| 13 | Turn Wi-Fi off and repeat step 2 | Still transcribes: nothing goes online | |

Write down anything that looked wrong, including how long step 2 took.

## Step 8 — Kokoro, the natural voice (2026-10-26, checks run by the agent)

| Check | Result |
|---|---|
| Downloads (`the_natural_voice_installs`, ignored test, Kairos's own code) | sherpa-onnx 1.13.8 (24.8 MB) and Kokoro en v0.19 (320 MB) fetched from GitHub, SHA-256 matched, unpacked with Windows's `tar.exe` into `%APPDATA%\kairos\voices`, archives removed: 95 s |
| Engine kept loaded (`george_speaks_a_sentence_quickly`) | Cold: 7.9 s for the first sentence (loading), then 0.8 s. Files cached: 1.8 s, then 0.3 s |
| Inside the dev app (`speak_natural` through WebView2's debugging port) | WAV returned as an `ArrayBuffer`: 4.5 s for the first sentence (with loading), 1.1 s for the next |
| Settings ▸ Voice ▸ Test the voice, in the dev app | George started **1.1 s** after the click (engine already loaded) and played 4.9 s; no error |
| Rust tests, clippy, fmt | 182 passed; clean |
| TS tests, lint, tsc | 127 passed; clean |

**Product owner, on the PC:**

| # | Do | Look for | Result |
|---|---|---|---|
| 14 | Settings ▸ Voice: the natural voice is already downloaded (the agent's check); Natural voice and George are selected | Test the voice: George, smooth | |
| 15 | Ask something by voice | The answer is read by George; the first words come within about 2 s of the answer appearing | |
| 16 | A long answer | Read sentence after sentence without long gaps; Esc stops it | |
| 17 | Switch to Windows voice, then back | Each is used; nothing else changes | |
| 18 | Remove the natural voice (asks first), then ask by voice | The Windows voice reads it; Download is offered again | |

## Step 9 — The wake word "Kairos" (2026-10-27, checks run by the agent)

| Check | Result |
|---|---|
| Download the wake word (17.6 MB, SHA-256 checked) | Installed under `%APPDATA%\kairos\voices` |
| George's clips through the real spotter (ignored Rust test) | "Kairos, open Notepad" and "Hey Kairos…" wake it; two sentences without the name don't |
| Thresholds on those clips | Found at 0.15, 0.25, 0.28, 0.30, 0.32; missed at 0.35 and 0.45 |
| In the dev app, a clip fed to `wake_feed` | Wakes at the 5th–6th chunk; the quick box opens and listens; 6 s of silence cancels it |
| Live, on the PC's microphone, at 0.25 | **Two stray requests sent** ("E aí, Kairos?", a Russian sentence). Guards added (under 0.4 s of speech, or a language other than en/fr/ar, sends nothing) and normal moved to 0.28. Wake word left **off** |
| Rust tests, clippy | Wake tests pass; clippy clean |
| TS tests, lint, tsc | 140 passed; clean |
| The product owner's own voice, spotter alone (three recordings, 2026-10-27) | **Never found**, at any threshold down to 0.10 and with ~17,700 sound-alike spellings; it hears only "GA I E", "KA I O", "GA U" |
| Same recordings, two steps (Whisper confirms; ignored Rust test) | **5 of 5** wake it, one by one (Whisper: "Gairo's", "Kairos" ×3, "Keiras"); George's 2 of 2; the two sentences without the name 0 |
| Live, in the dev app: George's "Hey Kairos, what time is it?" fed to `wake_feed` | Wakes at 1.8 s; the quick box sends **"what time is it?"** (the name dropped, nothing lost) and Kairos answers |
| Rust tests, clippy, fmt (two steps) | 197 + 19 passed; clean |
| TS tests, lint, tsc (two steps) | 144 passed; clean |

**Product owner, on the PC:**

| # | Do | Look for | Result |
|---|---|---|---|
| 19 | Settings ▸ Voice ▸ Wake word: switch on "Listen for Kairos" (Normal) | Windows shows the microphone in use || Works (product owner, live, 2026-10-27) |
| 20 | Say "Kairos, what time is it?" from your seat | The quick box opens, listens, sends when you pause, answers aloud || Works (product owner, live, 2026-10-27) |
| 21 | Say "Kairos", then nothing | The box opens and quietly gives up after about 6 s; nothing is sent | |
| 22 | Talk, play a video, leave it on for 30 min | No wake, and nothing sent, without the name. If it wakes by itself, try Strict | |
| 23 | A French and an Arabic request after "Kairos" | Both are sent | |
| 24 | Switch it off | The microphone indicator goes away | |
