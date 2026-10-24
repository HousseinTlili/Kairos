"""The Kairos plugin for Hermes: checks every tool call against Kairos's rules and records it.

The hook wiring (`register(ctx)`) arrives in M2 Step 4; until then this package only holds the
policy engine (`policy.py`) and its starter rules (`policy.default.json`).
"""
