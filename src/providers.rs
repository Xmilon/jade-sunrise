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

/// `1,234,567`.
pub fn grouped(n: i64) -> String {
    let s = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// A count as an item slot shows it: `12`, `12.3k`, `4.5M`.
fn short_count(n: u64) -> String {
    match n {
        0..10_000 => n.to_string(),
        10_000..1_000_000 => format!("{:.1}k", n as f64 / 1000.0).replace(".0k", "k"),
        1_000_000..1_000_000_000 => format!("{:.1}M", n as f64 / 1e6).replace(".0M", "M"),
        _ => format!("{:.1}B", n as f64 / 1e9).replace(".0B", "B"),
    }
}

/// Ticks as `1m 23s` or `45s`.
pub fn duration(ticks: i64) -> String {
    let secs = (ticks.max(0) + 19) / 20;
    if secs >= 3600 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

/// Ticks as `1:23`, the way the inventory shows an effect's time.
fn clock(ticks: i64) -> String {
    let secs = ticks.max(0) / 20;
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}

fn roman(n: i64) -> String {
    const R: [(i64, &str); 13] = [
        (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
    ];
    if !(1..4000).contains(&n) {
        return n.to_string();
    }
    let mut n = n;
    let mut out = String::new();
    for (v, s) in R {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    out
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

fn id_line(id: &str, details: bool, advanced: bool) -> Option<Line> {
    match config::choice("registry_id") {
        1 => Some(Line::text(id, GREY)),
        2 if details => Some(Line::text(id, GREY)),
        3 if advanced => Some(Line::text(id, GREY)),
        _ => None,
    }
}

fn tail(host: &dyn ClientHost, t: &mut Tooltip, id: &str, details: bool) {
    t.lines.extend(mod_line(id));
    t.lines.extend(id_line(id, details, host.gui_options().advanced_item_tooltips));
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
    tail(host, &mut t, "minecraft:grass_block", false);
    t
}

// ---------------------------------------------------------------------------------------------
// Blocks

const CROPS: &[&str] = &[
    "minecraft:cocoa", "minecraft:sweet_berry_bush", "minecraft:nether_wart", "minecraft:melon_stem",
    "minecraft:pumpkin_stem", "minecraft:pitcher_crop", "minecraft:torchflower_crop", "minecraft:cave_vines",
    "minecraft:kelp", "minecraft:bamboo_sapling",
];

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
    let mut name = host
        .translate(&format!("block.{ns}.{path}"))
        .or_else(|| icon.as_ref().map(|s| item_name(host, s)))
        .unwrap_or_else(|| title_case(path));
    if let Some(custom) = data.and_then(|d| d["name"].as_str()) {
        name = custom.to_owned();
    }
    // Spawners: "<Mob> (Monster Spawner)".
    if config::on("spawner")
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
        name_line.push(El::Text(format!(" ({})", tr(host, &format!("jade.direction.{f}"), &title_case(f))), NORMAL));
    }
    if config::on("distance")
        && let Some(cam) = host.camera()
    {
        let d = ((cam.position[0] - hit[0]).powi(2) + (cam.position[1] - hit[1]).powi(2) + (cam.position[2] - hit[2]).powi(2)).sqrt();
        name_line.push(El::Text(format!(" [{d:.1}]"), NORMAL));
    }
    // Harvest tool: icons, then a check or a cross.
    let harvest = harvest_els(host, key, state);
    let harvest_new_line = config::on("harvest_tool_new_line");
    if !harvest.is_empty() && !harvest_new_line {
        name_line.push(El::Space(6.0));
        name_line.els.extend(harvest.iter().cloned());
    }
    t.lines.push(name_line);
    if !harvest.is_empty() && harvest_new_line {
        t.lines.push(Line { els: harvest, ..Line::default() });
    }
    if config::on("show_unbreakable") && host.block_states().destroy_speed(state).is_some_and(|s| s < 0.0) {
        t.lines.push(Line::text(tr(host, "jade.unbreakable", "Unbreakable"), DANGER));
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
    // Crop growth.
    if config::on("crop_progress")
        && (host.block_in_tag(key, "minecraft:crops") == Some(true) || CROPS.contains(&id.as_str()))
        && let Some(age) = prop(&props, "age").and_then(|v| v.parse::<i64>().ok())
        && let Some(max) = prop_max(host, key, "age")
        && max > 0
    {
        let label = tr(host, "jade.crop_growth", "Growth");
        if age >= max {
            t.lines.push(Line::pair(&label, tr(host, "jade.crop_mature", "Mature"), SUCCESS));
        } else {
            t.lines.push(Line::pair(&label, format!("{}%", age * 100 / max), INFO));
        }
    }
    redstone(host, &mut t, &id, &props, be);
    if config::on("note_block")
        && id == "minecraft:note_block"
        && let (Some(inst), Some(note)) = (prop(&props, "instrument"), prop(&props, "note").and_then(|n| n.parse::<i64>().ok()))
    {
        const NAMES: [&str; 12] = ["F#", "G", "G#", "A", "A#", "B", "C", "C#", "D", "D#", "E", "F"];
        let octave = 3 + (note + 6) / 12;
        let inst_name = tr(host, &format!("jade.instrument.{inst}"), &title_case(inst));
        t.lines.push(Line::text(format!("{inst_name} {}{octave}", NAMES[(note % 12) as usize]), INFO));
    }
    if config::on("tnt_stability") && id == "minecraft:tnt" && prop(&props, "unstable") == Some("true") {
        t.lines.push(Line::text(tr(host, "jade.tnt.unstable", "Unstable"), DANGER));
    }
    if config::on("beehive") && (id == "minecraft:beehive" || id == "minecraft:bee_nest") {
        if let Some(honey) = prop(&props, "honey_level").and_then(|v| v.parse::<i64>().ok()) {
            t.lines.push(Line::pair(&tr(host, "jade.beehive.honey", "Honey"), format!("{honey} / 5"), if honey >= 5 { SUCCESS } else { INFO }));
        }
        if let Some(bees) = be.and_then(|b| b["bees"].as_array().or_else(|| b["Bees"].as_array())) {
            let n = bees.len();
            t.lines.push(Line::pair(&tr(host, "jade.beehive.bees", "Bees"), format!("{n} / 3"), if n >= 3 { SUCCESS } else { INFO }));
        }
    }
    if let Some(be) = be {
        block_entity_lines(host, &mut t, &id, be, data, details);
    } else if let Some(d) = data
        && d["ender"].is_array()
    {
        storage(host, &mut t, d["ender"].as_array().map(Vec::as_slice).unwrap_or(&[]), details);
    }
    // Block states, with the details key held.
    if config::on("block_states") && details && !props.is_empty() {
        let defaults = host.block_state_default(key).map(|d| properties(host, d)).unwrap_or_default();
        for (k, v) in &props {
            if prop(&defaults, k) != Some(v.as_str()) {
                t.lines.push(Line::text(format!("{k}: {v}"), NORMAL).details());
            }
        }
    }
    if config::on("block_properties") && details {
        let bs = host.block_states();
        if let Some(r) = bs.explosion_resistance(state) {
            t.lines.push(Line::pair(&tr(host, "jade.block_properties.explosion_resistance", "Explosion resistance"), trim(r), INFO));
        }
        if let Some(d) = bs.destroy_speed(state) {
            t.lines.push(Line::pair(&tr(host, "jade.block_properties.destroy_time", "Destroy time"), trim(d), INFO));
        }
    }
    if config::on("coordinates") {
        t.lines.push(Line::text(format!("{} {} {}", pos[0], pos[1], pos[2]), NORMAL));
    }
    tail(host, &mut t, &id, details);
    Some(t)
}

fn trim(f: f32) -> String {
    let s = format!("{f:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn redstone(host: &dyn ClientHost, t: &mut Tooltip, id: &str, props: &[(String, String)], be: Option<&Json>) {
    if !config::on("redstone") {
        return;
    }
    match id {
        "minecraft:lever" => {
            let on = prop(props, "powered") == Some("true");
            let label = tr(host, "jade.lever.state", "State");
            let value = if on { tr(host, "jade.on", "On") } else { tr(host, "jade.off", "Off") };
            t.lines.push(Line::pair(&label, value, if on { SUCCESS } else { DANGER }));
        }
        "minecraft:repeater" => {
            if let Some(d) = prop(props, "delay") {
                t.lines.push(Line::pair(&tr(host, "jade.repeater.delay", "Delay"), d.to_owned(), INFO));
            }
            if prop(props, "locked") == Some("true") {
                t.lines.push(Line::text(tr(host, "jade.repeater.locked", "Locked"), WARNING));
            }
        }
        "minecraft:comparator" => {
            let subtract = prop(props, "mode") == Some("subtract");
            let mode = if subtract { tr(host, "jade.comparator.subtractor", "Subtractor") } else { tr(host, "jade.comparator.comparator", "Comparator") };
            t.lines.push(Line::pair(&tr(host, "jade.comparator.mode", "Mode"), mode, INFO));
            if let Some(power) = be.and_then(|b| num(b, &["OutputSignal", "output_signal"])) {
                t.lines.push(Line::pair(&tr(host, "jade.power", "Power"), power.to_string(), INFO));
            }
        }
        _ => {
            if let Some(power) = prop(props, "power") {
                t.lines.push(Line::pair(&tr(host, "jade.power", "Power"), power.to_owned(), INFO));
            }
        }
    }
}

const FURNACES: &[&str] = &["minecraft:furnace", "minecraft:smoker", "minecraft:blast_furnace"];

fn block_entity_lines(host: &dyn ClientHost, t: &mut Tooltip, id: &str, be: &Json, data: Option<&Json>, details: bool) {
    let items: &[Json] = be["Items"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let slot = |n: i64| items.iter().find(|s| s["Slot"].as_i64() == Some(n)).and_then(|s| stack_of(host, s));
    if FURNACES.contains(&id) {
        if !config::on("furnace") {
            return;
        }
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
        t.lines.push(line);
        return;
    }
    match id {
        "minecraft:brewing_stand" => {
            if !config::on("brewing_stand") {
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
            t.lines.push(line);
        }
        "minecraft:jukebox" => {
            if !config::on("jukebox") {
                return;
            }
            let record = &be["RecordItem"];
            let record = if record.is_object() { record } else { &be["record_item"] };
            match record["id"].as_str() {
                Some(disc) => {
                    let song = disc.rsplit(':').next().unwrap_or(disc).strip_prefix("music_disc_").unwrap_or("");
                    let (ns, _) = split_id(disc);
                    let fallback = stack(host, disc, 1).map(|s| item_name(host, &s)).unwrap_or_else(|| title_case(song));
                    let title = tr(host, &format!("jukebox_song.{ns}.{song}"), &fallback);
                    t.lines.push(Line::text(trf(host, "record.nowPlaying", "Now Playing: %s", &[&title]), INFO));
                }
                None => t.lines.push(Line::text(tr(host, "jade.empty", "Empty"), NORMAL)),
            }
        }
        "minecraft:lectern" => {
            if !config::on("lectern") {
                return;
            }
            if let Some(book) = stack_of(host, &be["Book"]) {
                let title = be["Book"]["components"]["minecraft:written_book_content"]["title"]["raw"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| item_name(host, &book));
                let mut line = Line::new();
                line.push(El::Item(book, 12.0, None));
                line.push(El::Text(format!(" {title}"), INFO));
                t.lines.push(line);
            }
        }
        "minecraft:command_block" | "minecraft:chain_command_block" | "minecraft:repeating_command_block" => {
            if let Some(cmd) = be["Command"].as_str().filter(|c| !c.is_empty())
                && config::on("command_block")
            {
                let cmd: String = cmd.chars().take(60).collect();
                t.lines.push(Line::text(format!("> {cmd}"), INFO));
            }
        }
        "minecraft:campfire" | "minecraft:soul_campfire" => {
            if !config::on("campfire") {
                return;
            }
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
                t.lines.push(line);
            }
        }
        _ => {
            let hopper_locked = id == "minecraft:hopper" && data.is_some_and(|d| d["locked"].as_bool() == Some(true));
            if config::on("hopper_lock") && hopper_locked {
                t.lines.push(Line::text(tr(host, "jade.locked", "Inventory is locked"), WARNING));
            }
            if !config::on("item_storage") {
                return;
            }
            if be.get("LootTable").is_some() || be.get("loot_table").is_some() {
                t.lines.push(Line { els: vec![El::Italic(tr(host, "jade.not_generated", "What could it be?"), NORMAL, 1.0)], ..Line::default() });
                return;
            }
            if data.is_some_and(|d| d["locked"].as_bool() == Some(true)) && !hopper_locked {
                t.lines.push(Line::text(tr(host, "jade.locked", "Inventory is locked"), WARNING));
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
    }
}

/// A container's contents: a grid of icons, or rows with names when few kinds are inside.
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
    if config::on("sort_by_count") {
        merged.sort_by(|a, b| b.1.cmp(&a.1));
    }
    let limit = config::num(if details { "detailed_amount" } else { "normal_amount" }) as usize;
    if limit == 0 {
        return;
    }
    let shown = &merged[..merged.len().min(limit)];
    if merged.len() <= config::num("show_name_threshold") as usize {
        for (s, n) in shown {
            let mut line = Line::new();
            line.push(El::Item(s.clone(), 16.0, None));
            line.push(El::Space(3.0));
            line.push(El::Text(format!("{}× ", short_count(*n)), INFO));
            line.push(El::Text(item_name(host, s), NORMAL));
            t.lines.push(line);
        }
        return;
    }
    let per = (config::num("items_per_line") as usize).max(1);
    for row in shown.chunks(per) {
        let mut line = Line::new();
        for (s, n) in row {
            line.push(El::Item(s.clone(), 16.0, (*n > 1).then(|| short_count(*n))));
            line.push(El::Space(2.0));
        }
        t.lines.push(line);
    }
}

/// The tool icons for a block and whether the held item harvests it.
fn harvest_els(host: &dyn ClientHost, block: BlockKey, state: BlockState) -> Vec<El> {
    if !config::on("harvest_tool") || (creative(host) && !config::on("harvest_tool_creative")) {
        return Vec::new();
    }
    if host.block_states().destroy_speed(state).is_some_and(|s| s < 0.0) {
        return Vec::new();
    }
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
    for kind in ["pickaxe", "axe", "shovel", "hoe"] {
        if host.block_in_tag(block, &format!("minecraft:mineable/{kind}")) == Some(true) {
            tools.push(format!("minecraft:{tier}_{kind}"));
        }
    }
    let id = block.id(host);
    if host.block_in_tag(block, "minecraft:leaves") == Some(true)
        || host.block_in_tag(block, "minecraft:wool") == Some(true)
        || matches!(id.as_str(), "minecraft:cobweb" | "minecraft:vine" | "minecraft:glow_lichen" | "minecraft:dead_bush" | "minecraft:short_grass" | "minecraft:tall_grass" | "minecraft:fern" | "minecraft:seagrass" | "minecraft:hanging_roots" | "minecraft:twisting_vines" | "minecraft:weeping_vines" | "minecraft:tripwire")
    {
        tools.push("minecraft:shears".into());
    }
    if host.block_in_tag(block, "minecraft:sword_efficient") == Some(true) || id == "minecraft:cobweb" || id == "minecraft:bamboo" {
        tools.push("minecraft:wooden_sword".into());
    }
    let requires = host.block_states().requires_correct_tool(state).unwrap_or(false);
    if tools.is_empty() && !requires {
        return Vec::new();
    }
    let mut out = Vec::new();
    for tool in &tools {
        if let Some(s) = stack(host, tool, 1) {
            out.push(El::Item(s, 12.0, None));
        }
    }
    if requires || config::on("effective_tool") {
        let held = held(host);
        let correct = held.as_ref().and_then(|s| host.block_states().is_correct_tool_for_drops(s, state)).unwrap_or(false);
        // The cross means "this won't drop": a block any tool harvests gets the check when the held
        // tool is its effective one, and no mark otherwise.
        if requires || correct {
            out.push(El::Space(1.0));
            out.push(El::Check(correct));
        }
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
    // The icon: the item the entity is (a dropped item, a minecart, a boat, an armor stand).
    let dropped = (kind_id == "minecraft:item").then(|| nbt.and_then(|n| stack_of(host, &n["Item"]))).flatten();
    if config::on("item_entity")
        && let Some(s) = &dropped
    {
        name = item_name(host, s);
        t.icon = Some(s.clone());
    } else if kind.is_some() {
        t.icon = stack(host, &kind_id, 1);
    }
    t.lines.push(Line::text(name, TITLE));
    if config::on("item_entity")
        && let Some(s) = &dropped
        && s.count() > 1
    {
        t.lines.push(Line::pair(&tr(host, "jade.count", "Count"), s.count().to_string(), INFO));
    }
    if let Some(d) = data {
        health(host, &mut t, d);
    }
    if let Some(n) = nbt {
        entity_lines(host, &mut t, &kind_id, n, data, details);
    }
    if config::on("mob_effects")
        && let Some(effects) = data.and_then(|d| d["effects"].as_array())
    {
        let limit = config::num("effects_limit") as usize;
        for e in effects.iter().take(limit) {
            let Some(id) = e["id"].as_str() else { continue };
            let (ens, epath) = split_id(id);
            let mut text = tr(host, &format!("effect.{ens}.{epath}"), &title_case(epath));
            let amp = e["amp"].as_i64().unwrap_or(0);
            if amp > 0 {
                text.push(' ');
                text.push_str(&tr(host, &format!("enchantment.level.{}", amp + 1), &roman(amp + 1)));
            }
            let dur = e["dur"].as_i64().unwrap_or(0);
            text.push(' ');
            text.push_str(&if dur < 0 { tr(host, "jade.infinite", "Infinite") } else { clock(dur) });
            let harmful = host.effect_key(id).and_then(|k| k.category(host)) == Some(EffectCategory::Harmful);
            t.lines.push(Line { els: vec![El::Text(text, if harmful { DANGER } else { MOD_BLUE })], boxed: true, detail: false });
        }
    }
    if config::on("animal_owner")
        && let Some(owner) = data.and_then(|d| d["owner"].as_str())
    {
        t.lines.push(Line::pair(&tr(host, "jade.owner", "Owner"), owner.to_owned(), INFO));
    }
    tail(host, &mut t, &kind_id, details);
    Some(t)
}

fn health(host: &dyn ClientHost, t: &mut Tooltip, d: &Json) {
    let (Some(hp), Some(max)) = (d["health"].as_f64(), d["max_health"].as_f64()) else { return };
    let abs = d["absorption"].as_f64().unwrap_or(0.0);
    if config::on("entity_health") {
        let limit = config::num("max_health_for_render");
        if max + abs > limit || hp + abs > limit {
            let shown = if config::on("show_fractions") { format!("{:.1}", hp + abs) } else { grouped((hp + abs).ceil() as i64) };
            let mut line = Line::new();
            line.push(El::Sprite("hud/heart/full", 9, 9));
            line.push(El::Space(2.0));
            line.push(El::Text(format!("{shown} / {}", grouped(max.ceil() as i64)), INFO));
            t.lines.push(line);
        } else {
            let per = (config::num("icons_per_line") as usize).max(1);
            let hearts = ((max / 2.0).ceil() as usize).max(1);
            let abs_hearts = (abs / 2.0).ceil() as usize;
            let mut line = Line::new();
            let mut n = 0;
            let mut push_heart = |line: &mut Line, fill: Option<&'static str>, container: bool| {
                if n > 0 && n % per == 0 {
                    let done = std::mem::take(line);
                    t.lines.push(done);
                }
                if container {
                    line.push(El::Sprite("hud/heart/container", 9, 9));
                    line.push(El::Space(-9.0));
                }
                if let Some(f) = fill {
                    line.push(El::Sprite(f, 9, 9));
                    line.push(El::Space(-9.0));
                }
                line.push(El::Space(8.0));
                n += 1;
            };
            for i in 0..hearts {
                let v = hp - (i as f64) * 2.0;
                let fill = if v >= 2.0 { Some("hud/heart/full") } else if v >= 1.0 || v > 0.0 { Some("hud/heart/half") } else { None };
                push_heart(&mut line, fill, true);
            }
            for i in 0..abs_hearts {
                let v = abs - (i as f64) * 2.0;
                let fill = if v >= 2.0 { "hud/heart/absorbing_full" } else { "hud/heart/absorbing_half" };
                push_heart(&mut line, Some(fill), true);
            }
            if !line.els.is_empty() {
                line.push(El::Space(1.0));
                t.lines.push(line);
            }
        }
    }
    if config::on("entity_armor")
        && let Some(armor) = d["armor"].as_f64()
        && armor >= 1.0
    {
        let armor = armor.floor() as i64;
        let mut line = Line::new();
        if armor > 20 {
            line.push(El::Sprite("hud/armor_full", 9, 9));
            line.push(El::Space(2.0));
            line.push(El::Text(grouped(armor), INFO));
        } else {
            for i in 0..10 {
                let v = armor - i * 2;
                let s = if v >= 2 { "hud/armor_full" } else if v == 1 { "hud/armor_half" } else { "hud/armor_empty" };
                line.push(El::Sprite(s, 9, 9));
                line.push(El::Space(-1.0));
            }
            line.push(El::Space(1.0));
        }
        t.lines.push(line);
    }
    let _ = host;
}

fn entity_lines(host: &dyn ClientHost, t: &mut Tooltip, kind: &str, n: &Json, data: Option<&Json>, details: bool) {
    let age = n["Age"].as_i64();
    if config::on("villager_profession")
        && (kind == "minecraft:villager" || kind == "minecraft:zombie_villager")
        && let Some(prof) = n["VillagerData"]["profession"].as_str()
    {
        let (pns, ppath) = split_id(prof);
        let name = tr(host, &format!("entity.{pns}.villager.{ppath}"), &title_case(ppath));
        let mut text = name;
        if details
            && let Some(level) = n["VillagerData"]["level"].as_i64()
        {
            text.push_str(&format!(" ({})", tr(host, &format!("merchant.level.{level}"), &level.to_string())));
        }
        t.lines.push(Line::pair(&tr(host, "jade.profession", "Profession"), text, INFO));
    }
    if config::on("zombie_villager")
        && kind == "minecraft:zombie_villager"
        && let Some(c) = n["ConversionTime"].as_i64().filter(|c| *c > 0)
    {
        t.lines.push(Line::pair(&tr(host, "jade.zombie_conversion", "Conversion time"), duration(c), INFO));
    }
    if config::on("mob_growth")
        && let Some(a) = age.filter(|a| *a < 0)
    {
        t.lines.push(Line::pair(&tr(host, "jade.mob_growth", "Growing time"), duration(-a), INFO));
    }
    if config::on("mob_breeding") {
        if n["InLove"].as_i64().is_some_and(|l| l > 0) {
            t.lines.push(Line::pair(&tr(host, "jade.mob_breeding", "Breeding CD"), tr(host, "jade.mob_breeding.in_love", "In Love"), SUCCESS));
        } else if let Some(a) = age.filter(|a| *a > 0) {
            t.lines.push(Line::pair(&tr(host, "jade.mob_breeding", "Breeding CD"), duration(a), INFO));
        }
    }
    if config::on("next_drop") {
        if kind == "minecraft:chicken"
            && let Some(e) = n["EggLayTime"].as_i64().filter(|e| *e > 0)
            && age.is_none_or(|a| a >= 0)
        {
            t.lines.push(Line::pair(&tr(host, "jade.next_egg", "Next egg"), duration(e), INFO));
        }
        if kind == "minecraft:armadillo"
            && let Some(e) = n["scute_time"].as_i64().filter(|e| *e > 0)
        {
            t.lines.push(Line::pair(&tr(host, "jade.next_scute", "Next scute"), duration(e), INFO));
        }
    }
    if config::on("horse_stats") && HORSES.contains(&kind) {
        if let Some(j) = data.and_then(|d| d["jump"].as_f64()) {
            // The height a jump of this strength reaches, as the community's fit of vanilla's motion.
            let h = -0.1817584952 * j.powi(3) + 3.689713992 * j.powi(2) + 2.128599134 * j - 0.343930367;
            t.lines.push(Line::pair(&tr(host, "jade.horse_jump", "Jump height"), format!("{h:.1}"), INFO));
        }
        if let Some(s) = data.and_then(|d| d["speed"].as_f64()) {
            t.lines.push(Line::pair(&tr(host, "jade.horse_speed", "Speed"), format!("{:.1}", s * 42.16), INFO));
        }
        if let Some(st) = n["Strength"].as_i64() {
            t.lines.push(Line::pair(&tr(host, "jade.llama_strength", "Strength"), format!("{st} / 5"), INFO));
        }
    }
    if config::on("item_frame")
        && (kind == "minecraft:item_frame" || kind == "minecraft:glow_item_frame")
        && let Some(s) = stack_of(host, &n["Item"])
    {
        let mut line = Line::new();
        line.push(El::Item(s.clone(), 12.0, None));
        line.push(El::Text(format!(" {}", item_name(host, &s)), INFO));
        t.lines.push(line);
    }
    if config::on("painting")
        && kind == "minecraft:painting"
        && let Some(v) = n["variant"].as_str()
    {
        let (vns, vpath) = split_id(v);
        t.lines.push(Line::text(tr(host, &format!("painting.{vns}.{vpath}.title"), &title_case(vpath)), INFO));
        if let Some(author) = host.translate(&format!("painting.{vns}.{vpath}.author")) {
            t.lines.push(Line::text(author, NORMAL));
        }
    }
    if config::on("armor_stand") && kind == "minecraft:armor_stand" {
        let eq = &n["equipment"];
        for slot in ["head", "chest", "legs", "feet", "mainhand", "offhand"] {
            if let Some(s) = stack_of(host, &eq[slot]) {
                let mut line = Line::new();
                line.push(El::Item(s.clone(), 12.0, None));
                line.push(El::Text(format!(" {}", item_name(host, &s)), INFO));
                t.lines.push(line);
            }
        }
    }
    if config::on("item_storage")
        && kind.ends_with("minecart")
        && let Some(items) = n["Items"].as_array()
    {
        storage(host, t, items, details);
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
    tail(host, &mut t, &still, details);
    Some(t)
}
