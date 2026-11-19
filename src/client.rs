//! The client: what the crosshair is on, the tooltip built for it every tick, the keys, the
//! server's answers, and drawing the overlay with its fade and resize animation.

use crate::config::{self, Value};
use crate::providers;
use crate::server;
use crate::tooltip::{self, Layout, Measure, Paint, Tooltip};
use mod_api::serde_json::Value as Json;
use mod_api::*;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// What the tooltip describes.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Block { pos: [i32; 3], state: BlockState, face: [i32; 3], hit: [f64; 3] },
    Entity { entity: EntityRef, id: u32 },
    Fluid { pos: [i32; 3], fluid: FluidState },
}

impl Target {
    pub fn same(&self, other: &Target) -> bool {
        match (self, other) {
            (Target::Block { pos: a, state: sa, .. }, Target::Block { pos: b, state: sb, .. }) => a == b && sa == sb,
            (Target::Entity { id: a, .. }, Target::Entity { id: b, .. }) => a == b,
            (Target::Fluid { pos: a, .. }, Target::Fluid { pos: b, .. }) => a == b,
            _ => false,
        }
    }
    fn matches_reply(&self, reply: &Json) -> bool {
        match self {
            Target::Block { pos, .. } => {
                reply["t"] == "b" && reply["p"].as_array().is_some_and(|p| p.len() == 3 && (0..3).all(|i| p[i].as_i64() == Some(pos[i] as i64)))
            }
            Target::Entity { id, .. } => reply["t"] == "e" && reply["id"].as_u64() == Some(*id as u64),
            Target::Fluid { .. } => false,
        }
    }
}

#[derive(Default)]
struct State {
    in_world: bool,
    /// The overlay's Toggle-mode switch (Show Overlay key).
    shown: bool,
    target: Option<Target>,
    /// The server's answer for the current target, and when it came.
    data: Option<Json>,
    asked_at: u64,
    asked_for: Option<Target>,
    /// The tooltip built this tick, and the last one drawn (kept for the disappearing delay).
    tooltip: Option<Tooltip>,
    lost_at: u64,
    details: bool,
    /// Fade (0 to 1) and the box's drawn size and origin, chasing the layout's.
    alpha: f32,
    box_size: [f32; 2],
    box_origin: [f32; 2],
    last_frame: u64,
    started: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(|| State { shown: true, ..State::default() }))
}

/// Mod display names by namespace, from the loaded packages (read at registration).
static MOD_NAMES: OnceLock<HashMap<String, String>> = OnceLock::new();

pub fn set_mod_names(names: HashMap<String, String>) {
    let _ = MOD_NAMES.set(names);
}

/// The display name of the mod that owns `namespace`: "Minecraft" for vanilla.
pub fn mod_name(namespace: &str) -> String {
    if let Some(name) = MOD_NAMES.get().and_then(|m| m.get(namespace)) {
        return name.clone();
    }
    match namespace {
        "minecraft" | "brigadier" => "Minecraft".into(),
        "sunrise" => "Sunrise".into(),
        other => {
            let mut s = other.replace('_', " ");
            if let Some(f) = s.get_mut(0..1) {
                f.make_ascii_uppercase();
            }
            s
        }
    }
}

pub fn received(reply: Json) {
    if std::env::var_os("JADE_DEBUG").is_some() {
        eprintln!("[jade] reply {reply}");
    }
    with(|s| {
        if s.target.as_ref().is_some_and(|t| t.matches_reply(&reply)) {
            s.data = Some(reply);
        }
    });
}

pub fn logging_in() {
    with(|s| {
        s.in_world = true;
        s.target = None;
        s.data = None;
        s.tooltip = None;
    });
}

pub fn logging_out() {
    with(|s| {
        s.in_world = false;
        s.target = None;
        s.data = None;
        s.tooltip = None;
        s.alpha = 0.0;
    });
}

pub const KEYS: &[(&str, &str, &str)] = &[
    ("open_config", "key.keyboard.keypad.0", "key.jade.config"),
    ("show_overlay", "key.keyboard.keypad.1", "key.jade.show_overlay"),
    ("toggle_fluid", "key.keyboard.keypad.2", "key.jade.toggle_liquid"),
    ("narrate", "key.keyboard.keypad.5", "key.jade.narrate"),
    ("show_details", "key.keyboard.left.shift", "key.jade.show_details"),
];

fn reach(host: &dyn ClientHost, entity: bool) -> f64 {
    let attr = if entity { "minecraft:entity_interaction_range" } else { "minecraft:block_interaction_range" };
    let base = host
        .local_player()
        .and_then(|p| host.living(p.entity()))
        .and_then(|l| l.attribute_value(host, attr))
        .unwrap_or(if entity { 3.0 } else { 4.5 });
    base + config::num("extended_reach")
}

fn look(camera: &Camera) -> [f64; 3] {
    let (yaw, pitch) = ((camera.yaw as f64).to_radians(), (camera.pitch as f64).to_radians());
    [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()]
}

/// The first fluid cell along the view within `max` blocks, before `limit` (the block hit's
/// distance): a voxel walk reading the fluid in each cell, as the game's clip has no fluid mode.
fn fluid_along(host: &dyn ClientHost, level: LevelRef, from: [f64; 3], dir: [f64; 3], max: f64) -> Option<([i32; 3], FluidState, f64)> {
    let mut cell = [from[0].floor() as i32, from[1].floor() as i32, from[2].floor() as i32];
    let step = [dir[0].signum() as i32, dir[1].signum() as i32, dir[2].signum() as i32];
    let mut t_max = [0.0f64; 3];
    let mut t_delta = [f64::INFINITY; 3];
    for a in 0..3 {
        if dir[a] != 0.0 {
            let next = if dir[a] > 0.0 { cell[a] as f64 + 1.0 } else { cell[a] as f64 };
            t_max[a] = (next - from[a]) / dir[a];
            t_delta[a] = (1.0 / dir[a]).abs();
        } else {
            t_max[a] = f64::INFINITY;
        }
    }
    let mut t = 0.0;
    for _ in 0..64 {
        if t > max {
            return None;
        }
        if let Some(f) = level.fluid_state(host, cell)
            && f.amount > 0
        {
            // The fluid's surface: a full block for a falling or source cell, else its level.
            let height = if f.source || f.falling { 1.0 } else { f.amount as f64 / 9.0 };
            let y_at = from[1] + dir[1] * t;
            if y_at - cell[1] as f64 <= height + 1e-3 || dir[1] < 0.0 {
                return Some((cell, f, t));
            }
        }
        let a = if t_max[0] < t_max[1] { if t_max[0] < t_max[2] { 0 } else { 2 } } else if t_max[1] < t_max[2] { 1 } else { 2 };
        t = t_max[a];
        t_max[a] += t_delta[a];
        cell[a] += step[a];
    }
    None
}

/// Where a ray from `from` along `dir` (a unit vector) enters `aabb`: 0 when it starts inside,
/// None when it misses or enters past `max`.
fn ray_box(from: [f64; 3], dir: [f64; 3], aabb: [[f64; 3]; 2], max: f64) -> Option<f64> {
    let (mut near, mut far) = (0.0f64, max);
    for a in 0..3 {
        let (lo, hi) = (aabb[0][a], aabb[1][a]);
        if dir[a].abs() < 1e-12 {
            if from[a] < lo || from[a] > hi {
                return None;
            }
            continue;
        }
        let (t0, t1) = ((lo - from[a]) / dir[a], (hi - from[a]) / dir[a]);
        near = near.max(t0.min(t1));
        far = far.min(t0.max(t1));
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// The nearest entity along the view within `max` blocks, by Jade's own ray rather than the
/// game's pick: dropped items count (vanilla's crosshair skips them), and the reach is the
/// player's own plus Extended Reach. A box thinner than 0.3 on an axis is widened to 0.3 there.
/// Reads the client's tracked entities (`LevelRef::entities_in`) and their boxes.
fn entity_along(host: &dyn ClientHost, level: LevelRef, from: [f64; 3], dir: [f64; 3], max: f64) -> Option<(EntityRef, f64)> {
    let to = [from[0] + dir[0] * max, from[1] + dir[1] * max, from[2] + dir[2] * max];
    let lo = [from[0].min(to[0]) - 1.0, from[1].min(to[1]) - 1.0, from[2].min(to[2]) - 1.0];
    let hi = [from[0].max(to[0]) + 1.0, from[1].max(to[1]) + 1.0, from[2].max(to[2]) + 1.0];
    let me = host.local_player().map(|p| p.entity());
    let feet = me.and_then(|p| p.position(host));
    let mut best: Option<(EntityRef, f64)> = None;
    for e in level.entities_in(host, lo, hi).unwrap_or_default() {
        if Some(e.id()) == me.map(|p| p.id()) {
            continue;
        }
        let Some(mut b) = e.bounding_box(host) else { continue };
        // What the player rides holds the player's feet (the client can't read vehicles).
        if let Some(f) = feet
            && (0..3).all(|a| f[a] >= b[0][a] - 0.01 && f[a] <= b[1][a] + 0.01)
        {
            continue;
        }
        for a in 0..3 {
            let size = b[1][a] - b[0][a];
            if size < 0.3 {
                let grow = (0.3 - size) / 2.0;
                b[0][a] -= grow;
                b[1][a] += grow;
            }
        }
        if let Some(t) = ray_box(from, dir, b, max)
            && best.as_ref().is_none_or(|(_, bt)| t < *bt)
        {
            best = Some((e, t));
        }
    }
    best
}

fn find_target(host: &dyn ClientHost) -> Option<Target> {
    let crosshair = host.crosshair_target();
    let camera = host.camera();
    let level = host.local_player().and_then(|p| p.level(host));
    let breaking = host
        .local_player()
        .and_then(|p| p.block_breaking(host))
        .is_some_and(|b| matches!(b, BlockBreakingState::Breaking { .. }));
    let vanilla_entity = matches!(crosshair, Some(CrosshairTarget::Entity { .. }));
    let mut target = match crosshair {
        Some(CrosshairTarget::Block { pos, state, hit }) => {
            Some((Target::Block { pos: [pos.x, pos.y, pos.z], state, face: hit.face, hit: hit.point }, distance(camera.as_ref(), hit.point)))
        }
        Some(CrosshairTarget::Entity { entity, point }) => Some((Target::Entity { entity, id: entity.id() }, distance(camera.as_ref(), point))),
        _ => None,
    };
    // Extended reach: a block past the game's own reach.
    if target.is_none()
        && config::num("extended_reach") > 0.0
        && let (Some(cam), Some(level)) = (camera.as_ref(), level)
    {
        let d = look(cam);
        let r = reach(host, false);
        let to = [cam.position[0] + d[0] * r, cam.position[1] + d[1] * r, cam.position[2] + d[2] * r];
        if let Some(hit) = level.clip(host, cam.position, to, ClipBlock::Outline, ClipFluid::None)
            && !hit.miss
            && let Some(state) = level.block_state(host, hit.block)
        {
            let face = face_vec(hit.face);
            target = Some((Target::Block { pos: hit.block, state, face, hit: hit.location }, distance(Some(cam), hit.location)));
        }
    }
    // Entities the game's pick leaves out: dropped items, and mobs past its reach with Extended
    // Reach. The game's own entity hit stands; while a block is being mined, dropped items are
    // not targets (Jade's rule), so the game's answer stands then too.
    if !vanilla_entity
        && !breaking
        && let (Some(cam), Some(level)) = (camera.as_ref(), level)
    {
        let limit = target.as_ref().map(|(_, d)| *d).unwrap_or(f64::INFINITY).min(reach(host, true));
        if let Some((entity, d)) = entity_along(host, level, cam.position, look(cam), limit) {
            target = Some((Target::Entity { entity, id: entity.id() }, d));
        }
    }
    let fluid_mode = config::choice("fluid_mode");
    if fluid_mode != 1
        && let (Some(cam), Some(level)) = (camera.as_ref(), level)
        && (fluid_mode == 0 || target.is_none())
    {
        let limit = target.as_ref().map(|(_, d)| *d).unwrap_or_else(|| reach(host, false));
        if let Some((pos, fluid, d)) = fluid_along(host, level, cam.position, look(cam), limit)
            && target.as_ref().is_none_or(|(_, td)| d < *td - 1e-3)
        {
            target = Some((Target::Fluid { pos, fluid }, d));
        }
    }
    target.map(|(t, _)| t)
}

fn face_vec(face: BlockFace) -> [i32; 3] {
    match face {
        BlockFace::Down => [0, -1, 0],
        BlockFace::Up => [0, 1, 0],
        BlockFace::North => [0, 0, -1],
        BlockFace::South => [0, 0, 1],
        BlockFace::West => [-1, 0, 0],
        _ => [1, 0, 0],
    }
}

fn distance(camera: Option<&Camera>, at: [f64; 3]) -> f64 {
    camera.map_or(0.0, |c| {
        let d = [c.position[0] - at[0], c.position[1] - at[1], c.position[2] - at[2]];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    })
}

/// Once a client tick: keys, the target, the server request, the tooltip.
pub fn tick(host: &mut dyn ClientHost) {
    if !host.on(crate::ID) {
        return;
    }
    if !with(|s| std::mem::replace(&mut s.started, true)) {
        config::read_settings(host);
    }
    config::sync(host);
    let in_world = host.local_player().is_some();
    if !in_world {
        with(|s| s.tooltip = None);
        return;
    }
    // Keys.
    while host.consume_click(crate::ID, "show_overlay") {
        if config::choice("display_mode") == 0 {
            with(|s| s.shown = !s.shown);
        }
    }
    while host.consume_click(crate::ID, "toggle_fluid") {
        let next = (config::choice("fluid_mode") + 1) % config::FLUID_MODES.len();
        config::set("fluid_mode", Value::Choice(next));
        host.set_setting(crate::ID, "fluid_mode", &next.to_string());
    }
    while host.consume_click(crate::ID, "open_config") {
        crate::screen::open(host);
    }
    let narrate = {
        let mut n = false;
        while host.consume_click(crate::ID, "narrate") {
            n = true;
        }
        n
    };
    let details = host.is_down(crate::ID, "show_details");
    let target = if config::on("enabled") { find_target(host) } else { None };
    let target = target.filter(|t| match t {
        Target::Block { .. } | Target::Fluid { .. } => config::on("display_blocks"),
        Target::Entity { .. } => config::on("display_entities"),
    });
    let now = host.millis();
    // The request: at once for a new target, then every 250 ms.
    let request = with(|s| {
        if s.target.as_ref().map(|t| target.as_ref().is_some_and(|n| n.same(t))) != Some(true) {
            s.data = None;
        }
        s.target = target.clone();
        s.details = details;
        let new = s.asked_for.as_ref().map(|a| target.as_ref().is_some_and(|n| n.same(a))) != Some(true);
        let due = now >= s.asked_at + 250;
        match &target {
            Some(t) if new || due => {
                s.asked_at = now;
                s.asked_for = Some(t.clone());
                match t {
                    Target::Block { pos, .. } => Some(server::request_block(*pos, details)),
                    Target::Entity { id, .. } => Some(server::request_entity(*id, details)),
                    Target::Fluid { .. } => None,
                }
            }
            None => {
                s.asked_for = None;
                None
            }
            _ => None,
        }
    });
    if let Some(bytes) = request {
        let _ = host.send_payload_to_server(&PayloadId(server::CHANNEL.into()), PayloadBytes(bytes));
    }
    if std::env::var_os("JADE_DEBUG").is_some()
        && let Some(Target::Block { pos, .. }) = &target
        && with(|s| s.data.is_none())
        && let Some(level) = host.local_player().and_then(|p| p.level(host))
    {
        // What the client's own copy of the block entity holds (API-GAPS #10).
        let copy = host.block_entity(level, *pos).map(|b| b.data(host).map(|c| server::compound_json(&c)));
        eprintln!("[jade] client copy at {pos:?}: {copy:?}");
    }
    let data = with(|s| s.data.clone());
    let built = target.as_ref().and_then(|t| providers::build(host, t, data.as_ref(), details));
    if narrate && let Some(tip) = &built {
        let text: Vec<String> = tip.lines.iter().map(providers::line_text).filter(|s| !s.is_empty()).collect();
        let _ = host.narrate(&Text::literal(text.join(", ")), Narration::Now);
    }
    with(|s| match built {
        Some(tip) => {
            s.tooltip = Some(tip);
            s.lost_at = 0;
        }
        None => {
            if s.lost_at == 0 {
                s.lost_at = now;
            }
            let delay = (config::num("disappearing_delay") * 1000.0) as u64;
            if now.saturating_sub(s.lost_at) >= delay {
                s.tooltip = None;
            }
        }
    });
}

/// Whether the overlay may draw now: its mode's key, open screens and the player list.
fn visible(host: &dyn ClientHost, s: &State) -> bool {
    if !config::on("enabled") || host.hud_mode() != HudMode::AllVisible {
        return false;
    }
    if config::on("hide_from_guis") && !host.playing() && crate::screen::open_now(host).is_none() {
        return false;
    }
    if config::on("hide_from_player_list") && host.mapping("key.playerlist").ok().flatten().is_some_and(|m| m.down) {
        return false;
    }
    match config::choice("display_mode") {
        0 => s.shown,
        1 => host.is_down(crate::ID, "show_overlay"),
        _ => true,
    }
}

/// The boss bars drawn this frame (`customize_boss_event_progress`, told before the HUD layer
/// that draws the tooltip), each as its title and bar: `[x, y, w, h]` in GUI pixels.
static BOSS_BARS: Mutex<Vec<[f32; 4]>> = Mutex::new(Vec::new());
/// The tooltip box last drawn, `[x, y, w, h]` in GUI pixels, for Hide Boss Bar.
static LAST_BOX: Mutex<Option<[f32; 4]>> = Mutex::new(None);

fn overlaps(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

/// A boss bar is about to be drawn at `x, y` (its bar's top-left; the title sits 9 pixels above).
/// True to hide it: Boss Bar Overlap is Hide Boss Bar and the tooltip covers it.
pub fn boss_bar(x: i32, y: i32) -> bool {
    let rect = [x as f32, y as f32 - 9.0, 182.0, 14.0];
    let mut bars = BOSS_BARS.lock().unwrap_or_else(|e| e.into_inner());
    if bars.len() < 32 {
        bars.push(rect);
    }
    config::BOSS_BAR_MODES.get(config::choice("boss_bar_overlap")) == Some(&"hide_boss_bar")
        && LAST_BOX.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|b| overlaps(b, rect))
}

/// The HUD layer: the tooltip, faded and morphing toward its new size.
pub fn draw(host: &dyn ClientHost, draw: &mut dyn DrawContext) {
    let bars = std::mem::take(&mut *BOSS_BARS.lock().unwrap_or_else(|e| e.into_inner()));
    if !host.on(crate::ID) {
        return;
    }
    let preview = crate::screen::open_now(host).is_some();
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(s) = g.as_mut() else { return };
    let now = host.millis();
    let dt = now.saturating_sub(s.last_frame).min(250) as f32;
    s.last_frame = now;
    let mut want = visible(host, s) && (s.tooltip.is_some() || preview);
    let animate = config::on("animation");
    let tip = if preview && s.tooltip.is_none() { providers::preview(host) } else { s.tooltip.clone().unwrap_or_default() };
    let lite = config::choice("display_mode") == 2;
    let show_detail = !lite || s.details;
    let lay: Layout = tooltip::layout(&tip, &Measure { draw: &*draw }, config::choice("icon_mode"), show_detail);
    let [sw, sh] = draw.size();
    let mut scale = config::num("scale") as f32;
    let full = [lay.size[0] + 6.0, lay.size[1] + 6.0];
    let threshold = config::num("auto_scale_threshold") as f32;
    if full[1] * scale > sh * threshold {
        scale = (sh * threshold / full[1]).max(scale * 0.5).min(scale);
    }
    let (mut px, py) = (config::num("pos_x") as f32, config::num("pos_y") as f32);
    if config::on("flip_main_hand") && host.gui_options().main_hand_left {
        px = 1.0 - px;
    }
    let (ax, ay) = (config::num("anchor_x") as f32, config::num("anchor_y") as f32);
    let point = [sw * px, sh * (1.0 - py)];
    // The box's top-left (outside the padding), then the content's.
    let mut box_xy = [point[0] - ax * full[0] * scale, point[1] - ay * full[1] * scale];
    let mut top_gap = if ay == 0.0 && py >= 0.99 { 3.0 } else { 0.0 };
    // Boss Bar Overlap, against the bars drawn this frame (with the frame's 1-pixel apron).
    let apron = [box_xy[0] - scale, box_xy[1] + top_gap - scale, full[0] * scale + 2.0 * scale, full[1] * scale + 2.0 * scale];
    if !preview && bars.iter().any(|b| overlaps(apron, *b)) {
        match config::BOSS_BAR_MODES.get(config::choice("boss_bar_overlap")).copied() {
            Some("push_down") => {
                let below = bars.iter().map(|b| b[1] + b[3]).fold(0.0f32, f32::max);
                box_xy[1] = box_xy[1].max(below + 2.0 + scale);
                top_gap = 0.0;
            }
            Some("hide_tooltip") => want = false,
            _ => {}
        }
    }
    // Jade's fade: 0.6 a tick toward shown or hidden (a tick is 50 ms).
    let step = if animate { 0.6 * dt / 50.0 } else { 1.0 };
    s.alpha = if want { (s.alpha + step).min(1.0) } else { (s.alpha - step).max(0.0) };
    if (s.alpha < 0.1 && !want) || (tip.lines.is_empty() && tip.icon.is_none()) {
        *LAST_BOX.lock().unwrap_or_else(|e| e.into_inner()) = None;
        return;
    }
    let mut origin = [box_xy[0] + 3.0 * scale, box_xy[1] + 3.0 * scale + top_gap];
    origin[0] = origin[0].round();
    origin[1] = origin[1].round();
    // The 75 ms morph from the old box to the new one.
    let mut size = lay.size;
    if animate && s.box_size != [0.0, 0.0] && s.alpha > 0.0 {
        let k = (dt / 75.0).min(1.0);
        for i in 0..2 {
            size[i] = s.box_size[i] + (lay.size[i] - s.box_size[i]) * k;
            origin[i] = s.box_origin[i] + (origin[i] - s.box_origin[i]) * k;
        }
    }
    s.box_size = size;
    s.box_origin = origin;
    *LAST_BOX.lock().unwrap_or_else(|e| e.into_inner()) =
        Some([origin[0] - 4.0 * scale, origin[1] - 4.0 * scale, (size[0] + 8.0) * scale, (size[1] + 8.0) * scale]);
    let morphing = (size[0] - lay.size[0]).abs() > 0.5 || (size[1] - lay.size[1]).abs() > 0.5;
    let alpha = if morphing { s.alpha.max(0.55) } else { s.alpha };
    let paint = Paint {
        origin,
        scale,
        bg_alpha: config::num("opacity") as f32 * alpha,
        alpha,
        theme: config::choice("theme"),
        box_size: size,
        show_detail,
    };
    paint.background(draw, tip.progress);
    paint.content(draw, &tip, &lay);
}
