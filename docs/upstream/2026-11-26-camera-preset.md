# Camera 1/2 preset from the launcher (Near / Mid / Far)

- Engine commit: `3d771fc` (checkout `~/Projects/ssx3/ssx3-decomp`)
- Found: 2026-11-26, launcher request
- Status: proposed, not started
- s2X3 setting (planned): `SSX3_CAM1` (choice: near / mid / far), `SSX3_CAM2`

## Request

The game's Options screen has `Camera 1` and `Camera 2` (Near / Mid / Far).
The launcher should offer the same choice so it is applied automatically at
boot, without opening the in-game options.

## Current state

- The engine has no `SSX3_CAM*` variable: `crates/ssx3/src/settings.rs`
  `ITEMS` has no camera entry, and no code reads one.
- The Options screen is the game's own frontend code; the choice is stored in
  the game's options state (persisted with the save on the memory card).

## Plan

1. Find where the Options screen writes the `Camera 1` choice (the frontend
   options menu code in `crates/ssx3-game`; likely a byte in an options
   struct, 0 = Near, 1 = Mid, 2 = Far — confirm with the recompiled code).
2. Add `SSX3_CAM1` / `SSX3_CAM2` (choice, applied at boot like the other
   `SSX3_*` settings): poke the options value when the variable is set,
   before the frontend reads it. Keep the game's own menu authoritative
   afterwards.
3. Launcher: nothing to change — the session settings UI renders the
   engine's `settings_schema()` dynamically and passes values as
   environment variables on launch.

## Acceptance

- Setting `SSX3_CAM1=far` boots into a race with the far camera without
  touching the in-game options.
- The in-game Options screen reflects the preset value.
- Differential test / RAM gate stays green (the poke must happen at a point
  where the game would normally have read the value).
