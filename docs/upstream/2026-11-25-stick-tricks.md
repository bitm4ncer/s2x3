# Stick tricks (d-pad on a stick)

- Engine commit: `321776e` (branch `slope3/stick-tricks`, local patch)
- Found: 2026-11-25, request while playing with an Xbox pad
- Status: patched locally, to be proposed upstream
- Slope3 setting: `SSX3_STICK_TRICKS=right` (choice: off / right / left)

## Symptoms / request

SSX 3 tricks use the d-pad plus a face button. On an Xbox pad the d-pad is
awkward during a run; the player asked for the sticks instead. The engine has
no such option: the d-pad bits come only from the physical d-pad, and the
sticks are passed to the game as the PS2 analog sticks.

## Proposal

In `crates/ssx3/src/lib.rs` (`App::update_pad`), after the button bits:

```rust
// SSX3_STICK_TRICKS=right|left
const DEAD: f32 = 0.55;
let x = pad.value(ax);
let y = pad.value(ay);
if x >  DEAD { *buttons |= 1 << 5; }        // d-pad right
if x < -DEAD { *buttons |= 1 << 7; }        // d-pad left
if y >  DEAD { *buttons |= 1 << 4; }        // d-pad up
if y < -DEAD { *buttons |= 1 << 6; }        // d-pad down
```

- `right` is the intended mode: the right stick is otherwise little used in a
  race, while the left stick steers.
- `left` exists for players who want it, but steering then also presses the
  d-pad, which can trigger tricks unintentionally.
- Off by default; no change for anyone who does not set it.

## Test

Start a race, push the chosen stick diagonally and press a face button: the
trick fires without touching the d-pad. Check that the camera/steering feel
unchanged. The RAM gate is unaffected (input path only), but a scripted run
with the option set should be added before the PR.

## Notes

- Slope3 exposes it as a choice field (`Stick tricks`: off / right / left).
- Until merged, the launcher must tolerate an engine that ignores the
  variable (older builds simply do nothing).
