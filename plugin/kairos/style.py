"""How Kairos's answers are shaped (M13 §4): tables, diagrams, charts and pictures only when they
help. Kairos's chat draws Markdown tables, ```mermaid diagrams, ```svg sketches and the images
Hermes attaches; Telegram or the CLI don't, so the note goes only to Kairos's own conversations,
whose session ids start with "kairos-".
"""

from __future__ import annotations

import logging
from typing import Any

log = logging.getLogger("kairos.style")

KAIROS_SESSION = "kairos-"

STYLE = (
    "Format for Kairos's screen. Default to plain sentences; a short answer stays short. "
    "Use a Markdown table when comparing three or more things on two or more points, or when "
    "the Master asks for one. Use one Mermaid diagram (a ```mermaid block) for a process with steps "
    "or branches, how people or parts are organised or connect, a timeline, or when asked; keep it "
    "under 15 boxes. Use a Mermaid pie or xychart-beta chart for numbers that compare or change. "
    "When the Master asks for a drawing or a sketch, write a small SVG in the answer itself (a "
    "```svg block, under 20 KB, no scripts or links), never to a file; Kairos shows it. Generate "
    "an image only when the Master asks for a picture. Never add a table, diagram or picture as "
    "decoration, and never more than one of them unless asked."
)


def steering(session_id: str = "", **_: Any) -> dict | None:
    """The pre_llm_call hook: the note above on every turn of a Kairos conversation, nothing
    elsewhere (Hermes appends it to the user message, which keeps the prompt cache intact)."""
    try:
        if isinstance(session_id, str) and session_id.startswith(KAIROS_SESSION):
            return {"context": STYLE}
    except Exception:  # noqa: BLE001 — a hook never raises into Hermes
        log.exception("kairos: style steering failed")
    return None
