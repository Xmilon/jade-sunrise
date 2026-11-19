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
    /// Narration on or off: the key flips it (Jade's default `TTSMode.TOGGLE`,
    /// JadeClient.java:161-167); while on, the box's text is narrated at most every 500 ms
    /// (WailaTickHandler.narrate). Jade's Press mode needs a `tts_mode` option the port's
    /// config table lacks, and Jade persists its switch in the accessibility config.
    tts: bool,
    last_narration: String,
    last_narration_at: u64,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(State::default))
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

fn reach(host: &dyn ClientHost, entity: bool, extra: f64) -> f64 {
    let attr = if entity { "minecraft:entity_interaction_range" } else { "minecraft:block_interaction_range" };
    let base = host
        .local_player()
        .and_then(|p| host.living(p.entity()))
        .and_then(|l| l.attribute_value(host, attr))
        .unwrap_or(if entity { 3.0 } else { 4.5 });
    base + config::num("extended_reach") + extra
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
/// player's own plus Extended Reach. Jade's pick copies ProjectileUtil's (RayTracing.java:43-75):
/// a box whose largest side is under 0.3 is grown by 0.3 on every axis per side, and the first
/// box holding the ray's start wins at once. Reads the client's tracked entities
/// (`LevelRef::entities_in`) and their boxes; the ride stand-in (the client can't read
/// vehicles) skips any box holding the player's feet.
fn entity_along(host: &dyn ClientHost, level: LevelRef, from: [f64; 3], dir: [f64; 3], max: f64) -> Option<(EntityRef, f64, [f64; 3])> {
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
        // Jade: `AABB.getSize()` — the largest side — under 0.3, then `inflate(0.3)`: 0.3 more
        // on every axis per side, so a dropped item's quarter-block box is easy to hit.
        let size = (0..3).fold(0.0f64, |m, a| m.max(b[1][a] - b[0][a]));
        if size < 0.3 {
            for a in 0..3 {
                b[0][a] -= 0.3;
                b[1][a] += 0.3;
            }
        }
        if let Some(t) = ray_box(from, dir, b, max)
            && best.as_ref().is_none_or(|(_, bt)| t < *bt)
        {
            best = Some((e, t));
        }
    }
    best.map(|(e, t)| {
        let at = [from[0] + dir[0] * t, from[1] + dir[1] * t, from[2] + dir[2] * t];
        (e, t, at)
    })
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
    let mut vanilla_hit = None;
    let mut target: Option<(Target, f64)> = match crosshair {
        Some(CrosshairTarget::Block { pos, state, hit }) => {
            vanilla_hit = Some(hit.point);
            Some((
                Target::Block { pos: [pos.x, pos.y, pos.z], state, face: hit.face, hit: hit.point },
                distance(camera.as_ref(), hit.point),
            ))
        }
        Some(CrosshairTarget::Entity { entity, point }) => {
            vanilla_hit = Some(point);
            Some((Target::Entity { entity, id: entity.id() }, distance(camera.as_ref(), point)))
        }
        _ => None,
    };
    let (Some(cam), Some(level)) = (camera.as_ref(), level) else {
        return target.map(|(t, _)| t);
    };
    // Jade traces from the camera and adds the camera-to-eye distance to both reaches
    // (RayTracing.java:114-125; past 1e8 squared it calls the hit invalid and aims ahead).
    // Perspective Mode EYE instead starts the trace at the eye, adds no distance and keeps the
    // view vector, without re-aiming through the game's hit (RayTracing.java:115-136).
    let eye = host.local_player().and_then(|p| p.entity().eye_position(host));
    let from_eye = config::choice("perspective_mode") == 1;
    let mut extra = 0.0;
    if !from_eye && let Some(eye) = eye {
        let d2 = dist2(eye, cam.position);
        if d2 > 1e-5 && d2 <= 1e8 {
            extra = d2.sqrt();
        }
    }
    let origin = if from_eye { eye.unwrap_or(cam.position) } else { cam.position };
    // Jade re-aims through the game's own hit rather than straight ahead (RayTracing.java:127-136).
    let straight = look(cam);
    let dir = match vanilla_hit {
        Some(at) if !from_eye && !eye.is_some_and(|e| dist2(e, cam.position) > 1e8) => {
            let v = [at[0] - cam.position[0], at[1] - cam.position[1], at[2] - cam.position[2]];
            let l2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
            if l2 > 1e-12 { [v[0] / l2.sqrt(), v[1] / l2.sqrt(), v[2] / l2.sqrt()] } else { straight }
        }
        _ => straight,
    };
    // Extended reach: a block past the game's own reach. Jade's own block clip always runs at
    // its block reach times 1.001 (RayTracing.java:143-156); the game's pick covers the same ray
    // at the base reach, so the port clips only for the extra.
    if target.is_none() && config::num("extended_reach") > 0.0 {
        let r = reach(host, false, extra) * 1.001;
        let to = [origin[0] + dir[0] * r, origin[1] + dir[1] * r, origin[2] + dir[2] * r];
        if let Some(hit) = level.clip(host, origin, to, ClipBlock::Outline, ClipFluid::None)
            && !hit.miss
            && let Some(state) = level.block_state(host, hit.block)
        {
            let face = face_vec(hit.face);
            target = Some((Target::Block { pos: hit.block, state, face, hit: hit.location }, distance(Some(cam), hit.location)));
        }
    }
    // Jade's own entity ray: dropped items (the game's pick skips them) and, with Extended
    // Reach, mobs past the game's entity reach. While a block is being mined, dropped items are
    // not targets (Jade's rule), so the game's answer stands then too. Jade's winner must also
    // not be occluded by a collision-shape clip landing closer than it (RayTracing.java:158-171):
    // an entity beside the block hit still counts.
    if !vanilla_entity
        && !breaking
        && let Some((entity, d, at)) = entity_along(host, level, origin, dir, reach(host, true, extra))
    {
        let occluded = level
            .clip(host, origin, at, ClipBlock::Collider, ClipFluid::None)
            .is_some_and(|h| !h.miss && dist2(h.location, origin) < dist2(at, origin));
        if !occluded {
            target = Some((Target::Entity { entity, id: entity.id() }, d));
        }
    }
    // Fluids, per Fluid Mode. A vanilla entity hit ends Jade's fire() at once
    // (RayTracing.java:84-91), so fluids never replace it; and when the eye sits in a fluid,
    // Jade's clip ignores fluids for that tick (RayTracing.java:147-152), so the fluid being
    // swum in is never the target.
    let fluid_mode = config::choice("fluid_mode");
    if !vanilla_entity && fluid_mode != 1 && (fluid_mode == 0 || target.is_none()) {
        let eye_in_fluid = eye
            .map(|e| [e[0].floor() as i32, e[1].floor() as i32, e[2].floor() as i32])
            .and_then(|cell| level.fluid_state(host, cell))
            .is_some_and(|f| f.amount > 0);
        if !eye_in_fluid {
            let limit = target.as_ref().map(|(_, d)| *d).unwrap_or_else(|| reach(host, false, extra) * 1.001);
            if let Some((pos, fluid, d)) = fluid_along(host, level, origin, dir, limit)
                && target.as_ref().is_none_or(|(_, td)| d < *td - 1e-3)
            {
                target = Some((Target::Fluid { pos, fluid }, d));
            }
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
    camera.map_or(0.0, |c| dist2(c.position, at).sqrt())
}

fn dist2(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
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
        // In Toggle mode Jade flips Display Tooltip itself and saves the config
        // (JadeClient.java:133-147). The toast it shows the first time has no API here
        // (API-GAPS #6), and the narration of the new state rides on Jade's narrate-keys
        // accessibility option, which the port's config table lacks.
        if config::choice("display_mode") == 0 {
            let on = !config::on("enabled");
            config::set("enabled", Value::Bool(on));
            host.set_setting(crate::ID, "enabled", if on { "true" } else { "false" });
        }
    }
    while host.consume_click(crate::ID, "toggle_fluid") {
        // Jade toggles fluids on and off, Any <-> None, also from Fallback
        // (JadeClient.java:149-155); Fallback itself is a config-screen choice only.
        let next = if config::choice("fluid_mode") == 1 { 0 } else { 1 };
        config::set("fluid_mode", Value::Choice(next));
        host.set_setting(crate::ID, "fluid_mode", &next.to_string());
    }
    while host.consume_click(crate::ID, "open_config") {
        crate::screen::open(host);
    }
    let narrate_pressed = {
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
    // Narration: the key flips Jade's TTS (its default Toggle mode, JadeClient.java:161-179);
    // while on, the target's text is narrated at most every 500 ms, never the same text twice
    // in a row, the rate counting from each attempt (WailaTickHandler.java:66-97).
    let mut narrate_msg = None;
    with(|s| {
        if narrate_pressed {
            s.tts = !s.tts;
        }
        if s.tts && let Some(tip) = &built && now >= s.last_narration_at + 500 {
            s.last_narration_at = now;
            let text = tip.lines.iter().map(providers::line_text).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(", ");
            if !text.is_empty() && text != s.last_narration {
                s.last_narration = text.clone();
                narrate_msg = Some(text);
            }
        }
    });
    if let Some(text) = narrate_msg {
        let _ = host.narrate(&Text::literal(text), Narration::Now);
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
            // Jade's value counts partial ticks against the linger box
            // (WailaTickHandler.java:144-146, OverlayRenderer.java:143-151); a tick is 50 ms.
            let delay = (config::num("disappearing_delay") * 50.0) as u64;
            if now.saturating_sub(s.lost_at) >= delay {
                s.tooltip = None;
            }
            // Jade's clearState resets the narration dedupe (WailaTickHandler.java:99-104).
            s.last_narration.clear();
        }
    });
}

/// The boss bars drawn this frame (`customize_boss_event_progress`, told before the HUD layer
/// that draws the tooltip), each as its title and bar: `[x, y, w, h]` in GUI pixels.
static BOSS_BARS: Mutex<Vec<[f32; 4]>> = Mutex::new(Vec::new());
/// The tooltip box last drawn, `[x, y, w, h]` in GUI pixels, for Hide Boss Bar.
static LAST_BOX: Mutex<Option<[f32; 4]>> = Mutex::new(None);

fn overlaps(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

/// A boss bar is about to be drawn at `x, y` (its bar's top-left; the title sits 9 pixels
/// above). True to hide it: Boss Bar Overlap is Hide Boss Bar and the tooltip drew the frame
/// before — Jade cancels the whole boss-bar overlay while the tooltip is up, with no overlap
/// test (BossHealthOverlayMixin against `OverlayRenderer.shown`).
pub fn boss_bar(x: i32, y: i32) -> bool {
    let hide_bars = {
        let mut bars = BOSS_BARS.lock().unwrap_or_else(|e| e.into_inner());
        if bars.len() < 32 {
            bars.push([x as f32, y as f32 - 9.0, 182.0, 14.0]);
        }
        config::BOSS_BAR_MODES.get(config::choice("boss_bar_overlap")) == Some(&"hide_boss_bar")
    };
    hide_bars && LAST_BOX.lock().unwrap_or_else(|e| e.into_inner()).is_some()
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
    // Jade's per-frame show (OverlayRenderer.shouldShow, OverlayRenderer.java:48-69): a tooltip
    // built this tick, the mod on, the mode's key held in Hold mode, and — Hide Tooltip — boss
    // bars drawn this frame (OverlayRenderer.java:62-66). Jade's own capture records the rect
    // only in Push Down, so its Hide Tooltip never fires there; the port keeps the option
    // working. The port's own screen stands in for Jade's preview screens, which Jade exempts.
    let mut show = (s.tooltip.is_some() || preview) && config::on("enabled");
    if show && config::choice("display_mode") == 1 && !host.is_down(crate::ID, "show_overlay") {
        show = false;
    }
    if show
        && !preview
        && !bars.is_empty()
        && config::BOSS_BAR_MODES.get(config::choice("boss_bar_overlap")) == Some(&"hide_tooltip")
    {
        show = false;
    }
    // Jade's hard hides (shouldShowImmediately, OverlayRenderer.java:71-108): a screen over the
    // world with Hide from GUIs on (Jade's preview screens exempt), F1, the player list with
    // Hide from Player List on. These stop the drawing at once, without a fade; the alpha still
    // follows `show`, so the tooltip is back at once when the hide ends.
    let player_list = host.mapping("key.playerlist").ok().flatten().is_some_and(|m| m.down);
    let hidden = (config::on("hide_from_guis") && !host.playing() && !preview)
        || host.hud_mode() != HudMode::AllVisible
        || (config::on("hide_from_player_list") && player_list);
    let animate = config::on("animation");
    // Jade's fade: 0.6 a tick toward shown or hidden, or a jump when Animation is off
    // (OverlayRenderer.java:152-159).
    let step = if animate { 0.6 * dt / 50.0 } else { 1.0 };
    s.alpha = if show { (s.alpha + step).min(1.0) } else { (s.alpha - step).max(0.0) };
    let tip = if preview && s.tooltip.is_none() { providers::preview(host) } else { s.tooltip.clone().unwrap_or_default() };
    if s.alpha < 0.1 || hidden || (tip.lines.is_empty() && tip.icon.is_none()) {
        *LAST_BOX.lock().unwrap_or_else(|e| e.into_inner()) = None;
        return;
    }
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
    // Boss Bar Overlap, Push Down: when the box (with the frame's 1-pixel apron) would cover a
    // bar drawn this frame, it moves below the lowest one (BoxElementImpl.updateExpectedRect).
    // Hide Tooltip cleared `show` above; Hide Boss Bar hides the bars themselves in boss_bar().
    let apron = [box_xy[0] - scale, box_xy[1] + top_gap - scale, full[0] * scale + 2.0 * scale, full[1] * scale + 2.0 * scale];
    if !preview
        && config::BOSS_BAR_MODES.get(config::choice("boss_bar_overlap")) == Some(&"push_down")
        && bars.iter().any(|b| overlaps(apron, *b))
    {
        let below = bars.iter().map(|b| b[1] + b[3]).fold(0.0f32, f32::max);
        box_xy[1] = box_xy[1].max(below + 2.0 + scale);
        top_gap = 0.0;
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
