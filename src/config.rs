//! Jade's options: one table that feeds three places.
//!
//! - The game's own settings page (Options > Mods > Jade > Config), through `register_setting`: the
//!   in-game screen, kept by the game in the mod's settings folder.
//! - `config/jade-client.toml`, a plain file in the game's config folder (`register_configs`): read
//!   when the game starts and whenever it changes on disk, and rewritten when a value changes on the
//!   settings page or in Jade's own quick screen.
//! - An in-memory copy ([`get`]), so drawing a frame reads no file and takes no game lock.
//!
//! The file wins at start: its values are pushed into the settings page on the first client tick.

use mod_api::*;
use std::sync::Mutex;

pub const FILE: &str = "jade-client.toml";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Toggle(bool),
    /// Options (their file names, also the end of their translation keys) and the default index.
    Choice(&'static [&'static str], usize),
    /// min, max, step, default.
    Range(f64, f64, f64, f64),
}

#[derive(Clone, Copy, Debug)]
pub struct Opt {
    pub key: &'static str,
    pub section: &'static str,
    pub kind: Kind,
}

const fn t(section: &'static str, key: &'static str, on: bool) -> Opt {
    Opt { key, section, kind: Kind::Toggle(on) }
}
const fn c(section: &'static str, key: &'static str, options: &'static [&'static str], default: usize) -> Opt {
    Opt { key, section, kind: Kind::Choice(options, default) }
}
const fn r(section: &'static str, key: &'static str, min: f64, max: f64, step: f64, default: f64) -> Opt {
    Opt { key, section, kind: Kind::Range(min, max, step, default) }
}

pub const DISPLAY_MODES: &[&str] = &["toggle", "hold", "lite"];
pub const FLUID_MODES: &[&str] = &["any", "none", "fallback"];
pub const THEMES: &[&str] = &["dark", "classic", "flat"];
pub const ICON_MODES: &[&str] = &["top", "centered", "inline", "hide"];
pub const MOD_NAME_MODES: &[&str] = &["on", "smaller", "off"];
pub const ID_MODES: &[&str] = &["off", "always", "details", "advanced_tooltips"];
pub const BOSS_BAR_MODES: &[&str] = &["push_down", "no_operation", "hide_tooltip"];

/// Every option, in the order the settings page and the file list them.
pub const OPTS: &[Opt] = &[
    // General
    t("general", "enabled", true),
    t("general", "display_blocks", true),
    t("general", "display_entities", true),
    t("general", "display_bosses", true),
    c("general", "display_mode", DISPLAY_MODES, 0),
    c("general", "fluid_mode", FLUID_MODES, 0),
    c("general", "boss_bar_overlap", BOSS_BAR_MODES, 0),
    r("general", "extended_reach", 0.0, 20.0, 1.0, 0.0),
    t("general", "hide_from_guis", true),
    t("general", "hide_from_player_list", true),
    // Appearance
    c("appearance", "theme", THEMES, 0),
    r("appearance", "pos_x", 0.0, 1.0, 0.01, 0.5),
    r("appearance", "pos_y", 0.0, 1.0, 0.01, 1.0),
    r("appearance", "anchor_x", 0.0, 1.0, 0.05, 0.5),
    r("appearance", "anchor_y", 0.0, 1.0, 0.05, 0.0),
    r("appearance", "scale", 0.2, 2.0, 0.05, 1.0),
    r("appearance", "auto_scale_threshold", 0.2, 1.0, 0.05, 0.4),
    r("appearance", "opacity", 0.0, 1.0, 0.05, 0.7),
    c("appearance", "icon_mode", ICON_MODES, 0),
    t("appearance", "animation", true),
    r("appearance", "disappearing_delay", 0.0, 5.0, 0.1, 0.0),
    t("appearance", "flip_main_hand", false),
    // Formatting
    c("formatting", "mod_name", MOD_NAME_MODES, 0),
    c("formatting", "registry_id", ID_MODES, 0),
    // Providers: blocks
    t("blocks", "harvest_tool", true),
    t("blocks", "harvest_tool_creative", false),
    t("blocks", "effective_tool", true),
    t("blocks", "harvest_tool_new_line", false),
    t("blocks", "show_unbreakable", false),
    t("blocks", "breaking_progress", true),
    t("blocks", "block_states", true),
    t("blocks", "block_properties", false),
    t("blocks", "crop_progress", true),
    t("blocks", "redstone", true),
    t("blocks", "furnace", true),
    t("blocks", "brewing_stand", true),
    t("blocks", "beehive", true),
    t("blocks", "jukebox", true),
    t("blocks", "note_block", true),
    t("blocks", "spawner", true),
    t("blocks", "command_block", true),
    t("blocks", "lectern", true),
    t("blocks", "campfire", true),
    t("blocks", "tnt_stability", true),
    t("blocks", "hopper_lock", true),
    t("blocks", "block_face", false),
    t("blocks", "coordinates", false),
    t("blocks", "distance", false),
    // Providers: containers
    t("storage", "item_storage", true),
    r("storage", "items_per_line", 3.0, 27.0, 1.0, 9.0),
    r("storage", "normal_amount", 0.0, 54.0, 1.0, 9.0),
    r("storage", "detailed_amount", 0.0, 54.0, 1.0, 54.0),
    r("storage", "show_name_threshold", 0.0, 27.0, 1.0, 5.0),
    t("storage", "sort_by_count", false),
    // Providers: entities
    t("entities", "entity_health", true),
    r("entities", "max_health_for_render", 0.0, 200.0, 1.0, 40.0),
    r("entities", "icons_per_line", 5.0, 40.0, 1.0, 10.0),
    t("entities", "show_fractions", false),
    t("entities", "entity_armor", true),
    t("entities", "mob_effects", true),
    r("entities", "effects_limit", 1.0, 99.0, 1.0, 7.0),
    t("entities", "animal_owner", true),
    t("entities", "villager_profession", true),
    t("entities", "mob_growth", true),
    t("entities", "mob_breeding", true),
    t("entities", "horse_stats", true),
    t("entities", "item_frame", true),
    t("entities", "item_entity", true),
    t("entities", "painting", true),
    t("entities", "armor_stand", true),
    t("entities", "zombie_villager", true),
    t("entities", "next_drop", true),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Choice(usize),
    Num(f64),
}

impl Value {
    /// The text `set_setting` keeps (`SettingSpec::read`'s form).
    pub fn setting_text(self) -> String {
        match self {
            Value::Bool(b) => b.to_string(),
            Value::Choice(i) => i.to_string(),
            Value::Num(n) => trim_num(n),
        }
    }
}

fn trim_num(n: f64) -> String {
    let s = format!("{n:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" { "0".into() } else { s.to_owned() }
}

pub fn default_of(opt: &Opt) -> Value {
    match opt.kind {
        Kind::Toggle(b) => Value::Bool(b),
        Kind::Choice(_, i) => Value::Choice(i),
        Kind::Range(_, _, _, d) => Value::Num(d),
    }
}

pub struct Values {
    pub values: Vec<Value>,
    /// The file was read (or changed) and its values are not in the settings page yet.
    pub push_to_settings: bool,
    /// A value changed here and the file is to be written.
    pub write_file: bool,
    /// The settings page was read once since start.
    pub read_settings: bool,
}

static VALUES: Mutex<Option<Values>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Values) -> R) -> R {
    let mut guard = VALUES.lock().unwrap_or_else(|e| e.into_inner());
    let v = guard.get_or_insert_with(|| Values {
        values: OPTS.iter().map(default_of).collect(),
        push_to_settings: false,
        write_file: false,
        read_settings: false,
    });
    f(v)
}

fn index(key: &str) -> Option<usize> {
    OPTS.iter().position(|o| o.key == key)
}

pub fn get(key: &str) -> Value {
    let i = index(key).unwrap_or_else(|| panic!("jade: unknown option {key}"));
    with(|v| v.values[i])
}

pub fn on(key: &str) -> bool {
    matches!(get(key), Value::Bool(true))
}

pub fn choice(key: &str) -> usize {
    match get(key) {
        Value::Choice(i) => i,
        _ => 0,
    }
}

pub fn num(key: &str) -> f64 {
    match get(key) {
        Value::Num(n) => n,
        _ => 0.0,
    }
}

pub fn set(key: &str, value: Value) {
    if let Some(i) = index(key) {
        with(|v| {
            if v.values[i] != value {
                v.values[i] = value;
                v.write_file = true;
            }
        });
    }
}

/// The settings the game shows on Jade's page, with `config.jade.<key>` labels.
pub fn register(r: &mut dyn Registrar) {
    for opt in OPTS {
        let label = format!("config.jade.{}", opt.key);
        let spec = match opt.kind {
            Kind::Toggle(b) => SettingSpec::toggle(opt.key, label.clone(), b),
            Kind::Choice(options, d) => SettingSpec::choice(
                opt.key,
                label.clone(),
                options.iter().map(|o| format!("config.jade.{}.{o}", opt.key)),
                d,
            ),
            Kind::Range(min, max, step, d) => SettingSpec::range(opt.key, label.clone(), min, max, step, d),
        }
        .tooltip(format!("{label}.tooltip"));
        if let Err(e) = r.register_setting(spec) {
            eprintln!("[jade] setting {} refused: {e}", opt.key);
        }
    }
}

/// Reads every setting from the game's settings page into the in-memory copy.
pub fn read_settings(host: &dyn ClientHost) {
    let read: Vec<Value> = OPTS
        .iter()
        .map(|opt| match opt.kind {
            Kind::Toggle(d) => Value::Bool(host.setting_bool(opt.key).unwrap_or(d)),
            Kind::Choice(_, d) => Value::Choice(host.setting_choice(opt.key).map(|i| i as usize).unwrap_or(d)),
            Kind::Range(_, _, _, d) => Value::Num(host.setting_f64(opt.key).unwrap_or(d)),
        })
        .collect();
    with(|v| {
        if v.values != read {
            v.values = read;
            v.write_file = true;
        }
        v.read_settings = true;
    });
}

/// One setting changed on the settings page.
pub fn setting_changed(host: &dyn ClientHost, key: &str) {
    let Some(i) = index(key) else { return };
    let opt = OPTS[i];
    let value = match opt.kind {
        Kind::Toggle(d) => Value::Bool(host.setting_bool(key).unwrap_or(d)),
        Kind::Choice(_, d) => Value::Choice(host.setting_choice(key).map(|i| i as usize).unwrap_or(d)),
        Kind::Range(_, _, _, d) => Value::Num(host.setting_f64(key).unwrap_or(d)),
    };
    set(key, value);
}

/// Once a client tick: pushes the file's values into the settings page after the file was read,
/// writes the file after a value changed here.
pub fn sync(host: &mut dyn ClientHost) {
    let (push, write, values) = with(|v| {
        let out = (v.push_to_settings, v.write_file, v.values.clone());
        v.push_to_settings = false;
        v.write_file = false;
        out
    });
    if push {
        for (opt, value) in OPTS.iter().zip(&values) {
            host.set_setting(crate::ID, opt.key, &value.setting_text());
        }
    }
    if write {
        let text = to_toml(&values);
        match ConfigPath::new(FILE) {
            Ok(path) => {
                if let Err(e) = host.config_files().write(&path, text.as_bytes()) {
                    eprintln!("[jade] could not write config/{FILE}: {e:?}");
                }
            }
            Err(e) => eprintln!("[jade] config path: {e:?}"),
        }
    }
}

/// The file's text: one `[section]` per group, `key = value` per option, choices as quoted names.
pub fn to_toml(values: &[Value]) -> String {
    let mut out = String::from(
        "# Jade for Sunrise: the overlay's options. Edit here or in Options > Mods > Jade > Config;\n\
         # the game reads this file again when it changes.\n",
    );
    let mut section = "";
    for (opt, value) in OPTS.iter().zip(values) {
        if opt.section != section {
            section = opt.section;
            out.push_str(&format!("\n[{section}]\n"));
        }
        let text = match (opt.kind, value) {
            (Kind::Choice(options, _), Value::Choice(i)) => format!("\"{}\"", options.get(*i).copied().unwrap_or(options[0])),
            (_, v) => v.setting_text(),
        };
        out.push_str(&format!("{} = {text}\n", opt.key));
    }
    out
}

/// Parses the file: unknown keys and bad values keep the current value.
pub fn from_toml(text: &str) -> Vec<Value> {
    let mut values: Vec<Value> = with(|v| v.values.clone());
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, raw)) = line.split_once('=') else { continue };
        let (key, raw) = (key.trim(), raw.trim().trim_matches('"'));
        let Some(i) = index(key) else { continue };
        let parsed = match OPTS[i].kind {
            Kind::Toggle(_) => raw.parse::<bool>().ok().map(Value::Bool),
            Kind::Choice(options, _) => options
                .iter()
                .position(|o| *o == raw)
                .or_else(|| raw.parse::<usize>().ok().filter(|i| *i < options.len()))
                .map(Value::Choice),
            Kind::Range(min, max, _, _) => raw.parse::<f64>().ok().filter(|n| n.is_finite()).map(|n| Value::Num(n.clamp(min, max))),
        };
        if let Some(p) = parsed {
            values[i] = p;
        }
    }
    values
}

/// The file was read: take its values, and push them into the settings page next tick.
pub fn file_loaded(text: &str) {
    let values = from_toml(text);
    with(|v| {
        v.values = values;
        v.push_to_settings = true;
        // A file with missing keys (an older version) is completed on disk.
        v.write_file = text.trim().is_empty() || OPTS.iter().any(|o| !text.contains(&format!("{} =", o.key)));
    });
}

pub fn default_file() -> String {
    to_toml(&OPTS.iter().map(default_of).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_round_trips_every_option() {
        let mut values: Vec<Value> = OPTS.iter().map(default_of).collect();
        values[index("display_mode").unwrap()] = Value::Choice(2);
        values[index("scale").unwrap()] = Value::Num(1.25);
        values[index("enabled").unwrap()] = Value::Bool(false);
        let text = to_toml(&values);
        assert!(text.contains("display_mode = \"lite\""));
        assert_eq!(from_toml(&text), values);
    }

    #[test]
    fn out_of_range_numbers_are_clamped_and_unknown_choices_kept() {
        let text = "scale = 9\ntheme = \"neon\"\n";
        let values = from_toml(text);
        assert_eq!(values[index("scale").unwrap()], Value::Num(2.0));
        assert_eq!(values[index("theme").unwrap()], Value::Choice(0));
    }
}
