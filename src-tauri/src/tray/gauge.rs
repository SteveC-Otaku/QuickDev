use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::image::Image;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Visual {
    Idle,
    Running,
    Warning,
    Starting,
    Stopping,
}
pub(super) const SIZES: [u32; 6] = [16, 20, 24, 32, 40, 48];
type Frames = HashMap<(u32, Visual), Vec<Vec<u8>>>;
static CACHE: OnceLock<Mutex<Frames>> = OnceLock::new();

pub(super) fn icon(size: u32, visual: Visual, frame: usize) -> Image<'static> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (size, visual);
    let mut guard = cache.lock().unwrap();
    if !guard.contains_key(&key) {
        drop(guard);
        let count = if matches!(visual, Visual::Idle | Visual::Warning) {
            1
        } else {
            32
        };
        let frames = (0..count)
            .map(|frame| render(size, visual, frame))
            .collect();
        guard = cache.lock().unwrap();
        guard.entry(key).or_insert(frames);
    }
    let frames = &guard[&key];
    Image::new_owned(frames[frame % frames.len()].clone(), size, size)
}

fn render(size: u32, visual: Visual, frame: usize) -> Vec<u8> {
    // Coverage sampling, with no second blur filter or black transparent pixels.
    let n = size * 4;
    let scale = n as f32 / 48.0;
    let mut pixels = vec![0u8; (n * n * 4) as usize];
    let accent = match visual {
        Visual::Warning => [205, 141, 12, 255],
        Visual::Idle => [123, 139, 158, 255],
        _ => [20, 181, 124, 255],
    };
    let phase = frame as f32 / 32.0 * std::f32::consts::TAU;
    let angle = match visual {
        Visual::Idle => 150.0,
        Visual::Running => 42.0 + 18.0 * phase.sin(),
        Visual::Warning => 90.0,
        Visual::Starting => 95.0 + 55.0 * phase.cos(),
        Visual::Stopping => 40.0 + 110.0 * (frame as f32 / 16.0).min(1.0),
    };
    let a = angle.to_radians();
    // Shorten the needle to leave a visible gap from the arc at 16px.
    let tip = (24.0 + 10.5 * a.cos(), 30.0 - 10.5 * a.sin());
    for y in 0..n {
        for x in 0..n {
            // Use more of the fixed Windows tray slot, including at 16px.
            // Scale geometry directly to retain coverage sharpness; warning stays inside.
            let px = ((x as f32 + 0.5) / scale - 24.0) / 1.12 + 24.0;
            let py = ((y as f32 + 0.5) / scale - 27.0) / 1.12 + 27.0;
            let radial = ((px - 24.0).powi(2) + (py - 30.0).powi(2)).sqrt();
            // Scheme A: an open arc, with transparent interior and no dark plate.
            let outer = if py <= 30.0 {
                (radial - 18.0).abs()
            } else {
                ((px - 6.0).powi(2) + (py - 30.0).powi(2)).sqrt()
                    .min(((px - 42.0).powi(2) + (py - 30.0).powi(2)).sqrt())
            };
            let mut c = [0, 0, 0, 0];
            if outer < 2.0 {
                c = accent;
            }
            // A single top tick only at sizes where it has enough breathing room.
            if size >= 24 && segment(px,py,24.0,15.5,24.0,18.0) < 0.7 {
                c = accent;
            }
            let needle = segment(px, py, 24.0, 30.0, tip.0, tip.1);
            if needle < 1.7 {
                c = accent;
            }
            if ((px - 24.0).powi(2) + (py - 30.0).powi(2)).sqrt() < 2.0 {
                c = accent;
            }
            if visual == Visual::Warning {
                if segment(px, py, 38.0, 31.0, 38.0, 38.0) < 2.0
                    || ((px - 38.0).powi(2) + (py - 43.0).powi(2)).sqrt() < 2.0
                {
                    c = accent;
                }
            }
            let i = ((y * n + x) * 4) as usize;
            pixels[i..i + 4].copy_from_slice(&c);
        }
    }
    let mut output = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let mut alpha = 0u32;
            for sy in 0..4 { for sx in 0..4 {
                alpha += pixels[(((y*4+sy)*n+x*4+sx)*4+3) as usize] as u32;
            }}
            let index = ((y*size+x)*4) as usize;
            output[index..index+3].copy_from_slice(&accent[..3]);
            output[index+3] = ((alpha+8)/16) as u8;
        }
    }
    output
}
fn segment(x: f32, y: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let t = (((x - ax) * (bx - ax) + (y - ay) * (by - ay))
        / ((bx - ax).powi(2) + (by - ay).powi(2)))
    .clamp(0.0, 1.0);
    ((x - ax - t * (bx - ax)).powi(2) + (y - ay - t * (by - ay)).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_preview_when_requested() {
        let Ok(path) = std::env::var("QD_TRAY_PREVIEW") else {
            return;
        };
        let mut sheet = image::RgbaImage::new(1080, 160);
        for (row, bg) in [[245, 247, 250, 255], [24, 29, 38, 255]]
            .into_iter()
            .enumerate()
        {
            for y in row * 80..(row + 1) * 80 {
                for x in 0..1080 {
                    sheet.put_pixel(x, y as u32, image::Rgba(bg));
                }
            }
            for (col, size) in SIZES.into_iter().enumerate() {
                for (state, visual) in [Visual::Idle, Visual::Running, Visual::Warning]
                    .into_iter()
                    .enumerate()
                {
                    let glyph =
                        image::RgbaImage::from_raw(size, size, render(size, visual, 0)).unwrap();
                    image::imageops::overlay(
                        &mut sheet,
                        &glyph,
                        (col * 180 + state * 54) as i64,
                        (row * 80 + 8) as i64,
                    );
                }
            }
        }
        sheet.save(path).unwrap();
    }
    #[test]
    fn open_arc_states_are_distinct_transparent_and_animation_is_bounded() {
        for size in SIZES {
            let idle = render(size, Visual::Idle, 0);
            let running = render(size, Visual::Running, 0);
            let warning = render(size, Visual::Warning, 0);
            assert_eq!(idle.len(), (size * size * 4) as usize);
            assert_ne!(idle, running);
            assert_ne!(running, warning);
            assert!(
                idle.chunks_exact(4).filter(|p| p[3] == 0).count() > (size * size / 3) as usize
            );
            // The dial interior is transparent, away from ticks and the idle needle.
            assert_eq!(idle[((size / 2 * size + size * 6 / 10) * 4 + 3) as usize], 0);
            assert!(!idle.chunks_exact(4).any(|p| p == [25, 34, 46, 255]));
            assert_eq!(
                render(size, Visual::Idle, 0),
                render(size, Visual::Idle, 10)
            );
            assert_eq!(
                render(size, Visual::Warning, 0),
                render(size, Visual::Warning, 10)
            );
            assert_ne!(
                render(size, Visual::Running, 0),
                render(size, Visual::Running, 8)
            );
        }
    }
}
