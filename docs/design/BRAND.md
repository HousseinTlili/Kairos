# Kairos — brand

**Subject.** Kairos is a butler who is also an excellent engineer, living on one Windows PC. Its
name is the Greek for *the right moment* (as opposed to *chronos*, clock time). **Audience:** one
person, its Master, every day, often by voice. **Primary job:** hand Kairos a task and see, at a
glance, what it is doing and what it was allowed to do.

**Idea: a fine watch movement.** Not a generic dark dashboard: the materials of a watch dial. A
midnight dial of blued steel, gilt for what Kairos offers, lume (the glow paint on watch hands)
for what is live right now, and garnet (a watch's jewel bearings) for what was stopped. The mark
is a watch face whose hands show the real time; it becomes **the Dial**, the one memorable
element: Kairos's presence, which changes with what it is doing.

## Colour

| Name | Dark (default) | Light ("silver dial") | Role |
|---|---|---|---|
| Midnight | `#0E1626` | `#EEF0F3` | The room: page background |
| Dial | `#142036` | `#FFFFFF` | Surfaces: panels, the composer |
| Bezel | `#24334F` | `#D5DAE2` | Lines and edges |
| Gilt | `#D2AD62` | `#946F2A` | Kairos's accent: primary buttons, the selected item, the mark |
| Lume | `#8FE3C3` | `#1E8C68` | Live now: listening, working, connected, the level meter |
| Garnet | `#E8737B` | `#B23A44` | Blocked, denied, failed, delete |
| Enamel | `#E8ECF3` | `#18202E` | Text |
| Slate | `#8E9BB3` | `#5C677A` | Secondary text |

Gilt is never used for "live" and lume never for "click me": the two meanings stay apart, so a
glance tells you whether something is an offer or a state.

## Type

- **Segoe UI Variable** (Windows's own, nothing to download or ship) for everything you operate:
  Text at 14–15 px, Display at large sizes, tabular figures for every number that changes.
- **Sitka** (Windows's own serif) only for **Kairos speaking**: the greeting, the welcome-back
  briefing, the persona's own lines. The serif is its voice; the sans is the instrument.
- Cascadia Mono only for real code and commands, never for labels.
- Scale (≈ 1.2): 12 · 13 · 15 · 18 · 22 · 28 · 40. Sentence case everywhere; no all-caps labels.

## Layout

```
┌──────────┬───────────────────────────────────────────┐
│ ◴ Kairos │  title                         approvals  │   sidebar: the Dial, then
│ + New    │                                           │   new conversation, places,
│ System   │        conversation column, 760 px        │   conversations by day
│ Alerts   │        left-aligned, centred in the pane  │
│ …        │                                           │
│ Today    │   ┌ composer ───────────────── 🎙  ↑ ┐    │
│ ⋯        │   └──────────────────────────────────┘    │
└──────────┴───────────────────────────────────────────┘

Settings:
┌──────────┬────────────┬──────────────────────────────┐
│ sidebar  │ Kairos     │  section title               │   a list of sections in four
│          │  Persona   │  content, 720 px max,        │   groups replaces the tab row
│          │  Voice …   │  never wider than the pane   │   that overflowed
│          │ Safety     │                              │
│          │  Rules …   │                              │
└──────────┴────────────┴──────────────────────────────┘
```

Content is left-aligned. Panels exist only where something is a unit you act on (a rule, an
account, an approval); plain sections are separated by space and one bezel line, not boxed.
Radii follow hierarchy: 14 px for the composer and approval cards, 8 px for controls, 4 px for
chips; no shadow except under the quick box, which floats over other apps.

## Principles

1. **One bold thing: the Dial.** At rest its gilt hands show the time. Listening, a lume ring
   follows your voice; working, a second hand sweeps; speaking, the ring breathes with the
   speech. Everything else stays still and quiet. Motion only answers what Kairos or you are
   doing, and reduced-motion turns the sweep into a steady ring.
2. **States read at a glance.** Lume = live, gilt = offer, garnet = stopped. Never colour alone:
   every state also has words.
3. **Native, then refined.** Windows fonts, Windows scrollbars and controls told they are dark
   (`color-scheme`), then styled: no white dropdowns in a dark room.
4. **Nothing overflows.** No page scrolls sideways; long commands and paths wrap.

## Review against the generic defaults

- *Near-black with one acid accent:* the room is midnight blue, not black, with two accents
  whose meanings differ. Kept, with that distinction written down.
- *Cream and terracotta serif:* the light theme is cool silver, and the serif is for Kairos's
  voice only.
- *The SaaS card kit:* the System page and settings were exactly that (identical rounded cards,
  ALL-CAPS labels). Revised: panels only for units you act on, sentence-case labels, numbers
  set as instrument readouts.
- *Template chrome* (eyebrow labels, "A · B · C" meta strings, mono labels, "→" on buttons):
  removed where it exists (the usage line "13k in · 190 out" becomes "13k tokens in, 190 out").
