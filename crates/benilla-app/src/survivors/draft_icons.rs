//! The card screen's button pictograms, drawn here: the client data has no die or no-entry icon.
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

const SIDE: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pictogram {
    /// Auto: a gold spark for the selected build's suggested card.
    Auto,
    /// Reroll: a die showing five.
    Die,
    /// Skip: an arrow pointing on.
    Arrow,
    /// Banish: a no-entry sign.
    NoEntry,
}

/// Coverage of a shape at each pixel, sampled 4×4 so the edges are smooth.
fn paint(pixels: &mut [u8], color: [u8; 3], inside: impl Fn(f32, f32) -> bool) {
    for y in 0..SIDE {
        for x in 0..SIDE {
            let mut hits = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = (x as f32 + (sx as f32 + 0.5) / 4.0) / SIDE as f32;
                    let py = (y as f32 + (sy as f32 + 0.5) / 4.0) / SIDE as f32;
                    hits += u32::from(inside(px, py));
                }
            }
            if hits == 0 {
                continue;
            }
            let a = hits as f32 / 16.0;
            let at = (y * SIDE + x) * 4;
            let under = pixels[at + 3] as f32 / 255.0;
            for c in 0..3 {
                let old = pixels[at + c] as f32;
                pixels[at + c] = (color[c] as f32 * a + old * (1.0 - a)).round() as u8;
            }
            pixels[at + 3] = ((a + under * (1.0 - a)) * 255.0).round() as u8;
        }
    }
}

fn rounded_square(x: f32, y: f32, half: f32, radius: f32) -> bool {
    let dx = ((x - 0.5).abs() - (half - radius)).max(0.0);
    let dy = ((y - 0.5).abs() - (half - radius)).max(0.0);
    dx * dx + dy * dy <= radius * radius
}

/// RGBA8 pixels, top row first.
pub(super) fn pixels(icon: Pictogram) -> Vec<u8> {
    let mut out = vec![0; SIDE * SIDE * 4];
    let circle = |cx: f32, cy: f32, r: f32| move |x: f32, y: f32| (x - cx).powi(2) + (y - cy).powi(2) <= r * r;
    match icon {
        Pictogram::Auto => {
            paint(&mut out, [40, 30, 20], |x, y| rounded_square(x, y, 0.44, 0.12));
            paint(&mut out, [255, 205, 60], |x, y| {
                let dx = (x - 0.5).abs();
                let dy = (y - 0.5).abs();
                dx + dy <= 0.34 || (dx <= 0.07 && dy <= 0.42) || (dy <= 0.07 && dx <= 0.42)
            });
        }
        Pictogram::Die => {
            paint(&mut out, [40, 30, 20], |x, y| rounded_square(x, y, 0.44, 0.12));
            paint(&mut out, [245, 238, 220], |x, y| rounded_square(x, y, 0.39, 0.09));
            for (cx, cy) in [(0.3, 0.3), (0.7, 0.3), (0.5, 0.5), (0.3, 0.7), (0.7, 0.7)] {
                paint(&mut out, [30, 25, 20], circle(cx, cy, 0.075));
            }
        }
        Pictogram::Arrow => {
            let dark = |x: f32, y: f32, grow: f32| {
                let shaft = (0.12 - grow..=0.56).contains(&x) && (y - 0.5).abs() <= 0.11 + grow;
                // The head: a triangle from x = 0.5 to its tip at 0.9.
                let head = x >= 0.48 - grow && x <= 0.9 + grow && (y - 0.5).abs() <= (0.9 + grow - x) * 0.85;
                shaft || head
            };
            paint(&mut out, [40, 30, 10], |x, y| dark(x, y, 0.045));
            paint(&mut out, [255, 205, 60], |x, y| dark(x, y, 0.0));
        }
        Pictogram::NoEntry => {
            paint(&mut out, [255, 255, 255], circle(0.5, 0.5, 0.46));
            paint(&mut out, [210, 25, 25], circle(0.5, 0.5, 0.42));
            paint(&mut out, [255, 255, 255], |x, y| {
                (x - 0.5).abs() <= 0.29 && (y - 0.5).abs() <= 0.08
            });
        }
    }
    out
}

pub(super) fn image(icon: Pictogram, images: &mut Assets<Image>) -> Handle<Image> {
    images.add(Image::new(
        Extent3d {
            width: SIDE as u32,
            height: SIDE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels(icon),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(p: &[u8], x: usize, y: usize) -> u8 {
        p[(y * SIDE + x) * 4 + 3]
    }

    #[test]
    fn each_pictogram_is_drawn_on_a_clear_background() {
        for icon in [Pictogram::Auto, Pictogram::Die, Pictogram::Arrow, Pictogram::NoEntry] {
            let p = pixels(icon);
            assert_eq!(alpha(&p, 0, 0), 0, "{icon:?} corner");
            assert!(p.chunks(4).filter(|c| c[3] == 255).count() > SIDE * SIDE / 8, "{icon:?}");
        }
        // The no-entry bar is white across a red disc.
        let p = pixels(Pictogram::NoEntry);
        let at = |x: usize, y: usize| &p[(y * SIDE + x) * 4..(y * SIDE + x) * 4 + 3];
        assert_eq!(at(32, 32), [255, 255, 255]);
        assert_eq!(at(32, 12), [210, 25, 25]);
    }
}
