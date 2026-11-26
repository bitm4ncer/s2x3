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

/// The controls sheet is shipped under two texture ids: the front end uses the
/// first, the in-game pause/HUD path the second.
pub const ATLASES: [&str; 2] = ["f897162c0f661c8b.png", "75df99fccd7d9d0b.png"];

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

/// Atlases that are in the texreplace folder already.
pub fn patched(out_dir: &Path) -> usize {
    ATLASES
        .iter()
        .filter(|name| out_dir.join(name).is_file())
        .count()
}

/// Atlases available in the dump (the raw input for the patch).
pub fn dumped(dump_dir: &Path) -> Vec<&'static str> {
    ATLASES
        .iter()
        .filter(|name| dump_dir.join(name).is_file())
        .copied()
        .collect()
}

/// True when nothing is left to do: every known atlas is patched.
pub fn complete(out_dir: &Path) -> bool {
    patched(out_dir) == ATLASES.len()
}

/// Patch every atlas that exists in the dump and is not patched yet.
/// Returns how many atlases were written.
pub fn patch(dump_dir: &Path, out_dir: &Path) -> anyhow::Result<usize> {
    std::fs::create_dir_all(out_dir)?;
    let mut written = 0;
    for name in ATLASES {
        let src = dump_dir.join(name);
        let dst = out_dir.join(name);
        if dst.is_file() || !src.is_file() {
            continue;
        }
        let atlas = image::open(&src)?.to_rgba8();
        let patched_atlas = patch_atlas(&atlas)?;
        patched_atlas.save_with_format(&dst, image::ImageFormat::Png)?;
        written += 1;
    }
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
            // the glyph art is a coloured disc with a white letter: most of
            // the slot must carry the disc colour, none of it the old blue.
            let expected = match letter {
                'a' => [63, 191, 95],
                'b' => [229, 72, 77],
                'x' => [59, 130, 246],
                'y' => [242, 199, 68],
                _ => unreachable!(),
            };
            let mut disc = 0;
            let mut total = 0;
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let c = patched.get_pixel(x, y);
                    total += 1;
                    if (c[0] as i32 - expected[0] as i32).abs() < 50
                        && (c[1] as i32 - expected[1] as i32).abs() < 50
                        && (c[2] as i32 - expected[2] as i32).abs() < 50
                    {
                        disc += 1;
                    }
                    assert_ne!(c, &image::Rgba([40, 90, 140, 255]), "slot not patched");
                }
            }
            assert!(
                disc > total / 3,
                "letter {letter}: disc colour covers only {disc}/{total} pixels"
            );
        }
    }
}
