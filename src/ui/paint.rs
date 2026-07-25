//! Where the symbol art goes when it is drawn (§5.25).
//!
//! # Why the art needed a second destination
//!
//! Every check on the symbol art so far has been a person looking at a capture.
//! That caught real defects — gems reading as kites, a coin stack as a blob, an
//! egg as a teardrop, three stones that were one stone to a deuteranope — but it
//! needs a person, it needs a GPU, and it only ever examines the frames someone
//! thought to photograph.
//!
//! The art is drawn through a handful of primitives. Putting a trait in front of
//! them costs nothing at the call sites and buys a second implementation that
//! writes into a plain pixel buffer, with no window, no GL context and no
//! frame. Which means the art can be **rasterised inside a unit test** and
//! measured: is a symbol actually there, does it fill its cell, and — the
//! question §5.24 left open — can two of them still be told apart at the size a
//! six-row shifting reel draws them?
//!
//! `ScreenPainter` is what ships. `Buffer` is what the tests look at. Both draw
//! the same art from the same routines, which is the only reason a measurement
//! taken here says anything about what a player sees.

use macroquad::prelude::*;

/// The primitives the symbol art is built from.
///
/// Coordinates are already in pixels — the normalising is [`Canvas`]'s job, so
/// an implementation only has to know how to fill a shape.
pub trait Painter {
    fn tri(&mut self, a: Vec2, b: Vec2, c: Vec2, color: Color);
    fn circle(&mut self, center: Vec2, radius: f32, color: Color);
    fn ellipse(&mut self, center: Vec2, rx: f32, ry: f32, color: Color);
    fn rect(&mut self, at: Vec2, size: Vec2, color: Color);
}

/// Draws to the screen. What the game uses.
pub struct ScreenPainter;

impl Painter for ScreenPainter {
    fn tri(&mut self, a: Vec2, b: Vec2, c: Vec2, color: Color) {
        draw_triangle(a, b, c, color);
    }

    fn circle(&mut self, center: Vec2, radius: f32, color: Color) {
        draw_circle(center.x, center.y, radius, color);
    }

    fn ellipse(&mut self, center: Vec2, rx: f32, ry: f32, color: Color) {
        draw_ellipse(center.x, center.y, rx, ry, 0.0, color);
    }

    fn rect(&mut self, at: Vec2, size: Vec2, color: Color) {
        draw_rectangle(at.x, at.y, size.x, size.y, color);
    }
}

/// Draws into memory. What the tests read.
///
/// Test-only: nothing in the running game rasterises to a buffer. It is not
/// dead weight though — it is the only thing that can see the art without a GPU,
/// and every claim `symbols/legible.rs` makes rests on it.
///
/// Deliberately simple: no antialiasing, no sub-pixel coverage. A test asking
/// "are these two symbols different shapes" does not need either, and adding
/// them would make the buffer disagree with the GPU in ways that are hard to
/// reason about. What it does need is alpha blending, because half the art is
/// translucent highlights over solid facets.
#[cfg(test)]
#[derive(Debug, Clone)]
pub struct Buffer {
    width: usize,
    height: usize,
    /// Straight RGB, one triple per pixel. Starts black.
    pixels: Vec<[f32; 3]>,
}

#[cfg(test)]
impl Buffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![[0.0; 3]; width * height],
        }
    }

    pub fn at(&self, x: usize, y: usize) -> [f32; 3] {
        self.pixels[y * self.width + x]
    }

    /// The rectangle the art should be drawn into to fill this buffer.
    ///
    /// Named `bounds` rather than `rect` because `Painter::rect` is a drawing
    /// call and having both on one type reads as a mistake even when it compiles.
    pub fn bounds(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width as f32, self.height as f32)
    }

    /// Share of pixels with anything drawn on them.
    ///
    /// A symbol that covers almost nothing is invisible at a glance; one that
    /// covers almost everything is a coloured square rather than a shape.
    pub fn coverage(&self) -> f32 {
        let lit = self
            .pixels
            .iter()
            .filter(|pixel| pixel.iter().any(|channel| *channel > 0.02))
            .count();
        lit as f32 / self.pixels.len().max(1) as f32
    }

    /// Which pixels have anything on them at all.
    ///
    /// The silhouette is the part of a symbol that survives colour blindness,
    /// bad contrast and a very small cell. Two symbols with the same silhouette
    /// are the same symbol to anyone not looking closely.
    pub fn silhouette(&self) -> Vec<bool> {
        self.pixels
            .iter()
            .map(|pixel| pixel.iter().any(|channel| *channel > 0.02))
            .collect()
    }

    /// How unalike two silhouettes are: 0 for identical, 1 for no overlap at
    /// all. The Jaccard distance — the share of the *union* that only one of
    /// them covers.
    ///
    /// Not the share of the whole cell that differs, which was the first
    /// attempt and was useless: both symbols leave most of a cell empty, so two
    /// quite different shapes agreed on 93% of the pixels simply by both being
    /// small. Normalising by the area the symbols actually occupy is what makes
    /// the number mean something.
    pub fn silhouette_difference(&self, other: &Buffer) -> f32 {
        let mine = self.silhouette();
        let theirs = other.silhouette();

        let mut intersection = 0usize;
        let mut union = 0usize;
        for (a, b) in mine.iter().zip(theirs.iter()) {
            if *a && *b {
                intersection += 1;
            }
            if *a || *b {
                union += 1;
            }
        }
        if union == 0 {
            return 0.0;
        }
        1.0 - intersection as f32 / union as f32
    }

    /// How different two symbols look in **monochrome**, 0 to 1.
    ///
    /// Mean absolute difference in luminance over every pixel. This is the test
    /// that matters: it catches a shared outline *and* a shared interior at
    /// once, and monochrome is the strictest realistic case — it is what a
    /// symbol has left after colour blindness, a washed-out screen and a cell a
    /// sixth of the reel window tall.
    ///
    /// Silhouette alone proved too blunt a standard for this art. Every symbol
    /// is a centred object filling most of its cell, so a coin and a chest
    /// overlap heavily in outline and always will; what separates them is the
    /// lid, the keyhole and the shading, none of which an outline can see.
    pub fn monochrome_difference(&self, other: &Buffer) -> f32 {
        let luma = |pixel: [f32; 3]| 0.2126 * pixel[0] + 0.7152 * pixel[1] + 0.0722 * pixel[2];
        let total: f32 = self
            .pixels
            .iter()
            .zip(other.pixels.iter())
            .map(|(a, b)| (luma(*a) - luma(*b)).abs())
            .sum();
        total / self.pixels.len().max(1) as f32
    }

    fn blend(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let index = y as usize * self.width + x as usize;
        let alpha = color.a.clamp(0.0, 1.0);
        let pixel = &mut self.pixels[index];
        pixel[0] += (color.r - pixel[0]) * alpha;
        pixel[1] += (color.g - pixel[1]) * alpha;
        pixel[2] += (color.b - pixel[2]) * alpha;
    }

    /// Fill every pixel whose centre satisfies `inside`, over the given bounds.
    fn fill<F: Fn(f32, f32) -> bool>(
        &mut self,
        bounds: (f32, f32, f32, f32),
        color: Color,
        inside: F,
    ) {
        let (min_x, min_y, max_x, max_y) = bounds;
        let x0 = min_x.floor().max(0.0) as i32;
        let y0 = min_y.floor().max(0.0) as i32;
        let x1 = (max_x.ceil() as i32).min(self.width as i32);
        let y1 = (max_y.ceil() as i32).min(self.height as i32);

        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                if inside(px, py) {
                    self.blend(x, y, color);
                }
            }
        }
    }
}

#[cfg(test)]
impl Painter for Buffer {
    fn tri(&mut self, a: Vec2, b: Vec2, c: Vec2, color: Color) {
        // Edge functions. Sign-agnostic so winding order does not matter — the
        // art is not consistent about it and has no reason to be.
        let edge =
            |p: Vec2, q: Vec2, x: f32, y: f32| (q.x - p.x) * (y - p.y) - (q.y - p.y) * (x - p.x);
        let bounds = (
            a.x.min(b.x).min(c.x),
            a.y.min(b.y).min(c.y),
            a.x.max(b.x).max(c.x),
            a.y.max(b.y).max(c.y),
        );
        self.fill(bounds, color, |x, y| {
            let (e0, e1, e2) = (edge(a, b, x, y), edge(b, c, x, y), edge(c, a, x, y));
            (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0)
        });
    }

    fn circle(&mut self, center: Vec2, radius: f32, color: Color) {
        self.ellipse(center, radius, radius, color);
    }

    fn ellipse(&mut self, center: Vec2, rx: f32, ry: f32, color: Color) {
        let (rx, ry) = (rx.max(0.001), ry.max(0.001));
        let bounds = (center.x - rx, center.y - ry, center.x + rx, center.y + ry);
        self.fill(bounds, color, |x, y| {
            let dx = (x - center.x) / rx;
            let dy = (y - center.y) / ry;
            dx * dx + dy * dy <= 1.0
        });
    }

    fn rect(&mut self, at: Vec2, size: Vec2, color: Color) {
        let bounds = (at.x, at.y, at.x + size.x, at.y + size.y);
        self.fill(bounds, color, |x, y| {
            x >= at.x && x < at.x + size.x && y >= at.y && y < at.y + size.y
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filled_rectangle_covers_exactly_its_area() {
        let mut buffer = Buffer::new(40, 40);
        buffer.rect(vec2(10.0, 10.0), vec2(20.0, 20.0), WHITE);
        // 400 of 1600 pixels.
        assert!(
            (buffer.coverage() - 0.25).abs() < 0.02,
            "{}",
            buffer.coverage()
        );
    }

    #[test]
    fn a_triangle_fills_about_half_its_bounding_box() {
        let mut buffer = Buffer::new(40, 40);
        buffer.tri(vec2(0.0, 0.0), vec2(40.0, 0.0), vec2(0.0, 40.0), WHITE);
        assert!(
            (buffer.coverage() - 0.5).abs() < 0.05,
            "{}",
            buffer.coverage()
        );
    }

    #[test]
    fn winding_order_does_not_matter() {
        // The art is not consistent about it, and a rasteriser that cared would
        // silently drop half the facets.
        let mut clockwise = Buffer::new(32, 32);
        let mut widdershins = Buffer::new(32, 32);
        clockwise.tri(vec2(2.0, 2.0), vec2(30.0, 2.0), vec2(16.0, 30.0), WHITE);
        widdershins.tri(vec2(16.0, 30.0), vec2(30.0, 2.0), vec2(2.0, 2.0), WHITE);
        assert_eq!(clockwise.silhouette_difference(&widdershins), 0.0);
    }

    #[test]
    fn a_circle_fills_pi_over_four_of_its_box() {
        let mut buffer = Buffer::new(64, 64);
        buffer.circle(vec2(32.0, 32.0), 32.0, WHITE);
        let expected = std::f32::consts::FRAC_PI_4;
        assert!(
            (buffer.coverage() - expected).abs() < 0.02,
            "{} against {}",
            buffer.coverage(),
            expected
        );
    }

    #[test]
    fn alpha_blends_rather_than_replacing() {
        let mut buffer = Buffer::new(8, 8);
        buffer.rect(vec2(0.0, 0.0), vec2(8.0, 8.0), BLACK);
        buffer.rect(
            vec2(0.0, 0.0),
            vec2(8.0, 8.0),
            Color::new(1.0, 1.0, 1.0, 0.5),
        );
        let pixel = buffer.at(4, 4);
        assert!((pixel[0] - 0.5).abs() < 0.01, "{:?}", pixel);
    }

    #[test]
    fn drawing_outside_the_buffer_is_ignored_rather_than_panicking() {
        let mut buffer = Buffer::new(16, 16);
        buffer.rect(vec2(-100.0, -100.0), vec2(50.0, 50.0), WHITE);
        buffer.circle(vec2(500.0, 500.0), 20.0, WHITE);
        assert_eq!(buffer.coverage(), 0.0);
    }
}
