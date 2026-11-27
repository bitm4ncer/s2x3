//! Automatic Xbox button-prompt patcher.
//!
//! The game's PlayStation button glyphs live in two known UI atlases. When the
//! user picks Xbox prompts, the launcher patches those two textures out of the
//! texture dump into the texreplace folder so the engine shows A/B/X/Y in game.
//!
//! Flow (all in the background, no user action):
//! 1. On play, if the atlases are not in the dump yet, the launcher adds
//!    `SSX3_TEX_DUMP` so this run writes the dump.
//! 2. When the game quits, [`patch`] reads the two atlases from the dump and
//!    writes the patched copies into the texreplace folder.
//! 3. The next launch passes `SSX3_TEX_REPLACE` and the game shows Xbox glyphs.
//!
//! The patch contains no game art: the input atlas stays on the user's disk,
//! the output is our own glyph art drawn over the four symbol slots.

use std::path::Path;

use image::imageops::FilterType;

/// The PS2 glyph art every sheet shares, one sprite per face slot — the
/// fingerprint that says "this sheet carries button prompts".
const PS2_SLOTS: [&[u8]; 4] = [
    include_bytes!("../assets/glyphs/ps2-slot1.png"),
    include_bytes!("../assets/glyphs/ps2-slot2.png"),
    include_bytes!("../assets/glyphs/ps2-slot3.png"),
    include_bytes!("../assets/glyphs/ps2-slot4.png"),
];

/// Glyph slots in the 256x256 atlas: (x0, y0, x1, y1, letter) — 20x20 each.
/// Mapping is positional, like the engine's pad mapping:
/// triangle=Y, square=X, cross=A, circle=B.
const SLOTS: [(u32, u32, u32, u32, char); 4] = [
    (12, 123, 31, 142, 'y'),
    (34, 123, 53, 142, 'x'),
    (56, 123, 75, 142, 'a'),
    (78, 123, 97, 142, 'b'),
];

const GLYPH_A: &[u8] = include_bytes!("../assets/glyphs/a.png");
const GLYPH_B: &[u8] = include_bytes!("../assets/glyphs/b.png");
const GLYPH_X: &[u8] = include_bytes!("../assets/glyphs/x.png");
const GLYPH_Y: &[u8] = include_bytes!("../assets/glyphs/y.png");

/// Shoulder prompts in the 256x256 atlas: PS2 `R1 L1 / R2 L2` boxes
/// (x0, y0, x1, y1) and the Xbox box art replacing them (RB/LB/RT/LT —
/// PS2 R1 = Xbox RB, L1 = LB, R2 = RT, L2 = LT).
const SHOULDERS: [((u32, u32, u32, u32), char, &[u8]); 4] = [
    (
        (121, 158, 149, 172),
        'r',
        include_bytes!("../assets/glyphs/rb.png"),
    ), // R1 -> RB
    (
        (152, 158, 180, 172),
        'l',
        include_bytes!("../assets/glyphs/lb.png"),
    ), // L1 -> LB
    (
        (121, 175, 149, 189),
        'r',
        include_bytes!("../assets/glyphs/rt.png"),
    ), // R2 -> RT
    (
        (152, 175, 180, 189),
        'l',
        include_bytes!("../assets/glyphs/lt.png"),
    ), // L2 -> LT
];

/// The controller drawing at the sheet's top (its opaque bounds): the PS2
/// pad photo goes, an Xbox pad takes its place (same rect, contained).
const CONTROLLER: (u32, u32, u32, u32) = (7, 5, 177, 112);
const CONTROLLER_PNG: &[u8] = include_bytes!("../assets/glyphs/controller.png");

fn glyph_image(letter: char) -> anyhow::Result<image::RgbaImage> {
    let bytes = match letter {
        'a' => GLYPH_A,
        'b' => GLYPH_B,
        'x' => GLYPH_X,
        'y' => GLYPH_Y,
        _ => anyhow::bail!("unknown glyph `{letter}`"),
    };
    Ok(image::load_from_memory(bytes)?.to_rgba8())
}

/// True when nothing is left to do: every dump texture has been seen by a
/// patch run (the marker holds the dump's file count at patch time).
pub fn complete(dump_dir: &Path, out_dir: &Path) -> bool {
    let marker = out_dir.parent().unwrap_or(out_dir).join(".patched");
    let stored = std::fs::read_to_string(marker).unwrap_or_default();
    let now = std::fs::read_dir(dump_dir)
        .map(|d| d.count())
        .unwrap_or(usize::MAX);
    stored
        .trim()
        .parse::<usize>()
        .map(|n| n >= now)
        .unwrap_or(false)
}

/// True when the dump has texture sheets at all.
pub fn dumped(dump_dir: &Path) -> bool {
    std::fs::read_dir(dump_dir)
        .map(|d| {
            d.flatten().any(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("png"))
            })
        })
        .unwrap_or(false)
}

/// True when the sheet's four face slots still carry the shared PS2 glyph
/// art (exact match): only those sheets get patched.
fn has_the_ps2_glyphs(atlas: &image::RgbaImage) -> bool {
    for (i, &(x0, y0, x1, y1, _)) in SLOTS.iter().enumerate() {
        let Ok(reference) = image::load_from_memory(PS2_SLOTS[i]) else {
            return false;
        };
        let reference = reference.to_rgba8();
        for y in y0..=y1 {
            for x in x0..=x1 {
                if atlas.get_pixel(x, y) != reference.get_pixel((x - x0) as u32, (y - y0) as u32) {
                    return false;
                }
            }
        }
    }
    true
}

/// Is the controller rect holding the dark pad photo (only then is it
/// replaced)? The HUD variants of the sheet carry HUD art there instead.
fn controller_rect_is_a_photo(atlas: &image::RgbaImage) -> bool {
    let (x0, y0, x1, y1) = CONTROLLER;
    let mut opaque = 0u32;
    let mut dark = 0u32;
    let mut total = 0u32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let c = atlas.get_pixel(x, y);
            total += 1;
            if c[3] > 128 {
                opaque += 1;
                if (c[0] as u32 + c[1] as u32 + c[2] as u32) < 300 {
                    dark += 1;
                }
            }
        }
    }
    opaque * 10 > total * 8 && dark * 10 > opaque * 6
}

/// Patch every dump texture that carries the shared PS2 prompts: the face
/// slots, the shoulder boxes and — where the sheet shows the pad photo — the
/// controller drawing. Returns how many sheets were written.
pub fn patch(dump_dir: &Path, out_dir: &Path) -> anyhow::Result<usize> {
    std::fs::create_dir_all(out_dir)?;
    let mut written = 0;
    for entry in std::fs::read_dir(dump_dir)?.flatten() {
        let src = entry.path();
        if !src
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("png"))
        {
            continue;
        }
        let dst = out_dir.join(src.file_name().unwrap());
        if dst.is_file() {
            continue;
        }
        let Ok(atlas) = image::open(&src) else {
            continue;
        };
        let atlas = atlas.to_rgba8();
        if atlas.dimensions() != (256, 256) || !has_the_ps2_glyphs(&atlas) {
            continue;
        }
        let patched_atlas = patch_atlas(&atlas)?;
        patched_atlas.save_with_format(&dst, image::ImageFormat::Png)?;
        written += 1;
    }
    // Mark the dump's size at patch time: the pack regenerates when the
    // dump has grown since. The marker lives in the pack folder, not among
    // the texture files: the pack listing expects only engine-hash names.
    let count = std::fs::read_dir(dump_dir)?.count();
    let marker = out_dir.parent().unwrap_or(out_dir).join(".patched");
    let _ = std::fs::write(marker, count.to_string());
    Ok(written)
}

fn patch_atlas(atlas: &image::RgbaImage) -> anyhow::Result<image::RgbaImage> {
    let mut out = atlas.clone();
    for (x0, y0, x1, y1, letter) in SLOTS {
        let w = x1 - x0 + 1;
        let h = y1 - y0 + 1;
        // clear the slot, then composite our glyph on top
        for y in y0..=y1 {
            for x in x0..=x1 {
                out.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
            }
        }
        let glyph = image::imageops::resize(&glyph_image(letter)?, w, h, FilterType::Lanczos3);
        image::imageops::overlay(&mut out, &glyph, x0 as i64, y0 as i64);
    }
    // The shoulder boxes: same treatment, Xbox box art instead of glyphs.
    for ((x0, y0, x1, y1), _, art) in SHOULDERS {
        let w = (x1 - x0 + 1) as usize;
        let h = (y1 - y0 + 1) as usize;
        for y in y0..=y1 {
            for x in x0..=x1 {
                out.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
            }
        }
        let box_img = image::load_from_memory(art)?.to_rgba8();
        let box_img = image::imageops::resize(&box_img, w as u32, h as u32, FilterType::Lanczos3);
        image::imageops::overlay(&mut out, &box_img, x0 as i64, y0 as i64);
    }
    // The controller drawing: the Xbox pad, contained and centred in the
    // photo's rect.
    {
        let (x0, y0, x1, y1) = CONTROLLER;
        let (w, h) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                out.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
            }
        }
        let pad = image::load_from_memory(CONTROLLER_PNG)?.to_rgba8();
        let scale = (w / pad.width() as f32).min(h / pad.height() as f32);
        let nw = (pad.width() as f32 * scale).round().max(1.0) as u32;
        let nh = (pad.height() as f32 * scale).round().max(1.0) as u32;
        let pad = image::imageops::resize(&pad, nw, nh, FilterType::Lanczos3);
        let ox = (x0 as f32 + (w - nw as f32) / 2.0).round() as i64;
        let oy = (y0 as f32 + (h - nh as f32) / 2.0).round() as i64;
        image::imageops::overlay(&mut out, &pad, ox, oy);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_a_synthetic_atlas() {
        // 256x256 opaque atlas with the four slots at known positions.
        let mut atlas = image::RgbaImage::new(256, 256);
        for (x0, y0, x1, y1, _) in SLOTS {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    atlas.put_pixel(x, y, image::Rgba([40, 90, 140, 255]));
                }
            }
        }
        let patched = patch_atlas(&atlas).unwrap();
        for (x0, y0, x1, y1, letter) in SLOTS {
            // the ring-style art landed: most of the slot is opaque, and
            // none of it is the old blue.
            let mut opaque = 0;
            let mut total = 0;
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let c = patched.get_pixel(x, y);
                    total += 1;
                    if c[3] > 0 {
                        opaque += 1;
                    }
                    assert_ne!(c, &image::Rgba([40, 90, 140, 255]), "slot not patched");
                }
            }
            assert!(
                opaque > total / 3,
                "letter {letter}: only {opaque}/{total} pixels are painted"
            );
        }
    }
}
