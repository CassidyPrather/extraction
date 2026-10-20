//! Foliage card outlines. An alpha-tested card shades every fragment of
//! its quad, the empty corners too (a discarded fragment has been paid
//! for). Drawn as the 8-sided polygon round its opaque texels instead,
//! the same picture costs a quarter fewer fragments; in a wood the cards
//! are a third of the layers a pixel is shaded for (`render::overdraw`).
//!
//! The outlines are rect-local (s across `u0..u1`, t down `v0..v1`),
//! made by `scripts/card_outlines.py` from the atlases; the test below
//! holds them to the atlases, so a re-cut atlas can't lose a leaf.

/// spruce_branch [0.3, 0.385, 0.656, 0.768]: 80% of the rect.
pub const SPRIG_0: &[[f32; 2]] = &[
    [0.384, 0.000],
    [0.741, 0.000],
    [1.000, 0.259],
    [1.000, 0.852],
    [0.852, 1.000],
    [0.411, 1.000],
    [0.000, 0.589],
    [0.000, 0.384],
];
/// spruce_branch [0.66, 0.446, 0.935, 0.765]: 90% of the rect.
pub const SPRIG_1: &[[f32; 2]] = &[
    [0.298, 0.000],
    [0.684, 0.000],
    [1.000, 0.316],
    [1.000, 1.000],
    [0.139, 1.000],
    [0.000, 0.861],
    [0.000, 0.298],
];
/// spruce_branch [0.183, 0.041, 0.444, 0.326]: 77% of the rect.
pub const SPRIG_2: &[[f32; 2]] = &[
    [0.354, 0.000],
    [0.678, 0.000],
    [1.000, 0.322],
    [1.000, 1.000],
    [0.474, 1.000],
    [0.000, 0.526],
    [0.000, 0.354],
];
/// spruce_branch [0.646, 0.034, 0.944, 0.375]: 64% of the rect.
pub const SPRIG_3: &[[f32; 2]] = &[
    [0.398, 0.000],
    [0.672, 0.000],
    [1.000, 0.328],
    [1.000, 0.570],
    [0.570, 1.000],
    [0.518, 1.000],
    [0.000, 0.482],
    [0.000, 0.398],
];
/// birch_leaves [0.21, 0.0, 0.81, 1.0]: 71% of the rect.
pub const BIRCH: &[[f32; 2]] = &[
    [0.459, 0.000],
    [0.509, 0.000],
    [0.989, 0.480],
    [0.989, 0.687],
    [0.677, 1.000],
    [0.143, 1.000],
    [0.000, 0.857],
    [0.000, 0.459],
];

/// The full quad, for a rect with no outline.
pub const QUAD: &[[f32; 2]] = &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

/// Each outlined rect: its atlas, the rect, the outline.
pub const OUTLINES: [(&str, [f32; 4], &[[f32; 2]]); 5] = [
    ("spruce_branch", [0.300, 0.385, 0.656, 0.768], SPRIG_0),
    ("spruce_branch", [0.660, 0.446, 0.935, 0.765], SPRIG_1),
    ("spruce_branch", [0.183, 0.041, 0.444, 0.326], SPRIG_2),
    ("spruce_branch", [0.646, 0.034, 0.944, 0.375], SPRIG_3),
    ("birch_leaves", [0.21, 0.0, 0.81, 1.0], BIRCH),
];

/// The outline for an atlas rect (the whole quad if it has none).
pub fn outline(rect: [f32; 4]) -> &'static [[f32; 2]] {
    OUTLINES
        .iter()
        .find(|(_, r, _)| *r == rect)
        .map_or(QUAD, |(_, _, o)| *o)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `p` is inside the convex outline (either winding).
    fn inside(outline: &[[f32; 2]], p: [f32; 2]) -> bool {
        let mut sign = 0.0f32;
        for i in 0..outline.len() {
            let a = outline[i];
            let b = outline[(i + 1) % outline.len()];
            let c = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
            if c.abs() < 1e-6 {
                continue;
            }
            if sign == 0.0 {
                sign = c.signum();
            } else if c.signum() != sign {
                return false;
            }
        }
        true
    }

    /// Every texel with any alpha in each rect lies inside its outline
    /// (skipped where `fetch_assets.py` hasn't built the atlas).
    #[test]
    fn outlines_hold_every_leaf() {
        for (atlas, rect, outline) in OUTLINES {
            let path =
                crate::asset_root_path().join(format!("textures/foliage/{atlas}/albedo.png"));
            let Ok(bytes) = std::fs::read(&path) else {
                eprintln!("no atlas at {}: skipped", path.display());
                continue;
            };
            let img = bevy::image::Image::from_buffer(
                &bytes,
                bevy::image::ImageType::Extension("png"),
                bevy::image::CompressedImageFormats::NONE,
                false,
                bevy::image::ImageSampler::Default,
                bevy::asset::RenderAssetUsages::default(),
            )
            .unwrap();
            let (w, h) = (img.width() as f32, img.height() as f32);
            let data = img.data.as_ref().unwrap();
            let (x0, y0) = ((rect[0] * w) as u32, (rect[1] * h) as u32);
            let (x1, y1) = ((rect[2] * w).round() as u32, (rect[3] * h).round() as u32);
            let (rw, rh) = ((x1 - x0) as f32, (y1 - y0) as f32);
            let mut lost = 0;
            for y in y0..y1 {
                for x in x0..x1 {
                    let a = data[((y * img.width() + x) * 4 + 3) as usize];
                    if a == 0 {
                        continue;
                    }
                    // The texel's centre.
                    let p = [
                        (x - x0) as f32 / rw + 0.5 / rw,
                        (y - y0) as f32 / rh + 0.5 / rh,
                    ];
                    if !inside(outline, p) {
                        lost += 1;
                    }
                }
            }
            assert_eq!(
                lost, 0,
                "{atlas} {rect:?}: {lost} texels outside the outline (re-run scripts/card_outlines.py)"
            );
        }
    }

    /// The trees' atlas rects are the ones outlined (a moved rect would
    /// silently fall back to the full quad).
    #[test]
    fn tree_rects_are_outlined() {
        for r in crate::world::trees::CARD_RECTS {
            assert!(
                !std::ptr::eq(outline(r), QUAD),
                "no outline for card rect {r:?}: add it to OUTLINES and run scripts/card_outlines.py"
            );
        }
    }
}
