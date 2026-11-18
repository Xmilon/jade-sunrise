//! Jade's quick screen (the Open Config key, Numpad 0): every option as a button, with a live
//! preview of the overlay above it. Clicking changes the same settings Options > Mods > Jade
//! shows, and the config file follows.
//!
//! Toggles flip; choices cycle (right click goes back); numbers step up (right click: down).

use crate::config::{self, Kind, Value, OPTS};
use mod_api::*;
use std::sync::Mutex;

const W: f32 = 160.0;
const H: f32 = 20.0;
const GAP: f32 = 4.0;
/// The option list's top; above it the live preview (top centre) and the title.
const TOP: f32 = 76.0;

static SCROLL: Mutex<f32> = Mutex::new(0.0);
/// The GUI size the screen was last drawn at, for its clicks.
static SIZE: Mutex<[f32; 2]> = Mutex::new([0.0, 0.0]);

pub fn size() -> [f32; 2] {
    *SIZE.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn open(host: &mut dyn ClientHost) {
    *SCROLL.lock().unwrap_or_else(|e| e.into_inner()) = 0.0;
    host.mod_screen_open(crate::ID);
}

/// Some(()) while Jade's own screen is open.
pub fn open_now(host: &dyn ClientHost) -> Option<()> {
    (host.mod_screen() == Some(crate::ID)).then_some(())
}

fn columns(width: f32) -> usize {
    (((width - 20.0) / (W + GAP)).floor() as usize).clamp(1, 3)
}

/// Each option's button rect, in screen pixels, with the scroll applied.
fn rects(size: [f32; 2]) -> Vec<[f32; 4]> {
    let cols = columns(size[0]);
    let total_w = cols as f32 * W + (cols as f32 - 1.0) * GAP;
    let x0 = ((size[0] - total_w) / 2.0).floor();
    let scroll = *SCROLL.lock().unwrap_or_else(|e| e.into_inner());
    (0..OPTS.len())
        .map(|i| {
            let (col, row) = (i % cols, i / cols);
            [x0 + col as f32 * (W + GAP), TOP + row as f32 * (H + GAP) - scroll, W, H]
        })
        .collect()
}

fn done_rect(size: [f32; 2]) -> [f32; 4] {
    [(size[0] - 200.0) / 2.0, size[1] - 26.0, 200.0, 20.0]
}

fn label(host: &dyn ClientHost, i: usize) -> String {
    let opt = OPTS[i];
    let name = host.translate(&format!("config.jade.{}", opt.key)).unwrap_or_else(|| opt.key.replace('_', " "));
    let value = match (opt.kind, config::get(opt.key)) {
        (Kind::Toggle(_), Value::Bool(b)) => host
            .translate(if b { "options.on" } else { "options.off" })
            .unwrap_or_else(|| if b { "ON".into() } else { "OFF".into() }),
        (Kind::Choice(options, _), Value::Choice(c)) => {
            let o = options.get(c).copied().unwrap_or(options[0]);
            host.translate(&format!("config.jade.{}.{o}", opt.key)).unwrap_or_else(|| o.replace('_', " "))
        }
        (_, Value::Num(n)) => {
            let s = format!("{n:.2}");
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        }
        _ => String::new(),
    };
    format!("{name}: {value}")
}

fn inside(r: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

pub fn render(host: &dyn ClientHost, draw: &mut dyn DrawContext, mouse: [f32; 2]) {
    let size = draw.size();
    *SIZE.lock().unwrap_or_else(|e| e.into_inner()) = size;
    draw.fill([0.0, 0.0, size[0], size[1]], 0xA0101010);
    let title = host.translate("jade.config.title").unwrap_or_else(|| "Jade".into());
    let tw = draw.text_width(&title);
    draw.text(&title, [(size[0] - tw) / 2.0, TOP - 14.0], 0xFFFFFFFF, true);
    let clip = [0.0, TOP - 2.0, size[0], size[1] - 30.0 - (TOP - 2.0)];
    let _ = draw.push_clip(clip);
    for (i, r) in rects(size).into_iter().enumerate() {
        if r[1] + r[3] < clip[1] || r[1] > clip[1] + clip[3] {
            continue;
        }
        let hover = inside(r, mouse) && inside(clip, mouse);
        let _ = draw.widget_button(r, &Text::literal(label(host, i)), true, hover);
    }
    let _ = draw.pop_clip();
    let d = done_rect(size);
    let _ = draw.widget_button(d, &Text::translate("gui.done", Vec::new()), true, inside(d, mouse));
}

/// A click on the screen: true when it was taken.
pub fn click(host: &dyn ClientHost, size: [f32; 2], at: [f32; 2], button: u32) -> bool {
    if inside(done_rect(size), at) {
        return false;
    }
    let clip = [0.0, TOP - 2.0, size[0], size[1] - 30.0 - (TOP - 2.0)];
    if !inside(clip, at) {
        return true;
    }
    let Some(i) = rects(size).iter().position(|r| inside(*r, at)) else { return true };
    let opt = OPTS[i];
    let back = button == 1;
    let value = match (opt.kind, config::get(opt.key)) {
        (Kind::Toggle(_), Value::Bool(b)) => Value::Bool(!b),
        (Kind::Choice(options, _), Value::Choice(c)) => {
            let n = options.len();
            Value::Choice(if back { (c + n - 1) % n } else { (c + 1) % n })
        }
        (Kind::Range(min, max, step, _), Value::Num(v)) => {
            let big = if host.modifiers().shift() { 10.0 } else { 1.0 };
            let next = if back { v - step * big } else { v + step * big };
            let next = (next / step).round() * step;
            Value::Num(if next > max + 1e-9 { min } else if next < min - 1e-9 { max } else { next.clamp(min, max) })
        }
        (_, v) => v,
    };
    config::set(opt.key, value);
    host.set_setting(crate::ID, opt.key, &value.setting_text());
    true
}

/// Whether `at` is on the Done button.
pub fn on_done(size: [f32; 2], at: [f32; 2]) -> bool {
    inside(done_rect(size), at)
}

pub fn scroll(size: [f32; 2], amount: f32) {
    let rows = OPTS.len().div_ceil(columns(size[0]));
    let content = rows as f32 * (H + GAP);
    let view = size[1] - 30.0 - TOP;
    let max = (content - view).max(0.0);
    let mut s = SCROLL.lock().unwrap_or_else(|e| e.into_inner());
    *s = (*s - amount * 24.0).clamp(0.0, max);
}
