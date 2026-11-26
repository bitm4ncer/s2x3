# dump-textures: offline texture extraction (upstream engine)

**Status:** implemented locally on `s2x3/dump-textures` (worktree
`C:\Users\konta\Projects\ssx3\sdump`, commits `3f28c52`, `632a47f`,
`415a870`); PR against `andersfischernielsen/ssx3-decomp` pending push.

**Symptom:** the launcher's "Texturen neu lesen" needs `<engine> dump-textures`,
which did not exist; the fallback was one game session (`SSX3_TEX_DUMP`) before
any pack could be generated.

**What the branch adds**

- `ssx3-core::refpack` — EA RefPack (QFS) decoder (ported from the LUI
  patch's decoder; verified against all 6651 RefPack-compressed SSH members
  of the disc).
- `ssx3-core::iso::Disc::walk` — full-disc file listing.
- `ssx3-gfx::file_tex` — SHPS/BIGF decoding to the runtime's texture names
  (the file→TexKey mapping: FNV-1a over the file's linear pixel bytes + the
  CLUT, TexMan format 9/0xb/0, TEX0 with PSM 0x1b/0x14/0x00 + TCC + the
  indexed flag at bit 61, TEXA `0x8000000080`). The protocol and the
  exclusions are in `docs/notes/dump-textures-parity.md` (engine repo).
- `ssx3 dump-textures --iso <p> --out <dir> [--force]` — walks the disc,
  writes `<name>.png` per texture (one per name, idempotent), reports bad
  archives with their paths and exits non-zero.

**Parity:** every SSH sheet on the disc decodes offline (5714 textures);
all six textures cross-checked against runtime dumps match by name
(`over`, `help`, `load`, `Widg`, `moun`, `ssxt`). The runtime's remaining
textures come from non-SSH sources (LUI front-end screens, models) and are
excluded for now — the launcher covers them with the runtime dump that every
session grows (`SSX3_TEX_DUMP` at launch, launcher side).

**Launcher side (this repo, `3d5ee42`):** "Texturen neu lesen" runs the
extractor on every click (it is idempotent) and regenerates the pack; every
game launch passes `SSX3_TEX_DUMP` so sessions complete the dump invisibly.
