# Upstream notes

One file per engine bug, named `YYYY-MM-DD-<slug>.md`. The rule: the fix
lives in `ssx3-decomp`, never in s2X3 (see `AGENTS.md`).

Template:

```md
# <title>

- Engine commit: <sha>
- Found: <date>, by <name>
- Status: open | patched locally | PR #<n> | merged
- s2X3 setting/context: <tick rate, resolution, ...>

## Symptoms
What the player sees. Include screenshots and log lines.

## Reproduction
Steps, disc serial, settings, frame numbers, save file if needed.

## Evidence
Log excerpts, `SSX3_*` debug output, xemu/PCSX2 comparison.

## Hypothesis
First suspect and why.

## Proposal
Minimal change, affected files, test/scenario to add.

## Notes
Anything the launcher must know (version gate, workaround, revert plan).
```

Use `git format-patch` output or a branch link once a PR exists.
