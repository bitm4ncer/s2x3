# Depth readback never completes (frame stalls)

- Engine commit: `321776e` (2026-11-24, ssx3-decomp main)
- Found: 2026-11-24, while playing on the reference laptop
- Status: open (needs an upstream fix)
- Context: Linux/XWayland, Intel Iris Xe (Mesa 26.2.6), borderless fullscreen,
  60 Hz tick, default settings

## Symptoms

The game reports 60 game updates per second, but movement is not smooth: a
short hitch on every frame. The log fills with one line per frame:

```
[gfx] depth of frame 269: the readback never completed
[gfx] depth of frame 270: the readback never completed
```

No crash, audio is fine, and the image is correct.

## Reproduction

1. `SSX3_FULLSCREEN=1`, `SSX3_SKIP_MOVIES=1`, PS2 USA disc.
2. Enter a race (or the front end); watch `launcher.log`.
3. Every frame logs the message above; frame pacing is visibly uneven.

## Evidence

- `~/Games/SSX3-ps2/play.log`: hundreds of consecutive "readback never
  completed" lines, one per frame, in menus and in a race.
- `crates/ssx3-gfx/src/lib.rs` (`DepthPost::Buf`): the render thread maps the
  depth buffer and polls until `DEPTH_GIVE_UP` (60 ms), then logs and unmaps.
- `docs/plans/native-rendering.md` says the light query reads the depth of
  the frame two back and waits for the render thread when it is late
  (`SSX3_DEPTH_SYNC=0` for the latest).

## Hypothesis

On this GPU/driver combination the buffer map is never signalled (or the
queue answers "empty" while the copy is still pending), so every frame waits
the full 60 ms before giving up. Since the game still reaches 60 updates/s,
the wait is probably on the render thread and the frame is presented late,
which shows as uneven pacing.

## Proposal

Upstream, one of:

- Detect the "map never completes" case once per process and skip the light
  query for the rest of the run (log once, not per frame).
- Make the readback fully asynchronous: never block the frame, use the last
  available depth, and mark the query as "no answer" instead of stalling.
- Add a switch (e.g. `SSX3_DEPTH_QUERY=0`) for GPUs where the map is broken.

Test: a scripted race with frame-time percentiles, plus the RAM gate (the
query's result feeds the flare visibility, so a skipped query must be a known
deviation).

## Launcher workaround (not a fix)

`SSX3_DEPTH_SYNC=0` is worth trying to see whether the latest-depth path
avoids the stall. Slope3 must not depend on an unmerged engine patch; if the
upstream fix lands, pin the engine version in the installer.
