//! The tooltip's model, its layout and its drawing.
//!
//! A tooltip is an optional icon and lines of elements. Layout runs in "content pixels" (the
//! tooltip at scale 1); drawing turns them into GUI pixels with the overlay's position and scale.
//! The box is drawn the way vanilla draws an item tooltip (a filled body and a two-colour gradient
//! frame), in the theme's colours. Hearts, armour and the furnace arrow are the game's own HUD and
//! container sprites. Nothing here is Jade's art.

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
pub const GREY: u32 = 0xFF555555;
pub const MOD_BLUE: u32 = 0xFF5555FF;

#[derive(Clone, Debug)]
pub enum El {
    /// Plain text in one colour.
    Text(String, u32),
    /// Italic text (the mod name), at a scale (1 or 0.75).
    Italic(String, u32, f32),
    /// A stack's icon `size` pixels square, with an optional count text in the corner.
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

/// Text height and line pitch.
const TEXT_H: f32 = 9.0;
const PAD: f32 = 3.0;
const ICON: f32 = 16.0;

pub struct Measure<'a> {
    pub draw: &'a dyn DrawContext,
}

impl Measure<'_> {
    fn el_size(&self, el: &El) -> [f32; 2] {
        match el {
            El::Text(s, _) => [self.draw.text_width(s), TEXT_H],
            El::Italic(s, _, scale) => [self.draw.text_width(s) * scale, TEXT_H * scale.max(0.75)],
            El::Item(_, size, _) => [*size, *size],
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
        [w + if line.boxed { 6.0 } else { 0.0 }, h.max(TEXT_H) + 1.0]
    }
}

/// Where each part of a tooltip goes, in content pixels (origin at the box's top-left inside the
/// padding).
pub struct Layout {
    /// Content width and height (without padding).
    pub size: [f32; 2],
    pub icon_at: Option<[f32; 2]>,
    pub lines: Vec<([f32; 2], [f32; 2])>,
}

pub fn layout(tip: &Tooltip, m: &Measure, icon_mode: usize, show_detail: bool) -> Layout {
    let icon = tip.icon.is_some() && icon_mode != 3;
    let text_x = if icon && icon_mode != 2 { ICON + 3.0 } else { 0.0 };
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
        y += size[1];
        width = width.max(text_x + size[0]);
    }
    // The last line's spacing is not part of the box.
    let text_h = (y - 1.0).max(0.0);
    let mut height = text_h;
    let mut icon_at = None;
    if icon {
        height = height.max(ICON);
        let iy = if icon_mode == 1 { ((height - ICON) / 2.0).max(0.0) } else { 0.0 };
        icon_at = Some([0.0, iy]);
        width = width.max(ICON);
        // Top: lines are centred on the icon when they are shorter than it.
        if icon_mode == 0 && text_h < ICON {
            let shift = ((ICON - text_h) / 2.0).floor();
            for (at, _) in &mut lines {
                at[1] += shift;
            }
        }
    }
    Layout { size: [width.ceil(), height.ceil()], icon_at, lines }
}

/// A theme's colours: body, frame top, frame bottom.
pub fn theme_colors(theme: usize) -> (u32, u32, u32) {
    match theme {
        // Classic: HWYLA's body (16, 0, 16) with the vanilla tooltip frame.
        1 => (0xFF100010, 0xFF5000FF, 0xFF28007F),
        // Flat: square, one grey line.
        2 => (0xFF000000, 0xFF606060, 0xFF606060),
        // Dark: near-black body, the vanilla tooltip frame.
        _ => (0xFF131313, 0xFF5000FF, 0xFF28007F),
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
        let (body, top, bottom) = theme_colors(self.theme);
        let body = with_alpha(body, self.bg_alpha);
        let frame_a = self.bg_alpha.max(0.0) * 0.8 + 0.2 * self.alpha;
        let (top, bottom) = (with_alpha(top, frame_a * 0.31), with_alpha(bottom, frame_a * 0.31));
        let [w, h] = self.box_size;
        let (x, y) = (-PAD, -PAD);
        let (bw, bh) = (w + 2.0 * PAD, h + 2.0 * PAD);
        if self.theme == 2 {
            draw.fill(self.rect(x, y, bw, bh), body);
            for r in [[x, y, bw, 1.0], [x, y + bh - 1.0, bw, 1.0], [x, y, 1.0, bh], [x + bw - 1.0, y, 1.0, bh]] {
                draw.fill(self.rect(r[0], r[1], r[2], r[3]), with_alpha(0xFF606060, self.alpha));
            }
        } else {
            // Vanilla's tooltip: the body with its corners cut, then the frame inside its edge.
            draw.fill(self.rect(x, y - 1.0, bw, 1.0), body);
            draw.fill(self.rect(x, y + bh, bw, 1.0), body);
            draw.fill(self.rect(x, y, bw, bh), body);
            draw.fill(self.rect(x - 1.0, y, 1.0, bh), body);
            draw.fill(self.rect(x + bw, y, 1.0, bh), body);
            draw.fill_gradient(self.rect(x, y + 1.0, 1.0, bh - 2.0), top, bottom);
            draw.fill_gradient(self.rect(x + bw - 1.0, y + 1.0, 1.0, bh - 2.0), top, bottom);
            draw.fill(self.rect(x, y, bw, 1.0), top);
            draw.fill(self.rect(x, y + bh - 1.0, bw, 1.0), bottom);
        }
        if let Some((ratio, color)) = progress {
            let ratio = ratio.clamp(0.0, 1.0);
            draw.fill(self.rect(x, y + bh - 1.0, bw * ratio, 1.0), with_alpha(color, self.alpha));
        }
    }

    /// Items first (they draw under text), then one GUI paint buffer for text and sprites.
    pub fn content(&self, draw: &mut dyn DrawContext, tip: &Tooltip, lay: &Layout) {
        let s = self.scale;
        if let (Some(stack), Some(at)) = (&tip.icon, lay.icon_at) {
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
            let line_h = size[1] - 1.0;
            for el in &line.els {
                let [ew, eh] = m.el_size(el);
                let y = at[1] + ((line_h - eh) / 2.0).max(0.0);
                match el {
                    El::Text(t, c) => ops.push(GuiDraw::Text {
                        text: Text::literal(t.clone()),
                        x: x.round() as i32,
                        y: (at[1] + ((line_h - 8.0) / 2.0).max(0.0)).round() as i32,
                        color: with_alpha(*c, a),
                        shadow: true,
                    }),
                    El::Italic(t, c, scale) => {
                        let mut style = TextStyle::default();
                        style.italic = Some(true);
                        ops.push(GuiDraw::Push(vec![PoseOp::Translate(x, y), PoseOp::Scale(*scale, *scale)]));
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
                        items.push((stack.clone(), [x, y], *size));
                        if let Some(count) = count {
                            let cw = m.draw.text_width(count);
                            // The slot's count corner, scaled with the icon.
                            let k = size / 16.0;
                            ops.push(GuiDraw::Push(vec![PoseOp::Translate(x + size - cw * k, y + size - 7.0 * k), PoseOp::Scale(k, k)]));
                            ops.push(GuiDraw::Text { text: Text::literal(count.clone()), x: 0, y: 0, color: with_alpha(INFO, a), shadow: true });
                            ops.push(GuiDraw::Pop);
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
                        // Drawn from fills: vanilla's burn_progress sprite carries the furnace
                        // slot's grey behind the arrow, so it can't sit on the tooltip's box.
                        let (xi, yi) = (x.round() as i32, y.round() as i32);
                        let filled = (ARROW_W as f32 * ratio.clamp(0.0, 1.0)).round() as i32;
                        for (cx, y0, y1) in arrow_columns() {
                            let color = if cx < filled { 0xFFFFFFFF } else { 0xFF5A5A5A };
                            ops.push(GuiDraw::Fill { x0: xi + cx, y0: yi + y0, x1: xi + cx + 1, y1: yi + y1, color: with_alpha(color, a) });
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
            // The nested box behind the effect lines.
            let r = self.rect(0.0, y0 - 1.0, lay.size[0], y1 - y0);
            draw.fill(r, with_alpha(0x40000000 | 0x00202020, a));
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

/// The progress arrow, column by column (x, top, bottom) in a 22x16 cell: a shaft 4 pixels thick,
/// then a head narrowing from the full height to a point.
fn arrow_columns() -> impl Iterator<Item = (i32, i32, i32)> {
    (0..ARROW_W).map(|c| if c < 13 { (c, 6, 10) } else { let k = c - 13; (c, k, 16 - k) })
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
