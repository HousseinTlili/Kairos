# Kairos — brand: the illuminated charter

**Subject.** Kairos serves one person, its Master, from inside a Windows PC: a steward of a noble
house who is also an excellent engineer. **Primary job:** receive the Master's wishes (often
spoken) and show, at a glance, what it is doing and what it was given leave to do.

**Idea (chosen by the product owner on 2026-10-26).** A medieval charter of a noble house on
charcoal: gold leaf for what Kairos offers, crimson (*gules*) for what is refused or stopped, vert
for what is live, vellum-coloured text, a blackletter name and illuminated initials. Kairos's own
words are set like a manuscript; the controls stay plain and modern, so the house is grand and
still easy to run. Supersedes the "watch movement" brand of 2026-10-26.

## Colour

| Name | Dark (default) | Light ("vellum") | Role |
|---|---|---|---|
| Charcoal | `#121110` (near black, 2026-10-26) | `#EDE5D4` | The main colour: the room, with the diaper pattern |
| Hearth | `#1B1917` | `#F7F1E4` | Surfaces: panels, the composer (solid, no pattern) |
| Ember | `#24211D` | `#E5DCC8` | Raised and hovered |
| Iron | `#35312B` | `#CDBFA3` | Lines and edges |
| Gold leaf | `#C9A043` | `#86661D` | Offers: the main button, the selected item, ornaments, the seal's ring |
| Gules | `#C4474C` (text `#E5847F`) | `#9C2C30` | Refused, blocked, failed, delete; the seal's wax |
| Vert | `#7DB394` | `#2E7A55` | Live now: listening, working, connected, unread |
| Vellum | `#ECE4D3` | `#2A2420` (iron-gall ink) | Text |
| Ash | `#A89F8E` | `#6B5F50` | Secondary text |

The sidebar is charcoal in both themes: the house's dark panelling. Gold never means "live" and
vert never means "click me".

## Type

- **UnifrakturCook** (blackletter, bundled, OFL) for the name *Kairos* and the illuminated initial
  that opens Kairos's greeting and briefing. Never for anything you must read quickly.
- **EB Garamond** (bundled, OFL) for Kairos's voice: the greeting, its answers, the briefing, page
  titles. Answers are set at 17 px with generous leading, like a page of a book.
- **Segoe UI Variable** (Windows's own) for everything you operate: buttons, menus, settings,
  numbers (tabular figures). What *you* write in the chat is Segoe too: who is speaking shows
  before a word is read.
- Cascadia Mono only for real commands and code.
- Sentence case; no all-caps labels.

## Ornament (spent in few places)

1. **The seal**: a crimson wax seal with a gold ring and a blackletter K. It is Kairos's presence:
   at rest it is still; listening, a vert ring follows the voice; working, a gold ring turns;
   speaking, the ring breathes. The only thing that moves.
2. **Illuminated initials**: the first letter of the greeting and the briefing, in gold
   blackletter, three lines tall.
3. **The fleuron ❧** marks suggestions and the starts of sections in Kairos's briefing.
4. **The diaper**: the room's background is a manuscript diaper, a lattice of fine gold lozenges
   with a quatrefoil in each (a 56 px SVG tile in `global.css`, `--pattern`), at about 8 %
   opacity. Only the room shows it; panels, the composer and the sidebar are solid.

No borders of knotwork, no parchment textures, no fake torn edges: the charter is suggested by
type, colour and one quiet pattern, not costume.

## Words (a light touch, chosen 2026-10-26)

Kairos's own lines are courtly; the app's names stay plain.

- Kairos: "Good evening, Master." · "What is your will?" · approvals read "Kairos asks your
  leave" with **Grant once** / **Always allow** / **Refuse**.
- Menus, settings and pages keep their plain names (Chat, System, Rules, Settings).

## Layout

The sidebar folds to a rail (Ctrl+B); a 760 px conversation column whose message box holds the
model menu on its toolbar, beside the mic and send; Settings as a grouped
section list beside its content, nothing wider than the window, Windows controls told the theme.
Panels only for things you act on (an approval, a rule, an account); radii 10 px for panels, 6 px
for controls, 4 px for chips.

## Review against the generic defaults

- *Cream background with a serif and terracotta:* the default room is charcoal; the light theme
  is vellum because the subject is literally a charter, and its accents are gold and crimson, not
  clay.
- *Near-black with one accent:* charcoal is warm and lighter than black, with two accents whose
  meanings differ (gold = offer, vert = live) and crimson for refusal.
- *Costume medievalism* (parchment textures, knotwork, Old-English everywhere): blackletter is
  limited to the name and the initials; everything you operate is modern.
