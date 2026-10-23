# Fixtures

Real responses recorded from a local Hermes Agent API server on 2026-10-23, used as test input
for the Kairos client. Never edit them by hand to make a test pass; record new ones instead.

## `hermes-0.22.5/`

Hermes 0.22.5 (commit `bf0203a`), Windows 11, native install, model `upstage/solar-mini4:free`
via Nous Portal.

| Folder | What happened |
|---|---|
| `meta/` | `/health`, `/v1/capabilities`, `/v1/models`, `/v1/toolsets`, the `/v1/skills` server error, and a request with no key (401) |
| `run-simple/` | A plain question: create → events → status, then the same `Idempotency-Key` retried (replayed) and reused with a different body (conflict) |
| `run-smart-autoapprove/` | `rm -rf` on a scratch folder under approval mode `smart`: **auto-approved, no human prompt** |
| `run-approval-deny/` | The same command under `manual`: `approval.request`, a stale-generation answer (rejected), a `deny`, the final state |
| `run-stop/` | A long `ping`, stopped mid-run → `run.cancelled` |

## Sanitising

Recorded responses can contain personal data. Before committing a new fixture:

- Replace the Windows user name in paths with `USER`.
- Never commit `/api/sessions` or `/api/model/options` output from a real profile: they contain
  the Windows account SID, chat titles and previews, and account details.
- Check: `grep -rniE "<your user name>|S-1-5-21|@|API_SERVER_KEY" fixtures/hermes-*` must print
  nothing.
- Fixture bytes are kept exactly as recorded (`.gitattributes` marks them `-text`, so Git never
  rewrites line endings).
