//! The providers: what each kind of target adds to its tooltip, in Jade's order (name line, body,
//! health and armour, mod name, registry id). Client-visible facts (block states, the held tool,
//! names) are read here; the rest comes from the server's answer (`data`), and its lines are
//! simply absent until it arrives or when the server doesn't run Jade.

use crate::client::{self, Target};
use crate::config;
use crate::tooltip::{El, Line, Tooltip, DANGER, FAILURE, GREY, INFO, MOD_BLUE, NORMAL, SUCCESS, TITLE, WARNING};
use mod_api::serde_json::Value as Json;
use mod_api::*;

// ---------------------------------------------------------------------------------------------
// Small helpers

pub fn item(host: &dyn ClientHost, id: &str) -> Option<ItemKey> {
    let ident = Identifier::parse(id).ok()?;
    let key = host.item_key(&ident)?;
    (id != "minecraft:air").then_some(key)
}

pub fn stack(host: &dyn ClientHost, id: &str, count: u32) -> Option<ItemStack> {
    ItemStack::new(item(host, id)?, count.max(1))
}

/// The language's text for `key`, else `fallback`.
pub fn tr(host: &dyn ClientHost, key: &str, fallback: &str) -> String {
    host.translate(key).unwrap_or_else(|| fallback.to_owned())
}

/// `key` with `%s` (or `%1$s`) replaced by the arguments in order.
pub fn trf(host: &dyn ClientHost, key: &str, fallback: &str, args: &[&str]) -> String {
    let mut s = tr(host, key, fallback);
    for (i, a) in args.iter().enumerate() {
        let positional = format!("%{}$s", i + 1);
        if s.contains(&positional) {
            s = s.replace(&positional, a);
        } else {
            s = s.replacen("%s", a, 1);
        }
    }
    s
}

fn split_id(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or(("minecraft", id))
}

/// `snake_case` as words: `iron_golem` reads "Iron Golem".
fn title_case(path: &str) -> String {
    path.split(['_', '/'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A count as an item slot shows it (Jade `humanReadableNumber`, decimal, no rounding):
/// `1234`, `12.3k`, `1.23M`, `9.99G`; under 10 000 stays plain.
fn short_count(n: u64) -> String {
    let mut n = n as f64;
    if n < 10_000.0 {
        return (n as u64).to_string();
    }
    let mut exp = (n.log10() / 3.0) as usize;
    exp = exp.min(7);
    n /= 1000f64.powi(exp as i32);
    let text = if n < 10.0 { df_down(n, 2) } else if n < 100.0 { df_down(n, 1) } else { df_down(n, 0) };
    const SUFFIX: &[u8] = b"kMGTPEZ";
    format!("{text}{}", SUFFIX[exp - 1] as char)
}

/// A number as Jade's `DecimalFormat("0.##")` with `RoundingMode.DOWN` shows it: at most
/// `places` decimals, zeros and the dot trimmed, never rounded up.
fn df_down(v: f64, places: usize) -> String {
    let scaled = v * 10f64.powi(places as i32);
    let truncated = if scaled < 0.0 { scaled.ceil() } else { scaled.floor() };
    let mut s = format!("{:.1$}", truncated / 10f64.powi(places as i32), places);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

/// A float the way Java's `Component` renders a float argument (`String.valueOf`): `6` becomes
/// `6.0`, `0.5` stays `0.5`.
fn java_float(v: f64) -> String {
    let s = format!("{v}");
    if s.contains('.') || s.contains("e") || s.contains("E") {
        s
    } else {
        format!("{s}.0")
    }
}

/// Jade's distance format, `DecimalFormat("#.#")`: `5.3`, `6` (no trailing `.0`).
fn distance_text(d: f64) -> String {
    let s = format!("{d:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Ticks as Jade's `seconds` shows them: `45s`, `2m 5s`, `2m`, `1h`, `1h 30m`.
pub fn duration(ticks: i64) -> String {
    let secs = ticks.max(0) / 20;
    if secs >= 3600 {
        let mins = (secs % 3600) / 60;
        if mins == 0 {
            format!("{}h", secs / 3600)
        } else {
            format!("{}h {mins}m", secs / 3600)
        }
    } else if secs >= 60 {
        let s = secs % 60;
        if s == 0 {
            format!("{}m", secs / 60)
        } else {
            format!("{}m {s}s", secs / 60)
        }
    } else {
        format!("{secs}s")
    }
}

/// The note block's note names, Jade's order (note 0 is F♯/G♭).
const NOTE_PITCH: [&str; 12] = ["F♯/G♭", "G", "G♯/A♭", "A", "A♯/B♭", "B", "C", "C♯/D♭", "D", "D♯/E♭", "E", "F"];
/// The octave colour of a note (dark scheme): note/12 picks white, yellow or gold.
const NOTE_OCTAVE: [u32; 3] = [0xFFFFFFFF, 0xFFFFFF55, 0xFFAA8800];
/// The instruments a note block head plays: not tuned to the note, so no pitch line.
const HEAD_INSTRUMENTS: &[&str] = &["zombie", "skeleton", "creeper", "dragon", "wither_skeleton", "piglin", "custom_head"];
/// Jade's coordinate digit colours (dark scheme): X red-ish, Y green-ish, Z blue-ish.
const COORD_COLORS: [u32; 3] = [0xFFEF9A9A, 0xFFA5D6A7, 0xFF90CAF9];

/// Ticks as `1:23`, the way the inventory shows an effect's time.
fn clock(ticks: i64) -> String {
    let secs = ticks.max(0) / 20;
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}

/// A block state's properties as `/setblock` writes them: `[(facing, east), (half, bottom)]`.
fn properties(host: &dyn ClientHost, state: BlockState) -> Vec<(String, String)> {
    let Some(text) = host.block_state_string(state) else { return Vec::new() };
    let Some(inner) = text.split_once('[').map(|(_, r)| r.trim_end_matches(']')) else { return Vec::new() };
    inner
        .split(',')
        .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
        .collect()
}

fn prop<'a>(props: &'a [(String, String)], key: &str) -> Option<&'a str> {
    props.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// The highest value a numeric property may take (`age` of wheat: 7).
fn prop_max(host: &dyn ClientHost, block: BlockKey, key: &str) -> Option<i64> {
    host.block_states()
        .definition(block)?
        .into_iter()
        .find(|p| p.name == key)?
        .values
        .iter()
        .filter_map(|v| v.parse::<i64>().ok())
        .max()
}

fn stack_of(host: &dyn ClientHost, v: &Json) -> Option<ItemStack> {
    let id = v["id"].as_str()?;
    let count = v["count"].as_u64().or_else(|| v["Count"].as_u64()).unwrap_or(1) as u32;
    stack(host, id, count)
}

fn item_name(host: &dyn ClientHost, s: &ItemStack) -> String {
    let one = ItemStack::new(s.item(), 1).unwrap_or_else(|| s.clone());
    host.hover_name(&one)
}

fn num(v: &Json, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|k| v[*k].as_i64().or_else(|| v[*k].as_f64().map(|f| f as i64)))
}

/// The plain text of a line, for narration.
pub fn line_text(line: &Line) -> String {
    line.els
        .iter()
        .filter_map(|e| match e {
            El::Text(t, _) | El::Italic(t, _, _) => Some(t.as_str()),
            _ => None,
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

fn mod_line(id: &str) -> Option<Line> {
    let mode = config::choice("mod_name");
    if mode == 2 {
        return None;
    }
    let name = client::mod_name(split_id(id).0);
    let scale = if mode == 1 { 0.75 } else { 1.0 };
    Some(Line { els: vec![El::Italic(name, MOD_BLUE, scale)], ..Line::default() })
}

/// Jade's `RegistryNameProvider.Mode` (`registry_name`): 0 off, 1 on, 2 with details (port-only),
/// 3 following F3+H. Jade puts the registry id right after the name (`TooltipPosition.HEAD + 100`).
fn id_line(host: &dyn ClientHost, id: &str, details: bool) -> Option<Line> {
    match config::choice("registry_name") {
        1 => Some(Line::text(id, GREY)),
        2 if details => Some(Line::text(id, GREY)),
        3 if host.gui_options().advanced_item_tooltips => Some(Line::text(id, GREY)),
        _ => None,
    }
}

fn mod_tail(id: &str, t: &mut Tooltip) {
    t.lines.extend(mod_line(id));
}

fn creative(host: &dyn ClientHost) -> bool {
    host.local_player()
        .and_then(|p| p.game_mode(host))
        .is_some_and(|m| matches!(m, GameMode::Creative | GameMode::Spectator))
}

// ---------------------------------------------------------------------------------------------
// Entry points

pub fn build(host: &dyn ClientHost, target: &Target, data: Option<&Json>, details: bool) -> Option<Tooltip> {
    match target {
        Target::Block { pos, state, face, hit } => block(host, *pos, *state, *face, *hit, data, details),
        Target::Entity { entity, id: _ } => entity_tip(host, *entity, data, details),
        Target::Fluid { pos, fluid } => fluid_tip(host, *pos, *fluid, details),
    }
}

/// The preview Jade's quick screen shows: a grass block.
pub fn preview(host: &dyn ClientHost) -> Tooltip {
    let mut t = Tooltip { icon: stack(host, "minecraft:grass_block", 1), ..Tooltip::default() };
    t.lines.push(Line::text(tr(host, "block.minecraft.grass_block", "Grass Block"), TITLE));
    if let Some(l) = id_line(host, "minecraft:grass_block", false) {
        t.lines.push(l);
    }
    mod_tail("minecraft:grass_block", &mut t);
    t
}

// ---------------------------------------------------------------------------------------------
// Blocks

// Jade's CropProgressProvider: `CropBlock` crops (wheat family) by age/max, then blocks of the
// bonemealable kind that carry one of the ladder properties `AGE_2 3 4 5 7 15` (a ladder block
// with any other age max — cave vines' 25, torchflower's 1 — shows nothing). The port can't test
// block classes, so the two sets are the vanilla members' ids.
const CROP_BLOCK_CROPS: &[&str] = &["minecraft:wheat", "minecraft:carrots", "minecraft:potatoes", "minecraft:beetroots"];
const BONEMEALABLE_CROPS: &[&str] = &[
    "minecraft:cocoa",
    "minecraft:sweet_berry_bush",
    "minecraft:nether_wart",
    "minecraft:pitcher_crop",
    "minecraft:mangrove_propagule",
    "minecraft:chorus_flower",
    "minecraft:melon_stem",
    "minecraft:pumpkin_stem",
    "minecraft:sugar_cane",
    "minecraft:cactus",
    "minecraft:bamboo",
];
const CROP_AGE_MAXES: [i64; 6] = [2, 3, 4, 5, 7, 15];

/// Jade's crop growth line: `Growth: 57%`, or `Mature` in green.
fn crop_growth(host: &dyn ClientHost, t: &mut Tooltip, id: &str, key: BlockKey, props: &[(String, String)]) {
    let is_crop = CROP_BLOCK_CROPS.contains(&id)
        || (BONEMEALABLE_CROPS.contains(&id)
            && prop_max(host, key, "age").is_some_and(|max| CROP_AGE_MAXES.contains(&max)));
    if !is_crop {
        return;
    }
    let Some(age) = prop(props, "age").and_then(|v| v.parse::<i64>().ok()) else { return };
    let Some(max) = prop_max(host, key, "age").filter(|max| *max > 0) else { return };
    let label = tr(host, "jade.crop_growth", "Growth");
    if age >= max {
        t.lines.push(Line::pair(&label, tr(host, "jade.crop_mature", "Mature"), SUCCESS).details());
    } else {
        // Jade prints `%.0f` of the growth value: rounds, not truncates.
        t.lines.push(Line::pair(&label, format!("{:.0}%", age as f32 / max as f32 * 100.0), INFO).details());
    }
}

fn block(host: &dyn ClientHost, pos: [i32; 3], state: BlockState, face: [i32; 3], hit: [f64; 3], data: Option<&Json>, details: bool) -> Option<Tooltip> {
    let key = state.block();
    let id = key.id(host);
    if id == "minecraft:air" || id == "minecraft:cave_air" || id == "minecraft:void_air" {
        return None;
    }
    let (ns, path) = split_id(&id);
    let props = properties(host, state);
    let be = data.map(|d| &d["be"]).filter(|v| v.is_object());
    let icon_item = host.block_item(key).filter(|i| i.id(host) != "minecraft:air");
    let icon = icon_item.and_then(|i| ItemStack::new(i, 1));
    // Jade's ObjectNameProvider.ForBlock: the block's translation, else the pick item's name,
    // else the literal translation key.
    let mut name = host
        .translate(&format!("block.{ns}.{path}"))
        .or_else(|| icon.as_ref().map(|s| item_name(host, s)))
        .unwrap_or_else(|| format!("block.{ns}.{path}"));
    if let Some(custom) = data.and_then(|d| d["name"].as_str()) {
        name = custom.to_owned();
    }
    // A player head names itself after its owner (Jade's PlayerHeadProvider) when it has no
    // custom name; the owner's name comes from the server's block entity data.
    if let Some(owner) = be
        .and_then(|b| b["SkullOwner"].get("Name").or_else(|| b["skull_owner"].get("Name")))
        .and_then(|v| v.as_str())
        .filter(|_| id.ends_with("_head") || id == "minecraft:skull")
        .filter(|_| !data.map(|d| d["name"].as_str()).unwrap_or_default().is_some())
    {
        let base = tr(host, "block.minecraft.player_head.named", "%s's Head");
        name = base.replace("%1$s", owner).replace("%s", owner).replace("{0}", owner);
    }
    // Spawners: "<Mob> (Monster Spawner)".
    if config::on("mob_spawner")
        && (id == "minecraft:spawner" || id == "minecraft:trial_spawner")
        && let Some(mob) = be
            .and_then(|b| b["SpawnData"]["entity"]["id"].as_str().or_else(|| b["spawn_data"]["entity"]["id"].as_str()))
    {
        let (mns, mpath) = split_id(mob);
        let mob_name = tr(host, &format!("entity.{mns}.{mpath}"), &title_case(mpath));
        name = trf(host, "jade.spawner", "%s (%s)", &[&mob_name, &name]);
    }
    if name.chars().count() > 100 {
        name = name.chars().take(100).collect();
    }
    let mut t = Tooltip { icon, ..Tooltip::default() };
    let mut name_line = Line::new();
    if config::on("object_name") {
        name_line.push(El::Text(name, TITLE));
        if config::on("block_face") {
            let f = match face {
                [0, -1, 0] => "down",
                [0, 1, 0] => "up",
                [0, 0, -1] => "north",
                [0, 0, 1] => "south",
                [-1, 0, 0] => "west",
                _ => "east",
            };
            name_line.push(El::Text(format!(" ({})", tr(host, &format!("jade.{f}"), &title_case(f))), NORMAL));
        }
    }
    // Harvest tool: icons, then a check or a cross.
    let harvest = harvest_els(host, key, state);
    let harvest_new_line = config::on("harvest_tool.new_line");
    if !harvest.is_empty() && !harvest_new_line {
        name_line.push(El::Space(6.0));
        name_line.els.extend(harvest.iter().cloned());
    }
    if !name_line.els.is_empty() {
        t.lines.push(name_line);
    }
    if !harvest.is_empty() && harvest_new_line {
        t.lines.push(Line { els: harvest, ..Line::default() });
    }
    // The registry id sits right after the name (Jade `HEAD + 100`).
    if let Some(l) = id_line(host, &id, details) {
        t.lines.push(l);
    }
    // Breaking progress.
    if config::on("breaking_progress")
        && let Some(player) = host.local_player()
        && let Some(BlockBreakingState::Breaking { position, progress, .. }) = player.block_breaking(host)
        && [position.x, position.y, position.z] == pos
    {
        let ok = can_harvest(host, state);
        t.progress = Some((progress, if ok { TITLE } else { FAILURE }));
    }
    distance_and_coords(host, &mut t, [pos[0] as f64, pos[1] as f64, pos[2] as f64], hit);
    // Block states, as Jade's debug provider: every property, booleans coloured on the value.
    if config::on("block_states") && !props.is_empty() {
        for (k, v) in &props {
            let color = if v == "true" {
                SUCCESS
            } else if v == "false" {
                DANGER
            } else {
                NORMAL
            };
            t.lines.push(Line {
                els: vec![El::Text(format!("{k}:"), NORMAL), El::Text(format!(" {v}"), color)],
                ..Line::default()
            }
            .details());
        }
    }
    // The body providers, in Jade's line order (registration order inside one priority):
    // brewing stand, TNT, beehive, note block, command block, furnace, redstone, crop growth,
    // jukebox, lectern, block properties, storage.
    if let Some(be) = be {
        brewing_line(host, &mut t, &id, be);
    }
    if config::on("tnt_stability") && id == "minecraft:tnt" && prop(&props, "unstable") == Some("true") {
        t.lines.push(Line::text(tr(host, "jade.tnt.unstable", "Unstable"), DANGER).details());
    }
    if config::on("beehive") && (id == "minecraft:beehive" || id == "minecraft:bee_nest") {
        if let Some(honey) = prop(&props, "honey_level").and_then(|v| v.parse::<i64>().ok()) {
            t.lines.push(Line::pair(&tr(host, "jade.beehive.honey", "Honey"), format!("{honey} / 5"), if honey >= 5 { SUCCESS } else { INFO }).details());
        }
        if let Some(bees) = be.and_then(|b| b["bees"].as_array().or_else(|| b["Bees"].as_array())) {
            let n = bees.len() as i64;
            t.lines.push(Line::pair(&tr(host, "jade.beehive.bees", "Bees"), n.to_string(), if n >= 3 { SUCCESS } else { INFO }).details());
        }
    }
    if config::on("note_block")
        && id == "minecraft:note_block"
        && let Some(inst) = prop(&props, "instrument")
    {
        let inst_name = tr(host, &format!("jade.instrument.{inst}"), &title_case(inst));
        if HEAD_INSTRUMENTS.contains(&inst) {
            t.lines.push(Line::text(inst_name, NORMAL).details());
        } else if let Some(note) = prop(&props, "note").and_then(|n| n.parse::<usize>().ok()) {
            let pitch = NOTE_PITCH[note % NOTE_PITCH.len()];
            let octave = NOTE_OCTAVE[(note / NOTE_PITCH.len()).min(2)];
            t.lines.push(Line {
                els: vec![El::Text(format!("{inst_name} "), NORMAL), El::Text(pitch.to_owned(), octave)],
                ..Line::default()
            }
            .details());
        } else {
            t.lines.push(Line::text(inst_name, NORMAL).details());
        }
    }
    if config::on("command_block")
        && (id == "minecraft:command_block" || id == "minecraft:chain_command_block" || id == "minecraft:repeating_command_block")
        && let Some(cmd) = be.and_then(|b| b["Command"].as_str().or_else(|| b["command"].as_str())).filter(|c| !c.is_empty())
    {
        let cmd: String = if cmd.chars().count() > 40 {
            format!("{}...", cmd.chars().take(37).collect::<String>())
        } else {
            cmd.to_owned()
        };
        t.lines.push(Line::text(format!("> {cmd}"), NORMAL).details());
    }
    if let Some(be) = be {
        furnace_line(host, &mut t, &id, be);
    }
    redstone(host, &mut t, &id, &props, be, data);
    if config::on("crop_progress") {
        crop_growth(host, &mut t, &id, key, &props);
    }
    if config::on("jukebox")
        && id == "minecraft:jukebox"
        && let Some(be) = be
    {
        jukebox(host, &mut t, be);
    }
    if config::on("lectern")
        && id == "minecraft:lectern"
        && let Some(be) = be
        && let Some(book) = stack_of(host, &be["Book"])
    {
        let title = be["Book"]["components"]["minecraft:written_book_content"]["title"]["raw"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| item_name(host, &book));
        let mut line = Line::new();
        line.push(El::Item(book, 12.0, None));
        line.push(El::Text(format!(" {title}"), NORMAL));
        t.lines.push(line.details());
    }
    // Block properties, Jade's debug provider: destroy time, then explosion resistance.
    if config::on("block_properties") {
        let bs = host.block_states();
        if let Some(d) = bs.destroy_speed(state) {
            t.lines.push(Line::text(trf(host, "jade.block_destroy_time", "Destroy time: %s", &[&java_float(d as f64)]), INFO).details());
        }
        if let Some(r) = bs.explosion_resistance(state) {
            t.lines.push(Line::text(trf(host, "jade.block_explosion_resistance", "Explosion resistance: %s", &[&java_float(r as f64)]), INFO).details());
        }
    }
    // The storage group: campfire, a container's contents, or the player's ender chest.
    match be {
        Some(be) if id == "minecraft:campfire" || id == "minecraft:soul_campfire" => {
            if config::on("campfire") {
                campfire(host, &mut t, be);
            }
        }
        Some(be) => storage_line(host, &mut t, &id, be, data, details),
        None => {
            if let Some(d) = data
                && d["ender"].is_array()
            {
                storage(host, &mut t, d["ender"].as_array().map(Vec::as_slice).unwrap_or(&[]), details);
            }
        }
    }
    // Jade draws the unbreakable text from the harvest tool provider (TAIL - 2000: after the
    // storage, before the mod name), behind the harvest tool's own gates.
    if config::on("harvest_tool") && config::on("harvest_tool.show_unbreakable") && host.block_states().destroy_speed(state).is_some_and(|s| s < 0.0) {
        let hidden = creative(host) && !config::on("harvest_tool.creative");
        if !hidden {
            t.lines.push(Line::text(tr(host, "jade.harvest_tool.unbreakable", "Unbreakable"), DANGER));
        }
    }
    mod_tail(&id, &mut t);
    Some(t)
}

/// Jade's `DistanceProvider` line: coloured coordinates, the distance after them in brackets, or
/// `Distance: x` alone. Its position is after health, before the body (`-4600`).
fn distance_and_coords(host: &dyn ClientHost, t: &mut Tooltip, at: [f64; 3], hit: [f64; 3]) {
    let distance = if config::on("distance") {
        host.camera().map(|cam| {
            ((cam.position[0] - hit[0]).powi(2) + (cam.position[1] - hit[1]).powi(2) + (cam.position[2] - hit[2]).powi(2)).sqrt()
        })
    } else {
        None
    };
    if config::on("coordinates") {
        let coords = if config::on("coordinates.rel") && host.is_key_down("key.keyboard.left_control") {
            let eye = host.camera().map(|c| c.position).unwrap_or([0.0; 3]);
            [at[0] - eye[0], at[1] - eye[1], at[2] - eye[2]].map(|v| v.floor() as i64)
        } else {
            at.map(|v| v.floor() as i64)
        };
        let mut line = Line::new();
        for (i, n) in coords.iter().enumerate() {
            if i > 0 {
                line.push(El::Text(" ".to_owned(), NORMAL));
            }
            line.push(El::Text(n.to_string(), COORD_COLORS[i]));
        }
        if let Some(d) = distance {
            line.push(El::Text(format!(" [{}]", distance_text(d)), NORMAL));
        }
        t.lines.push(line.details());
    } else if let Some(d) = distance {
        t.lines.push(Line::text(trf(host, "jade.distance2", "Distance: %s", &[&distance_text(d)]), NORMAL).details());
    }
}

fn redstone(host: &dyn ClientHost, t: &mut Tooltip, id: &str, props: &[(String, String)], be: Option<&Json>, data: Option<&Json>) {
    if !config::on("redstone") {
        return;
    }
    match id {
        "minecraft:lever" => {
            let on = prop(props, "powered") == Some("true");
            let label = tr(host, "jade.lever.state", "State");
            let value = if on { tr(host, "jade.on", "On") } else { tr(host, "jade.off", "Off") };
            t.lines.push(Line::pair(&label, value, if on { SUCCESS } else { DANGER }).details());
        }
        "minecraft:repeater" => {
            // Jade's repeater branch shows the delay and returns: no locked line.
            if let Some(d) = prop(props, "delay") {
                t.lines.push(Line::pair(&tr(host, "jade.repeater.delay", "Delay"), d.to_owned(), INFO).details());
            }
        }
        "minecraft:comparator" => {
            let subtract = prop(props, "mode") == Some("subtract");
            let mode = if subtract { tr(host, "jade.comparator.subtractor", "Subtractor") } else { tr(host, "jade.comparator.comparator", "Comparator") };
            t.lines.push(Line::pair(&tr(host, "jade.comparator.mode", "Mode"), mode, INFO).details());
            if let Some(power) = be.and_then(|b| num(b, &["OutputSignal", "output_signal"])) {
                t.lines.push(Line::pair(&tr(host, "jade.power", "Power"), power.to_string(), INFO).details());
            }
        }
        "minecraft:calibrated_sculk_sensor" => {
            // Jade streams the signal of the block behind the sensor's back.
            if let Some(input) = data.and_then(|d| d["input_signal"].as_i64()) {
                t.lines.push(Line::text(trf(host, "jade.input_signal", "Input: %s", &[&input.to_string()]), INFO).details());
            }
            if let Some(power) = prop(props, "power") {
                t.lines.push(Line::pair(&tr(host, "jade.power", "Power"), power.to_owned(), INFO).details());
            }
        }
        _ => {
            if let Some(power) = prop(props, "power") {
                t.lines.push(Line::pair(&tr(host, "jade.power", "Power"), power.to_owned(), INFO).details());
            }
        }
    }
}

const FURNACES: &[&str] = &["minecraft:furnace", "minecraft:smoker", "minecraft:blast_furnace"];

/// Jade's first body line for furnaces-and-friends (registration order): input, fuel, the
/// progress arrow, output.
fn furnace_line(host: &dyn ClientHost, t: &mut Tooltip, id: &str, be: &Json) {
    if !FURNACES.contains(&id) || !config::on("furnace") {
        return;
    }
    let items: &[Json] = be["Items"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let slot = |n: i64| items.iter().find(|s| s["Slot"].as_i64() == Some(n)).and_then(|s| stack_of(host, s));
    let (input, fuel, out) = (slot(0), slot(1), slot(2));
    if input.is_none() && fuel.is_none() && out.is_none() {
        return;
    }
    let spent = num(be, &["cooking_time_spent", "CookTime"]).unwrap_or(0);
    let total = num(be, &["cooking_total_time", "CookTimeTotal"]).unwrap_or(0);
    let ratio = if total > 0 { spent as f32 / total as f32 } else { 0.0 };
    let mut line = Line::new();
    for s in [input, fuel] {
        match s {
            Some(s) => {
                let c = (s.count() > 1).then(|| s.count().to_string());
                line.push(El::Item(s, 16.0, c));
            }
            None => {
                line.push(El::Space(16.0));
            }
        }
        line.push(El::Space(2.0));
    }
    line.push(El::Space(2.0));
    line.push(El::Arrow(ratio));
    line.push(El::Space(4.0));
    if let Some(s) = out {
        let c = (s.count() > 1).then(|| s.count().to_string());
        line.push(El::Item(s, 16.0, c));
    }
    t.lines.push(line.details());
}

/// Jade's very first body provider: the brewing stand's blaze powder, fuel and brew time.
fn brewing_line(host: &dyn ClientHost, t: &mut Tooltip, id: &str, be: &Json) {
    if id != "minecraft:brewing_stand" || !config::on("brewing_stand") {
        return;
    }
    let fuel = num(be, &["Fuel", "fuel"]).unwrap_or(0);
    let time = num(be, &["BrewTime", "brew_time"]).unwrap_or(0);
    let mut line = Line::new();
    if let Some(s) = stack(host, "minecraft:blaze_powder", 1) {
        line.push(El::Item(s, 12.0, None));
    }
    line.push(El::Text(format!(" {fuel}"), INFO));
    if time > 0 {
        line.push(El::Space(6.0));
        if let Some(s) = stack(host, "minecraft:clock", 1) {
            line.push(El::Item(s, 12.0, None));
        }
        line.push(El::Text(format!(" {}", duration(time)), INFO));
    }
    t.lines.push(line.details());
}

/// Jade's jukebox line: the song's description, or nothing when no record sits inside.
fn jukebox(host: &dyn ClientHost, t: &mut Tooltip, be: &Json) {
    let record = &be["RecordItem"];
    let record = if record.is_object() { record } else { &be["record_item"] };
    if let Some(disc) = record["id"].as_str() {
        let song = disc.rsplit(':').next().unwrap_or(disc).strip_prefix("music_disc_").unwrap_or("");
        let (ns, _) = split_id(disc);
        let fallback = stack(host, disc, 1).map(|s| item_name(host, &s)).unwrap_or_else(|| title_case(song));
        let title = tr(host, &format!("jukebox_song.{ns}.{song}.desc"), &fallback);
        t.lines.push(Line::text(trf(host, "record.nowPlaying", "Now Playing: %s", &[&title]), NORMAL).details());
    }
}

/// Jade renders campfire items through the item-storage pipeline, each item's time left in the
/// slot-count corner.
fn campfire(host: &dyn ClientHost, t: &mut Tooltip, be: &Json) {
    let items: &[Json] = be["Items"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let times = be["CookingTimes"].as_array().or_else(|| be["cooking_times"].as_array());
    let totals = be["CookingTotalTimes"].as_array().or_else(|| be["cooking_total_times"].as_array());
    let mut line = Line::new();
    for s in items {
        let Some(stack) = stack_of(host, s) else { continue };
        let n = s["Slot"].as_u64().unwrap_or(0) as usize;
        let left = match (times.and_then(|a| a.get(n)?.as_i64()), totals.and_then(|a| a.get(n)?.as_i64())) {
            (Some(done), Some(total)) => Some(duration(total - done)),
            _ => None,
        };
        line.push(El::Item(stack, 16.0, left));
        line.push(El::Space(2.0));
    }
    if !line.els.is_empty() {
        t.lines.push(line.details());
    }
}

/// The storage group: loot and lock flags, then the contents. Furnaces never reach here (Jade's
/// item storage skips them; the furnace line carries their items).
fn storage_line(host: &dyn ClientHost, t: &mut Tooltip, id: &str, be: &Json, data: Option<&Json>, details: bool) {
    if FURNACES.contains(&id) {
        return;
    }
    let items: &[Json] = be["Items"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let hopper_disabled = id == "minecraft:hopper" && data.is_some_and(|d| d["hopper_disabled"].as_bool() == Some(true));
    // Jade's HopperLockProvider renames the title "Locked {name}" when the hopper is off: the
    // name element in the first line, the face text and harvest icons kept.
    if hopper_disabled && config::on("redstone") && config::on("hopper_lock") {
        if let Some(line) = t.lines.first_mut() {
            let locked = line
                .els
                .iter()
                .find_map(|el| match el {
                    El::Text(name, _) => Some({
                        let base = tr(host, "jade.access.block.locked", "Locked {0}");
                        base.replace("{0}", name).replace("%1$s", name).replace("%s", name)
                    }),
                    _ => None,
                });
            if let Some(renamed) = locked {
                for el in &mut line.els {
                    if let El::Text(_, _) = el {
                        *el = El::Text(renamed, TITLE);
                        break;
                    }
                }
            }
        }
    }
    if !config::on("item_storage") {
        return;
    }
    if be.get("LootTable").is_some() || be.get("loot_table").is_some() {
        t.lines.push(Line { els: vec![El::Italic(tr(host, "jade.loot_not_generated", "What could it be?"), NORMAL, 1.0)], ..Line::default() }.details());
        return;
    }
    // Jade only flags the lock for a viewer that can't open it anyway; the server half can't read
    // the viewer's game mode, so the flag reaches creative players too (API limit).
    if data.is_some_and(|d| d["locked"].as_bool() == Some(true)) {
        t.lines.push(Line::text(tr(host, "jade.locked", "Inventory is locked"), NORMAL).details());
        return;
    }
    if let Some(ender) = data.and_then(|d| d["ender"].as_array()) {
        storage(host, t, ender, details);
    } else if !items.is_empty() {
        storage(host, t, items, details);
    } else if let Some(one) = be.get("item").filter(|v| v.is_object()) {
        storage(host, t, std::slice::from_ref(one), details);
    }
}

/// A container's contents: a grid of icons, or rows with names while few kinds are inside
/// (Jade's item storage: at most 54 kinds, sorted by count when the option says so).
fn storage(host: &dyn ClientHost, t: &mut Tooltip, items: &[Json], details: bool) {
    let mut merged: Vec<(ItemStack, u64)> = Vec::new();
    for s in items {
        let Some(stack) = stack_of(host, s) else { continue };
        let n = stack.count() as u64;
        match merged.iter_mut().find(|(m, _)| m.item() == stack.item()) {
            Some((_, c)) => *c += n,
            None => merged.push((stack, n)),
        }
    }
    if merged.is_empty() {
        return;
    }
    if merged.len() > 54 {
        // Jade's ItemCollector.MAX_SIZE.
        merged.truncate(54);
    }
    if config::on("item_storage.sort") {
        merged.sort_by(|a, b| b.1.cmp(&a.1));
    }
    let limit = config::num(if details { "item_storage.detailed_amount" } else { "item_storage.normal_amount" }) as usize;
    if limit == 0 {
        return;
    }
    let shown = &merged[..merged.len().min(limit)];
    // Named rows while strictly fewer kinds than the threshold are inside (Jade turns to the grid
    // the moment the count reaches `show_name_amount`).
    if merged.len() < config::num("item_storage.show_name_amount") as usize {
        for (s, n) in shown {
            let mut line = Line::new();
            line.push(El::Item(s.clone(), 12.0, None));
            line.push(El::Space(3.0));
            line.push(El::Text(format!("{}× ", short_count(*n)), NORMAL));
            line.push(El::Text(item_name(host, s), NORMAL));
            t.lines.push(line.details());
        }
        return;
    }
    let per = (config::num("item_storage.items_per_line") as usize).max(1);
    for row in shown.chunks(per) {
        let mut line = Line::new();
        for (s, n) in row {
            line.push(El::Item(s.clone(), 16.0, (*n > 1).then(|| short_count(*n))));
            line.push(El::Space(2.0));
        }
        t.lines.push(line.details());
    }
}

/// The tool icons for a block and whether the held item harvests it (Jade's HarvestToolProvider
/// and its tool types: pickaxe, axe, shovel, hoe, sword, shears, in that order).
fn harvest_els(host: &dyn ClientHost, block: BlockKey, state: BlockState) -> Vec<El> {
    if !config::on("harvest_tool") || (creative(host) && !config::on("harvest_tool.creative")) {
        return Vec::new();
    }
    if host.block_states().destroy_speed(state).is_some_and(|s| s < 0.0) {
        return Vec::new();
    }
    let id = block.id(host);
    let requires = host.block_states().requires_correct_tool(state).unwrap_or(false);
    let destroy_speed = host.block_states().destroy_speed(state).unwrap_or(0.0);
    // Jade's tool tiers: the vanilla tag picks the tier whose tool mines for drops (gold mines at
    // wood level), so `needs_iron_tool` shows the iron icon and so on.
    let tier = if host.block_in_tag(block, "minecraft:needs_diamond_tool") == Some(true) {
        "diamond"
    } else if host.block_in_tag(block, "minecraft:needs_iron_tool") == Some(true) {
        "iron"
    } else if host.block_in_tag(block, "minecraft:needs_stone_tool") == Some(true) {
        "stone"
    } else {
        "wooden"
    };
    let mut tools: Vec<String> = Vec::new();
    // The four tiered kinds skip the icon on a block that breaks instantly by any hand, unless it
    // insists on the right tool (Jade's `skipInstaBreakingBlock`).
    for kind in ["pickaxe", "axe", "shovel", "hoe"] {
        if host.block_in_tag(block, &format!("minecraft:mineable/{kind}")) == Some(true)
            && (requires || destroy_speed != 0.0)
        {
            tools.push(format!("minecraft:{tier}_{kind}"));
        }
    }
    // Jade's shears tier: the wool and leaves tags, the blocks shears collect, and the tripwire.
    if host.block_in_tag(block, "minecraft:leaves") == Some(true)
        || host.block_in_tag(block, "minecraft:wool") == Some(true)
        || matches!(id.as_str(), "minecraft:cobweb" | "minecraft:vine" | "minecraft:glow_lichen" | "minecraft:dead_bush" | "minecraft:short_grass" | "minecraft:tall_grass" | "minecraft:fern" | "minecraft:seagrass" | "minecraft:hanging_roots" | "minecraft:twisting_vines" | "minecraft:weeping_vines" | "minecraft:tripwire")
    {
        tools.push("minecraft:shears".into());
    }
    if host.block_in_tag(block, "minecraft:sword_efficient") == Some(true) || id == "minecraft:cobweb" || id == "minecraft:bamboo" {
        tools.push("minecraft:wooden_sword".into());
    }
    let mut out = Vec::new();
    if !tools.is_empty() && (requires || config::on("harvest_tool.effective_tool")) {
        for tool in &tools {
            if let Some(s) = stack(host, tool, 1) {
                out.push(El::Item(s, 12.0, None));
            }
        }
    }
    if out.is_empty() {
        return Vec::new();
    }
    let can_harvest = held(host).and_then(|s| host.block_states().is_correct_tool_for_drops(&s, state)).unwrap_or(false);
    // Jade's mark: always when the block insists on the right tool (check or cross by the held
    // item), else only when the held item won't get the drop (a cross).
    if requires || !can_harvest {
        out.push(El::Space(1.0));
        out.push(El::Check(can_harvest));
    }
    out
}

fn held(host: &dyn ClientHost) -> Option<ItemStack> {
    let player = host.local_player()?;
    host.living(player.entity())?.hand_item(host, Hand::Main)
}

fn can_harvest(host: &dyn ClientHost, state: BlockState) -> bool {
    let requires = host.block_states().requires_correct_tool(state).unwrap_or(false);
    !requires || held(host).and_then(|s| host.block_states().is_correct_tool_for_drops(&s, state)).unwrap_or(false)
}

// ---------------------------------------------------------------------------------------------
// Entities

const HORSES: &[&str] = &[
    "minecraft:horse", "minecraft:donkey", "minecraft:mule", "minecraft:skeleton_horse", "minecraft:zombie_horse",
    "minecraft:llama", "minecraft:trader_llama", "minecraft:camel",
];

fn entity_tip(host: &dyn ClientHost, entity: EntityRef, data: Option<&Json>, details: bool) -> Option<Tooltip> {
    let kind = entity.kind(host).map(|k| k.id(host)).or_else(|| data.and_then(|d| d["type"].as_str()).map(str::to_owned));
    let nbt = data.map(|d| &d["nbt"]).filter(|v| v.is_object());
    let kind_id = kind.clone().unwrap_or_else(|| "minecraft:unknown".into());
    let (ns, path) = split_id(&kind_id);
    let is_boss = matches!(kind_id.as_str(), "minecraft:ender_dragon" | "minecraft:wither");
    if is_boss && !config::on("display_bosses") {
        return None;
    }
    let mut name = entity
        .name(host)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| tr(host, &format!("entity.{ns}.{path}"), &title_case(path)));
    let mut t = Tooltip::default();
    // The icon: the item the entity is (a dropped item, a falling block, a display entity).
    let dropped = (kind_id == "minecraft:item").then(|| nbt.and_then(|n| stack_of(host, &n["Item"]))).flatten();
    if config::on("item_entity")
        && let Some(s) = &dropped
    {
        name = item_name(host, s);
        t.icon = Some(s.clone());
    } else if let Some(s) = nbt
        .and_then(|n| n["BlockState"]["Name"].as_str())
        .and_then(|block_id| stack(host, block_id, 1))
        .filter(|_| kind_id == "minecraft:falling_block")
    {
        // Jade's FallingBlockProvider: the falling block's item is the icon.
        t.icon = Some(s);
    } else if let Some(s) = nbt.and_then(|n| stack_of(host, &n["item"])).filter(|_| kind_id == "minecraft:item_display") {
        // Jade's ItemDisplayProvider: the displayed item is the icon.
        t.icon = Some(s);
    } else if kind_id == "minecraft:block_display" {
        // Jade's BlockDisplayProvider: the displayed block's item, and its name when the type has
        // no custom name.
        if let Some(block_id) = nbt.and_then(|n| n["block_state"]["Name"].as_str())
            && let Some(s) = stack(host, block_id, 1)
        {
            t.icon = Some(s);
            let (bns, bpath) = split_id(block_id);
            name = tr(host, &format!("block.{bns}.{bpath}"), &title_case(bpath));
        }
    } else if kind.is_some() {
        t.icon = stack(host, &kind_id, 1);
    }
    // Jade's MobSpawnerProvider.ForEntity: "<Mob> (Minecart with Spawner)".
    if config::on("mob_spawner")
        && kind_id == "minecraft:spawner_minecart"
        && let Some(mob) = nbt.and_then(|n| n["SpawnData"]["entity"]["id"].as_str())
    {
        let (mns, mpath) = split_id(mob);
        let mob_name = tr(host, &format!("entity.{mns}.{mpath}"), &title_case(mpath));
        name = trf(host, "jade.spawner", "%s (%s)", &[&mob_name, &name]);
    }
    if config::on("object_name") {
        t.lines.push(Line::text(name, TITLE));
    }
    if let Some(l) = id_line(host, &kind_id, details) {
        t.lines.push(l);
    }
    if let Some(d) = data {
        health(host, &mut t, d);
    }
    if let Some(at) = entity.position(host) {
        distance_and_coords(host, &mut t, at, at);
    }
    if let Some(n) = nbt {
        // The body providers, in Jade's line order: horse stats, item frame, effects, growth,
        // breeding, armor stand, painting, next drop, profession, owner, zombie conversion.
        entity_lines(host, &mut t, &kind_id, n, data, details);
    }
    if config::on("animal_owner")
        && let Some(owner) = data.and_then(|d| d["owner"].as_str())
    {
        // Jade sends nothing when the owner's name can't be resolved: no line.
        t.lines.push(Line::pair(&tr(host, "jade.owner", "Owner"), owner.to_owned(), INFO).details());
    }
    if config::on("zombie_villager")
        && kind_id == "minecraft:zombie_villager"
        && let Some(c) = nbt.and_then(|n| n["ConversionTime"].as_i64()).filter(|c| *c > 0)
    {
        t.lines.push(Line::pair(&tr(host, "jade.zombie_conversion", "Conversion time"), duration(c), INFO).details());
    }
    if config::on("item_storage")
        && let Some(n) = nbt
        && kind_id.ends_with("minecart")
    {
        if n.get("LootTable").is_some() {
            t.lines.push(Line { els: vec![El::Italic(tr(host, "jade.loot_not_generated", "What could it be?"), NORMAL, 1.0)], ..Line::default() }.details());
        } else if let Some(items) = n["Items"].as_array() {
            storage(host, &mut t, items, details);
        }
    }
    mod_tail(&kind_id, &mut t);
    Some(t)
}

/// Jade's `EntityHealthAndArmorProvider` with its `HealthElement` and `ArmorElement`: hearts in
/// rows of `icons_per_line`, a `hp/max` number above `max_for_render`, armour icons turning into a
/// number above its own threshold. Health and armour share a line when both show numbers.
fn health(_host: &dyn ClientHost, t: &mut Tooltip, d: &Json) {
    let (Some(hp), Some(max)) = (d["health"].as_f64(), d["max_health"].as_f64()) else { return };
    let abs = d["absorption"].as_f64().unwrap_or(0.0);
    let show_fractions = config::on("entity_health.show_fractions");
    let mut health_els: Option<Vec<El>> = None;
    let mut health_text = false;
    if config::on("entity_health") {
        // Jade counts ceil(max) + ceil(absorption) points against the limit.
        let points = max.ceil() + abs.ceil();
        let limit = config::num("entity_health.max_for_render");
        if points > limit {
            let mut shown_hp = hp + abs;
            let mut shown_max = max;
            if !show_fractions {
                shown_hp = shown_hp.ceil();
                shown_max = shown_max.ceil();
            }
            let mut line = Line::new();
            line.push(El::Sprite("hud/heart/full", 9, 9));
            line.push(El::Space(2.0));
            line.push(El::Text(format!("{}/{}", df_down(shown_hp, 2), df_down(shown_max, 2)), INFO));
            health_text = true;
            health_els = Some(line.els);
        } else {
            let per = (config::num("entity_health.icons_per_line") as usize).max(1);
            // Jade: ceil(max * 0.5) hearts of health, the rest absorption, the row count from
            // ceil((ceil(max) + ceil(abs)) * 0.5) hearts in total; the shown health is rounded.
            // `cur` counts the filled hearts from the row's left edge (hearts = points / 2).
            let health_hearts = (max * 0.5).ceil() as usize;
            let total_hearts = (((max.ceil() + abs.ceil()) * 0.5).ceil() as usize).max(1);
            let cur_h = (hp.round() * 0.5).min(health_hearts as f64);
            let cur_a = (max.ceil() + abs) * 0.5;
            let mut line = Line::new();
            let mut n = 0;
            let mut push_heart = |line: &mut Line, fill: Option<&'static str>| {
                if n > 0 && n % per == 0 {
                    let done = std::mem::take(line);
                    t.lines.push(done);
                }
                line.push(El::Sprite("hud/heart/container", 9, 9));
                line.push(El::Space(-9.0));
                if let Some(f) = fill {
                    line.push(El::Sprite(f, 9, 9));
                    line.push(El::Space(-9.0));
                }
                line.push(El::Space(8.0));
                n += 1;
            };
            for i in 0..total_hearts {
                let (fill, cur) = if i >= health_hearts {
                    (Some(("hud/heart/absorbing_full", "hud/heart/absorbing_half")), cur_a)
                } else {
                    (Some(("hud/heart/full", "hud/heart/half")), cur_h)
                };
                let k = i as f64;
                let sprite = if k < cur.floor() {
                    fill.map(|f| f.0)
                } else if k < cur {
                    fill.map(|f| f.1)
                } else {
                    None
                };
                push_heart(&mut line, sprite);
            }
            if !line.els.is_empty() {
                line.push(El::Space(1.0));
                health_els = Some(line.els);
            }
        }
    }
    let mut armor_els: Option<Vec<El>> = None;
    let mut armor_text = false;
    if config::on("entity_armor")
        && let Some(armor) = d["armor"].as_f64()
        && armor > 0.0
    {
        let limit = config::num("entity_armor.max_for_render");
        if armor > limit {
            let shown = if show_fractions { armor } else { armor.ceil() };
            let mut line = Line::new();
            line.push(El::Sprite("hud/armor_full", 9, 9));
            line.push(El::Space(2.0));
            line.push(El::Text(df_down(shown, 2), INFO));
            armor_text = true;
            armor_els = Some(line.els);
        } else {
            let per = (config::num("entity_health.icons_per_line") as usize).max(1);
            // Jade draws ceil(armor / 2) icons: no row of empty shells.
            let icons = (armor * 0.5).ceil() as usize;
            let mut line = Line::new();
            let mut n = 0;
            for i in 0..icons {
                if n > 0 && n % per == 0 {
                    let done = std::mem::take(&mut line);
                    t.lines.push(done);
                }
                let v = armor - (i as f64) * 2.0;
                let s = if v >= 2.0 {
                    "hud/armor_full"
                } else if v > 0.0 {
                    "hud/armor_half"
                } else {
                    "hud/armor_empty"
                };
                line.push(El::Sprite(s, 9, 9));
                line.push(El::Space(-1.0));
                n += 1;
            }
            if !line.els.is_empty() {
                line.push(El::Space(1.0));
                armor_els = Some(line.els);
            }
        }
    }
    match (health_els, armor_els) {
        (Some(mut h), Some(mut a)) if health_text && armor_text => {
            // Jade joins the two numbers on one line with a 4-pixel gap.
            h.push(El::Space(4.0));
            h.append(&mut a);
            t.lines.push(Line { els: h, ..Line::default() });
        }
        (Some(h), _) => t.lines.push(Line { els: h, ..Line::default() }),
        (None, Some(a)) => t.lines.push(Line { els: a, ..Line::default() }),
        (None, None) => {}
    }
}

/// The entity's body providers, in Jade's line order: horse stats, item frame, mob effects, mob
/// growth, breeding, armour stand, painting, next drop, villager profession.
fn entity_lines(host: &dyn ClientHost, t: &mut Tooltip, kind: &str, n: &Json, data: Option<&Json>, details: bool) {
    // A mob's growing age; a dropped item's or an orb's `Age` is its time on the ground.
    let age = data.and_then(|d| d.get("health")).and(n["Age"].as_i64());
    if config::on("horse_stats") && HORSES.contains(&kind) {
        horse_stats(host, t, kind, data, n, details);
    }
    if config::on("item_frame")
        && (kind == "minecraft:item_frame" || kind == "minecraft:glow_item_frame")
        && let Some(s) = stack_of(host, &n["Item"])
    {
        // Jade shows the frame's item name as plain text: no icon, no count.
        t.lines.push(Line::text(item_name(host, &s), NORMAL).details());
    }
    if config::on("potion_effects")
        && let Some(effects) = data.and_then(|d| d["effects"].as_array())
    {
        let limit = config::num("potion_effects.limit") as usize;
        for e in effects.iter().take(limit) {
            let Some(id) = e["id"].as_str() else { continue };
            let (ens, epath) = split_id(id);
            let mut text = tr(host, &format!("effect.{ens}.{epath}"), &title_case(epath));
            let amp = e["amp"].as_i64().unwrap_or(0);
            if amp >= 1 {
                // Jade appends `enchantment.level.N`, or the plain number when untranslatable.
                text.push(' ');
                text.push_str(&tr(host, &format!("enchantment.level.{}", amp + 1), &(amp + 1).to_string()));
            }
            let dur = e["dur"].as_i64().unwrap_or(0);
            let time = if dur < 0 { tr(host, "effect.duration.infinite", "Infinite") } else { clock(dur) };
            text = format!("{text} ({time})");
            let color = match host.effect_key(id).and_then(|k| k.category(host)) {
                Some(EffectCategory::Beneficial) => SUCCESS,
                Some(EffectCategory::Harmful) => DANGER,
                _ => INFO,
            };
            t.lines.push(Line { els: vec![El::Text(text, color)], boxed: true, detail: true });
        }
    }
    if config::on("mob_growth")
        && let Some(a) = age.filter(|a| *a < 0)
    {
        // Jade shows "Paused" while the mob's growth is locked (in love).
        let value = if n["InLove"].as_i64().is_some_and(|l| l > 0) {
            tr(host, "jade.mobgrowth.paused", "Paused")
        } else {
            duration(-a)
        };
        t.lines.push(Line::pair(&tr(host, "jade.mob_growth", "Growing time"), value, INFO).details());
    }
    if config::on("mob_breeding") {
        if n["InLove"].as_i64().is_some_and(|l| l > 0) {
            t.lines.push(Line::pair(&tr(host, "jade.mob_breeding", "Breeding CD"), tr(host, "jade.mob_breeding.in_love", "In Love"), SUCCESS).details());
        } else if let Some(c) = n["DuplicationCooldown"].as_i64().filter(|c| *c > 0 && *c < i32::MAX as i64) {
            // Jade: an allay's duplication cooldown shares the breeding option.
            t.lines.push(Line::text(trf(host, "jade.mobduplication.time", "Duplication CD: %s", &[&duration(c)]), INFO).details());
        } else if let Some(a) = age.filter(|a| *a > 0) {
            // A grown mob's positive age is its breeding cooldown; villagers too.
            t.lines.push(Line::pair(&tr(host, "jade.mob_breeding", "Breeding CD"), duration(a), INFO).details());
        }
    }
    if config::on("armor_stand") && kind == "minecraft:armor_stand" {
        let eq = &n["equipment"];
        for slot in ["head", "chest", "legs", "feet", "mainhand", "offhand"] {
            if let Some(s) = stack_of(host, &eq[slot]) {
                let mut line = Line::new();
                line.push(El::Item(s.clone(), 12.0, None));
                line.push(El::Text(format!(" {}", item_name(host, &s)), NORMAL));
                t.lines.push(line.details());
            }
        }
    }
    if config::on("painting")
        && kind == "minecraft:painting"
        && let Some(v) = n["variant"].as_str()
    {
        let (vns, vpath) = split_id(v);
        // Jade colours the title with the warning colour, the author line stays plain.
        t.lines.push(Line::text(tr(host, &format!("painting.{vns}.{vpath}.title"), &title_case(vpath)), WARNING).details());
        if let Some(author) = host.translate(&format!("painting.{vns}.{vpath}.author")) {
            t.lines.push(Line::text(author, NORMAL).details());
        }
    }
    if config::on("next_entity_drop") {
        // Jade caps the shown timer at two days and skips babies.
        const MAX: i64 = 24000 * 2;
        let grown = age.is_none_or(|a| a >= 0);
        if kind == "minecraft:chicken"
            && grown
            && let Some(e) = n["EggLayTime"].as_i64().filter(|e| *e > 0 && *e < MAX)
        {
            t.lines.push(Line::pair(&tr(host, "jade.next_egg", "Next egg"), duration(e), INFO).details());
        }
        if kind == "minecraft:armadillo"
            && grown
            && let Some(e) = n["scute_time"].as_i64().filter(|e| *e > 0 && *e < MAX)
        {
            t.lines.push(Line::pair(&tr(host, "jade.next_scute", "Next scute"), duration(e), INFO).details());
        }
    }
    if config::on("villager_profession")
        && (kind == "minecraft:villager" || kind == "minecraft:zombie_villager")
        && let Some(prof) = n["VillagerData"]["profession"].as_str()
    {
        // Jade's line is the profession name, then " - " and the level's name — always, not only
        // with details. The `none` profession has no level.
        let (pns, ppath) = split_id(prof);
        let name = tr(host, &format!("entity.{pns}.villager.{ppath}"), &title_case(ppath));
        if prof == "minecraft:none" {
            t.lines.push(Line::text(name, NORMAL).details());
        } else if let Some(level) = n["VillagerData"]["level"].as_i64().filter(|l| (1..=5).contains(l)) {
            let level_name = tr(host, &format!("merchant.level.{level}"), &level.to_string());
            t.lines.push(Line::text(format!("{name} - {level_name}"), NORMAL).details());
        } else {
            t.lines.push(Line::text(name, NORMAL).details());
        }
    }
}

/// Jade's HorseStatsProvider: a llama shows only its strength, a camel nothing, the rest show
/// jump height and speed; with details held, each value gains its maximum.
fn horse_stats(host: &dyn ClientHost, t: &mut Tooltip, kind: &str, data: Option<&Json>, n: &Json, details: bool) {
    const MAX_JUMP_STRENGTH: f64 = 1.0;
    const MAX_MOVEMENT_SPEED: f64 = 0.3375;
    let jump_height = |j: f64| 4.53680079 * j * j + 1.61431730 * j - 0.22656224;
    let speed = |s: f64| s * 43.171815466666658 - 0.000000339999999;
    let switch_text = |t: &mut Tooltip, key: &str, fallback: &str, value: f64, max: f64| {
        let text = if details {
            format!("{} / {}", df_down(value, 2), df_down(max, 2))
        } else {
            df_down(value, 2)
        };
        t.lines.push(Line::text(trf(host, key, fallback, &[&text]), INFO).details());
    };
    if kind == "minecraft:llama" || kind == "minecraft:trader_llama" {
        if let Some(st) = n["Strength"].as_i64() {
            switch_text(t, "jade.llamaStrength", "Strength: %s", st as f64, 5.0);
        }
        return;
    }
    if kind == "minecraft:camel" {
        return;
    }
    if let Some(j) = data.and_then(|d| d["jump"].as_f64()) {
        switch_text(t, "jade.horseStat.jump", "Jump height: %s", jump_height(j), jump_height(MAX_JUMP_STRENGTH));
    }
    if let Some(s) = data.and_then(|d| d["speed"].as_f64()) {
        switch_text(t, "jade.horseStat.speed", "Speed: %s", speed(s), speed(MAX_MOVEMENT_SPEED));
    }
}

// ---------------------------------------------------------------------------------------------
// Fluids

fn fluid_tip(host: &dyn ClientHost, _pos: [i32; 3], fluid: FluidState, details: bool) -> Option<Tooltip> {
    let id = fluid.fluid.id(host);
    let still = id.replace("flowing_", "");
    let mut t = Tooltip { icon: host.fluid_bucket(&still).and_then(|k| ItemStack::new(k, 1)), ..Tooltip::default() };
    let name = host.fluid_name(&still);
    let (ns, path) = split_id(&still);
    let name = if name.is_empty() { tr(host, &format!("block.{ns}.{path}"), &title_case(path)) } else { name };
    t.lines.push(Line::text(name, TITLE));
    if let Some(l) = id_line(host, &still, details) {
        t.lines.push(l);
    }
    mod_tail(&still, &mut t);
    Some(t)
}
