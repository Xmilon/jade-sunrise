//! The tooltip's model, its layout and its drawing.
//!
//! A tooltip is an optional icon and lines of elements. Layout runs in "content pixels" (the
//! tooltip at scale 1); drawing turns them into GUI pixels with the overlay's position and scale.
//! The box is drawn the way Jade's theme sprites draw it (a flat body, a 1-pixel apron around it
//! with cut corners, and a 1-pixel edge line on each side of the box), in the theme's colours
//! sampled from Jade's own sprites but drawn here from fills — no Jade art is copied. Hearts,
//! armour and the furnace arrow are the game's own HUD and container sprites, drawn from fills
//! where the sprite would carry a background.

use mod_api::*;
use mod_api::client::hud_layout::{Blend, PoseOp};
use mod_api::text::TextStyle;

/// Semantic colours (Jade's palette).
pub const NORMAL: u32 = 0xFFA0A0A0;
pub const INFO: u32 = 0xFFFFFFFF;
pub const TITLE: u32 = 0xFFFFFFFF;
pub const SUCCESS: u32 = 0xFF55FF55;
pub const WARNING: u32 = 0xFFFFC107;
pub const DANGER: u32 = 0xFFFF5555;
pub const FAILURE: u32 = 0xFFAA0000;
/// Chat GRAY, the registry id line's colour in Jade.
pub const GREY: u32 = 0xFFAAAAAA;
pub const MOD_BLUE: u32 = 0xFF5555FF;

#[derive(Clone, Debug)]
pub enum El {
    /// Plain text in one colour.
    Text(String, u32),
    /// Italic text (the mod name), at a scale (1 or 0.75).
    Italic(String, u32, f32),
    /// A stack's graphic `size` pixels square (Jade's element is 2 pixels bigger), with an
    /// optional count text in the corner.
    Item(ItemStack, f32, Option<String>),
    /// A HUD sprite (`hud/heart/full`), `w` × `h`.
    Sprite(&'static str, i32, i32),
    /// The furnace's arrow, filled to the ratio.
    Arrow(f32),
    /// A small check (true) or cross (false).
    Check(bool),
    /// Empty space.
    Space(f32),
}

#[derive(Clone, Debug, Default)]
pub struct Line {
    pub els: Vec<El>,
    /// Inside a nested box (mob effects).
    pub boxed: bool,
    /// Drawn only with the details key held, or in non-lite modes (Jade's "lite" filter).
    pub detail: bool,
}

impl Line {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn text(s: impl Into<String>, color: u32) -> Self {
        Self { els: vec![El::Text(s.into(), color)], ..Self::default() }
    }
    /// `label` in the normal grey, then `value` in its own colour: `Growth: 47%`.
    pub fn pair(label: &str, value: impl Into<String>, color: u32) -> Self {
        Self { els: vec![El::Text(format!("{label}: "), NORMAL), El::Text(value.into(), color)], ..Self::default() }
    }
    pub fn push(&mut self, el: El) -> &mut Self {
        self.els.push(el);
        self
    }
    pub fn details(mut self) -> Self {
        self.detail = true;
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct Tooltip {
    pub icon: Option<ItemStack>,
    pub lines: Vec<Line>,
    /// The bottom-edge progress strip (breaking progress) and its colour.
    pub progress: Option<(f32, u32)>,
}

/// Text height (Jade's text element is the font height − 1), the gap between lines, and the
/// icon's graphic size and element size (Jade's item element is 18 tall, graphic 16 inside it).
const TEXT_H: f32 = 8.0;
const LINE_GAP: f32 = 2.0;
const PAD: f32 = 3.0;
const ICON: f32 = 16.0;
const ICON_EL: f32 = 18.0;

pub struct Measure<'a> {
    pub draw: &'a dyn DrawContext,
}

impl Measure<'_> {
    fn el_size(&self, el: &El) -> [f32; 2] {
        match el {
            El::Text(s, _) => [self.draw.text_width(s), TEXT_H],
            // Jade's scaled text: round(width · scale) wide, round(9 · scale) − 1 tall.
            El::Italic(s, _, scale) => {
                [(self.draw.text_width(s) * scale).round(), ((9.0 * scale).round() - 1.0).max(0.0)]
            }
            El::Item(_, size, _) => [*size + 2.0, *size + 2.0],
            El::Sprite(_, w, h) => [*w as f32, *h as f32],
            El::Arrow(_) => [22.0, 16.0],
            El::Check(_) => [8.0, 8.0],
            El::Space(w) => [*w, 0.0],
        }
    }

    pub fn line_size(&self, line: &Line) -> [f32; 2] {
        let mut w = 0.0f32;
        let mut h = 0.0f32;
        for el in &line.els {
            let [ew, eh] = self.el_size(el);
            w += ew;
            h = h.max(eh);
        }
        [w + if line.boxed { 6.0 } else { 0.0 }, h.max(TEXT_H)]
    }
}

/// Where each part of a tooltip goes, in content pixels (origin at the box's top-left inside the
/// padding).
pub struct Layout {
    /// Content width and height (without padding).
    pub size: [f32; 2],
    /// The icon's 16-pixel graphic (inside its 18-pixel element, at 1, 1).
    pub icon_at: Option<[f32; 2]>,
    pub lines: Vec<([f32; 2], [f32; 2])>,
}

pub fn layout(tip: &Tooltip, m: &Measure, icon_mode: usize, show_detail: bool) -> Layout {
    // Jade shows the icon in Top and Centered modes only (Inline and Hide draw none).
    let icon = tip.icon.is_some() && matches!(icon_mode, 0 | 1);
    let text_x = if icon { ICON_EL + 3.0 } else { 0.0 };
    let mut y = 0.0f32;
    let mut width = 0.0f32;
    let mut lines = Vec::with_capacity(tip.lines.len());
    for line in &tip.lines {
        if line.detail && !show_detail {
            lines.push(([text_x, y], [0.0, 0.0]));
            continue;
        }
        let size = m.line_size(line);
        lines.push(([text_x, y], size));
        y += size[1] + LINE_GAP;
        width = width.max(text_x + size[0]);
    }
    // Lines sit GAP apart; the trailing gap is not part of the box.
    let text_h = (y - LINE_GAP).max(0.0);
    let mut height = text_h;
    let mut icon_at = None;
    if icon {
        height = height.max(ICON_EL);
        width = width.max(text_x);
        let (iy, line_shift) = if icon_mode == 1 {
            // Centered: icon and lines each centred against the taller one.
            (((height - ICON_EL) / 2.0).floor(), ((height - text_h) / 2.0).floor())
        } else if text_h < ICON_EL {
            // Top: short line stacks are centred against the icon (Jade's rule).
            (0.0, ((ICON_EL - text_h) / 2.0).floor())
        } else {
            (0.0, 0.0)
        };
        icon_at = Some([1.0, iy + 1.0]);
        for (at, _) in &mut lines {
            at[1] += line_shift;
        }
    }
    Layout { size: [width.ceil(), height.ceil()], icon_at, lines }
}

/// A theme's colours: body, top edge, bottom edge, side edges. Sampled from Jade's theme sprites
/// (dark and waila, each a flat body with a 1-pixel edge line, lighter on top than bottom).
pub fn theme_colors(theme: usize) -> (u32, u32, u32, u32) {
    match theme {
        // Classic (Jade's "waila"): HWYLA's body.
        1 => (0xFF100010, 0xFF5000FF, 0xFF290080, 0xFF3C00BF),
        // Flat: square, one grey line. Not one of Jade's themes; kept from the first port.
        2 => (0xFF000000, 0xFF606060, 0xFF606060, 0xFF606060),
        // Dark (Jade's default): near-black body, grey edges.
        _ => (0xFF131313, 0xFF383838, 0xFF242424, 0xFF2E2E2E),
    }
}

fn with_alpha(argb: u32, a: f32) -> u32 {
    let base = ((argb >> 24) & 0xFF) as f32 / 255.0;
    let alpha = (base * a.clamp(0.0, 1.0) * 255.0).round() as u32;
    (alpha << 24) | (argb & 0x00FF_FFFF)
}

/// Draws the tooltip with its content's top-left at `origin` (GUI pixels), scaled.
pub struct Paint {
    pub origin: [f32; 2],
    pub scale: f32,
    /// The box's background alpha (config opacity × fade) and the content alpha (fade).
    pub bg_alpha: f32,
    pub alpha: f32,
    pub theme: usize,
    /// Size the box is drawn at (it may be morphing from an older size).
    pub box_size: [f32; 2],
    pub show_detail: bool,
}

impl Paint {
    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        let s = self.scale;
        [self.origin[0] + x * s, self.origin[1] + y * s, w * s, h * s]
    }

    pub fn background(&self, draw: &mut dyn DrawContext, progress: Option<(f32, u32)>) {
        let (body_c, top_c, bottom_c, side_c) = theme_colors(self.theme);
        let body = with_alpha(body_c, self.bg_alpha);
        let edge = |c: u32| with_alpha(c, self.bg_alpha);
        let [w, h] = self.box_size;
        let (x, y) = (-PAD, -PAD);
        let (bw, bh) = (w + 2.0 * PAD, h + 2.0 * PAD);
        if self.theme == 2 {
            // Flat: square body with a 1-pixel border.
            draw.fill(self.rect(x, y, bw, bh), body);
            for r in [[x, y, bw, 1.0], [x, y + bh - 1.0, bw, 1.0], [x, y, 1.0, bh], [x + bw - 1.0, y, 1.0, bh]] {
                draw.fill(self.rect(r[0], r[1], r[2], r[3]), edge(side_c));
            }
        } else {
            // Jade's theme sprites: the body with a 1-pixel apron whose corners are cut, and a
            // 1-pixel edge line on the box's border (the corners take the top/bottom colours).
            draw.fill(self.rect(x, y - 1.0, bw, 1.0), body);
            draw.fill(self.rect(x, y + bh, bw, 1.0), body);
            draw.fill(self.rect(x, y, bw, bh), body);
            draw.fill(self.rect(x - 1.0, y, 1.0, bh), body);
            draw.fill(self.rect(x + bw, y, 1.0, bh), body);
            draw.fill(self.rect(x, y, bw, 1.0), edge(top_c));
            draw.fill(self.rect(x, y + bh - 1.0, bw, 1.0), edge(bottom_c));
            draw.fill(self.rect(x, y + 1.0, 1.0, bh - 2.0), edge(side_c));
            draw.fill(self.rect(x + bw - 1.0, y + 1.0, 1.0, bh - 2.0), edge(side_c));
        }
        if let Some((ratio, color)) = progress {
            // Jade's strip: over the box's bottom edge, fading in to 60% of the box's alpha.
            let ratio = ratio.clamp(0.0, 1.0);
            draw.fill(self.rect(x, y + bh - 1.0, bw * ratio, 1.0), with_alpha(color, self.bg_alpha * 0.6));
        }
    }

    /// Items first (they draw under text), then one GUI paint buffer for text and sprites.
    pub fn content(&self, draw: &mut dyn DrawContext, tip: &Tooltip, lay: &Layout) {
        let s = self.scale;
        // Jade stops drawing items (icon, stacks, counts) below half fade.
        let items_visible = self.alpha >= 0.5;
        if items_visible
            && let (Some(stack), Some(at)) = (&tip.icon, lay.icon_at)
        {
            let r = self.rect(at[0], at[1], ICON, ICON);
            draw.item_stack(stack, [r[0], r[1]], ICON * s);
        }
        let pose = GuiDraw::Push(vec![PoseOp::Translate(self.origin[0], self.origin[1]), PoseOp::Scale(s, s)]);
        // Text in one buffer and sprites in another: a sprite the resource pack lacks refuses its
        // whole buffer, and the text must still show.
        let mut ops: Vec<GuiDraw> = vec![pose.clone()];
        let mut sprites: Vec<GuiDraw> = vec![pose];
        let a = self.alpha;
        let m = Measure { draw: &*draw };
        let mut items: Vec<(ItemStack, [f32; 2], f32)> = Vec::new();
        let mut box_rows: Option<(f32, f32)> = None;
        for (line, (at, size)) in tip.lines.iter().zip(&lay.lines) {
            if line.detail && !self.show_detail {
                continue;
            }
            if line.boxed {
                box_rows = Some(match box_rows {
                    Some((y0, _)) => (y0, at[1] + size[1]),
                    None => (at[1], at[1] + size[1]),
                });
            }
            let mut x = at[0] + if line.boxed { 3.0 } else { 0.0 };
            let line_h = size[1];
            for el in &line.els {
                let [ew, eh] = m.el_size(el);
                // Jade lays elements out from the line's top; text sits at the line's top, and
                // taller elements are centred, the way Jade's own item rows centre themselves.
                let y = at[1] + ((line_h - eh) / 2.0).max(0.0);
                match el {
                    El::Text(t, c) => ops.push(GuiDraw::Text {
                        text: Text::literal(t.clone()),
                        x: x.round() as i32,
                        y: at[1].round() as i32,
                        color: with_alpha(*c, a),
                        shadow: true,
                    }),
                    El::Italic(t, c, scale) => {
                        // Jade's scaled text drops by its scale: translate(x, y + scale) · scale.
                        let mut style = TextStyle::default();
                        style.italic = Some(true);
                        ops.push(GuiDraw::Push(vec![
                            PoseOp::Translate(x, at[1] + scale),
                            PoseOp::Scale(*scale, *scale),
                        ]));
                        ops.push(GuiDraw::Text {
                            text: Text::literal(t.clone()).with_style(style),
                            x: 0,
                            y: 0,
                            color: with_alpha(*c, a),
                            shadow: true,
                        });
                        ops.push(GuiDraw::Pop);
                    }
                    El::Item(stack, size, count) => {
                        // The 16-pixel graphic sits 1 pixel inside Jade's 18-pixel element.
                        let (gx, gy) = (x + 1.0, y + 1.0);
                        if items_visible {
                            items.push((stack.clone(), [gx, gy], *size));
                            if let Some(count) = count {
                                // Jade's corner count: 0.75× text (0.5× past 3 chars) with its
                                // right edge at (22 − width) and its top at 13, in graphic units.
                                let k = size / 16.0;
                                let (ts, tx, ty) =
                                    if count.chars().count() > 3 { (0.5, 32.0, 23.0) } else { (0.75, 22.0, 13.0) };
                                let cw = m.draw.text_width(count);
                                ops.push(GuiDraw::Push(vec![
                                    PoseOp::Translate(gx + (tx - cw) * ts * k, gy + ty * ts * k),
                                    PoseOp::Scale(ts * k, ts * k),
                                ]));
                                ops.push(GuiDraw::Text {
                                    text: Text::literal(count.clone()),
                                    x: 0,
                                    y: 0,
                                    color: with_alpha(INFO, a),
                                    shadow: true,
                                });
                                ops.push(GuiDraw::Pop);
                            }
                        }
                    }
                    El::Sprite(name, w, h) => sprites.push(GuiDraw::Sprite {
                        sprite: (*name).into(),
                        x: x.round() as i32,
                        y: y.round() as i32,
                        w: *w,
                        h: *h,
                        region: None,
                        color: with_alpha(0xFFFFFFFF, a),
                        blend: Blend::Textured,
                    }),
                    El::Arrow(ratio) => {
                        // Drawn from fills, column by column as Jade's sprites draw the arrow:
                        // a grey base under a white fill cropped at ceil(ratio · 22) columns,
                        // whose last row is a dark edge.
                        let (xi, yi) = (x.round() as i32, y.round() as i32);
                        let filled = (22.0 * ratio.clamp(0.0, 1.0)).ceil() as i32;
                        for c in 0..ARROW_W {
                            let (base, fill, edge_y) = arrow_column(c);
                            arrow_span(&mut ops, xi, yi, c, base, 0xFF8B8B8B, a);
                            if c < filled {
                                arrow_span(&mut ops, xi, yi, c, fill, 0xFFFFFFFF, a);
                                arrow_span(&mut ops, xi, yi, c, (edge_y, edge_y + 1), 0xFF686868, a);
                            }
                        }
                    }
                    El::Check(ok) => {
                        let (xi, yi) = (x.round() as i32, y.round() as i32);
                        for (px, py, pw, ph) in check_pixels(*ok) {
                            ops.push(GuiDraw::Fill {
                                x0: xi + px,
                                y0: yi + py,
                                x1: xi + px + pw,
                                y1: yi + py + ph,
                                color: with_alpha(if *ok { SUCCESS } else { DANGER }, a),
                            });
                        }
                    }
                    El::Space(_) => {}
                }
                x += ew;
            }
        }
        ops.push(GuiDraw::Pop);
        sprites.push(GuiDraw::Pop);
        if let Some((y0, y1)) = box_rows {
            // Jade's nested box: a 1-pixel grey outline around the lines, transparent inside.
            let color = with_alpha(0xFF808080, a * 0.75);
            let [rx, ry, rw, rh] = self.rect(0.0, y0 - 3.0, lay.size[0], y1 - y0 + 6.0);
            for r in [[rx, ry, rw, 1.0], [rx, ry + rh - 1.0, rw, 1.0], [rx, ry, 1.0, rh], [rx + rw - 1.0, ry, 1.0, rh]] {
                draw.fill(r, color);
            }
        }
        for (stack, at, size) in &items {
            let r = self.rect(at[0], at[1], *size, *size);
            draw.item_stack(stack, [r[0], r[1]], size * s);
        }
        if sprites.len() > 2
            && let Err(e) = draw.gui_paint(&sprites)
        {
            report_once(&format!("sprites: {e:?}"));
        }
        if let Err(e) = draw.gui_paint(&ops) {
            report_once(&format!("text: {e:?}"));
        }
    }
}

/// The check and the cross, as 1-pixel fills in an 8 × 8 cell (drawn here, not a font glyph).
const ARROW_W: i32 = 22;

/// One column of Jade's arrow sprites in the 22 × 16 cell: the grey base span (start, end), the
/// white fill span (1 pixel higher, its bottom row a dark edge), and that edge's y. The shaft is
/// 3 rows thick (rows 7–9), the head spans rows 1–15 with its tip at column 21.
fn arrow_column(c: i32) -> ((i32, i32), (i32, i32), i32) {
    match c {
        0..=13 => ((7, 10), (6, 9), 9),
        14 => ((1, 16), (0, 15), 15),
        15 => ((2, 15), (1, 14), 14),
        16 => ((3, 14), (2, 13), 13),
        17 => ((4, 13), (3, 12), 12),
        18 => ((5, 12), (4, 11), 11),
        19 => ((6, 11), (5, 10), 10),
        20 => ((7, 10), (6, 9), 9),
        _ => ((8, 9), (7, 8), 8),
    }
}

fn arrow_span(ops: &mut Vec<GuiDraw>, xi: i32, yi: i32, c: i32, (y0, y1): (i32, i32), color: u32, a: f32) {
    if y1 <= y0 {
        return;
    }
    ops.push(GuiDraw::Fill {
        x0: xi + c,
        y0: yi + y0,
        x1: xi + c + 1,
        y1: yi + y1,
        color: with_alpha(color, a),
    });
}

fn check_pixels(ok: bool) -> Vec<(i32, i32, i32, i32)> {
    if ok {
        vec![(0, 4, 2, 1), (1, 5, 2, 1), (2, 6, 2, 1), (3, 5, 2, 1), (4, 4, 1, 1), (4, 3, 2, 1), (5, 2, 1, 1), (5, 1, 2, 1), (6, 0, 1, 1), (6, 1, 1, 1)]
    } else {
        (0..7).flat_map(|i| [(i, i, 1, 1), (6 - i, i, 1, 1)]).collect()
    }
}

fn report_once(e: &str) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static SAID: AtomicBool = AtomicBool::new(false);
    if !SAID.swap(true, Ordering::Relaxed) {
        eprintln!("[jade] a part of the tooltip was refused: {e}");
    }
}
