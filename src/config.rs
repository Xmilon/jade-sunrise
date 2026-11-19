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
//!
//! Names follow Jade's own config ids ([`Jade.java`], `WailaConfig`, and the `addConfig` entries of
//! its plugins): the main options after Jade's JSON fields (`general`, `overlay`; snake case), the
//! rest after Jade's plugin config ids (`minecraft:harvest_tool.effective_tool` and
//! `jade:registry_name` become `harvest_tool.creative` and `registry_name`), grouped in one
//! `plugin` section as Jade groups them under Plugin Settings. Jade keeps its main options in
//! `jade.json` and its plugin options in the same file's `plugin` object; this port keeps one TOML
//! (the README's decision), so the sections here are `general`, `overlay` and `plugin`.
//!
//! Options marked *port-only* exist here but not in Jade: Jade shows that behaviour always, or has
//! no such line. Options marked *not supported* exist in Jade but this port cannot act on them
//! (the mod API lacks the reads or hooks); they are kept for parity of the option list and do
//! nothing. Options marked *providers* are honoured once `src/providers.rs` reads them.

use mod_api::*;
use std::sync::Mutex;

pub const FILE: &str = "jade-client.toml";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Toggle(bool),
    /// Options (their file names, also the end of their translation keys) and the default index.
    /// The order is Jade's enum order where the port's readers allow it.
    Choice(&'static [&'static str], usize),
    /// min, max, step, default.
    /// `clamp` says an out-of-range value in the file is clamped into range (Jade's plain-float
    /// fields with clamping setters: position and anchors); otherwise it falls back to the
    /// default, as Jade's `floatRange` codecs and its plugin entries do.
    Range(f64, f64, f64, f64, bool),
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
const fn c(
    section: &'static str,
    key: &'static str,
    options: &'static [&'static str],
    default: usize,
) -> Opt {
    Opt { key, section, kind: Kind::Choice(options, default) }
}
const fn r(section: &'static str, key: &'static str, min: f64, max: f64, step: f64, default: f64) -> Opt {
    Opt { key, section, kind: Kind::Range(min, max, step, default, false) }
}
/// A range whose file values clamp instead of falling back (Jade's overlay position and anchors).
const fn rc(section: &'static str, key: &'static str, min: f64, max: f64, step: f64, default: f64) -> Opt {
    Opt { key, section, kind: Kind::Range(min, max, step, default, true) }
}

/// Jade's enum orders, kept where the port's readers index them by number. Where Jade's order
/// differs (`DisplayMode` HOLD_KEY, TOGGLE, LITE; `FluidMode` NONE, ANY, FALLBACK;
/// `BossBarOverlapMode` NO_OPERATION, HIDE_BOSS_BAR, HIDE_TOOLTIP, PUSH_DOWN; `ModNameProvider`
/// ON, OFF, SMALLER), the port's order is kept because providers and the client read the values by
/// index; the difference is only the cycle order on the quick screen.
pub const DISPLAY_MODES: &[&str] = &["toggle", "hold_key", "lite"];
pub const FLUID_MODES: &[&str] = &["any", "none", "fallback"];
pub const THEMES: &[&str] = &["dark", "classic", "flat"];
pub const ICON_MODES: &[&str] = &["top", "centered", "inline", "hide"];
pub const MOD_NAME_MODES: &[&str] = &["on", "smaller", "off"];
/// Jade's `RegistryNameProvider.Mode`: ON, OFF, ADVANCED_TOOLTIPS. The port adds `details`
/// (port-only: show with Show Details held); Jade has no such mode.
pub const ID_MODES: &[&str] = &["off", "on", "details", "advanced_tooltips"];
/// Jade's four modes. Hide Boss Bar is last so files written before it still read the same index.
pub const BOSS_BAR_MODES: &[&str] = &["push_down", "no_operation", "hide_tooltip", "hide_boss_bar"];
/// Jade's `PerspectiveMode`: CAMERA, EYE. EYE starts the trace at the eye (client.rs).
pub const PERSPECTIVE_MODES: &[&str] = &["camera", "eye"];

/// Every option, in the order the settings page and the file list them. The order of the first two
/// groups is Jade's config screen order (WailaConfigScreen.createOptions); `plugin` groups the
/// plugin config entries as Jade's Plugin Settings screen does.
pub const OPTS: &[Opt] = &[
    // General (Jade: general).
    t("general", "display_tooltip", true),
    t("general", "display_entities", true),
    t("general", "display_bosses", true),
    t("general", "display_blocks", true),
    c("general", "fluid_mode", FLUID_MODES, 0),
    c("general", "display_mode", DISPLAY_MODES, 0),
    // not supported: Jade adds the mod name to item tooltips; the port has no item tooltip hook.
    t("general", "item_mod_name", true),
    t("general", "hide_from_guis", true),
    c("general", "boss_bar_overlap_mode", BOSS_BAR_MODES, 0),
    r("general", "extended_reach", 0.0, 20.0, 0.5, 0.0),
    // providers: EYE traces from the eye instead of the camera (client.rs).
    c("general", "perspective_mode", PERSPECTIVE_MODES, 0),
    // not supported: Jade's dev switch; the port's is the JADE_DEBUG environment variable.
    t("general", "debug", false),
    // not supported: Jade's camouflage (infested, powder snow, suspicious gravel) needs block
    // swaps the port does not do.
    t("general", "builtin_camouflage", true),
    t("general", "hide_from_tab_list", true),
    // Appearance (Jade: overlay).
    c("overlay", "active_theme", THEMES, 0),
    rc("overlay", "overlay_pos_x", 0.0, 1.0, 0.01, 0.5),
    rc("overlay", "overlay_pos_y", 0.0, 1.0, 0.01, 1.0),
    rc("overlay", "overlay_anchor_x", 0.0, 1.0, 0.05, 0.5),
    rc("overlay", "overlay_anchor_y", 0.0, 1.0, 0.05, 0.0),
    r("overlay", "overlay_scale", 0.2, 2.0, 0.05, 1.0),
    r("overlay", "auto_scale_threshold", 0.0, 1.0, 0.05, 0.4),
    r("overlay", "alpha", 0.0, 1.0, 0.05, 0.7),
    c("overlay", "icon_mode", ICON_MODES, 0),
    t("overlay", "animation", true),
    // In ticks (Jade compares the value against partial-tick accumulation, 50 ms per 1.0).
    // Jade's file range is 0..infinity; 5 keeps the quick screen usable.
    r("overlay", "disappearing_delay", 0.0, 5.0, 0.1, 0.0),
    t("overlay", "flip_main_hand", false),
    // Providers (Jade: the plugin config, "Core Features", "Debug" and the Minecraft groups).
    // providers: Jade's Target Name toggle; the name line shows until providers.rs honours it.
    t("plugin", "object_name", true),
    c("plugin", "mod_name", MOD_NAME_MODES, 0),
    // not supported: Jade translates mod names through its language files; the port reads
    // package names and has no translation table for them.
    t("plugin", "mod_name.translated", true),
    t("plugin", "distance", false),
    t("plugin", "coordinates", false),
    // providers: relative coordinates while Ctrl is held, as Jade.
    t("plugin", "coordinates.rel", false),
    t("plugin", "block_face", false),
    c("plugin", "registry_name", ID_MODES, 0),
    // not supported: Jade's special ids (block entity type, fluid, poi, painting variant) need
    // client reads the API lacks (API-GAPS #10).
    t("plugin", "registry_name.special", false),
    t("plugin", "block_states", false),
    t("plugin", "block_properties", false),
    t("plugin", "harvest_tool", true),
    t("plugin", "harvest_tool.creative", false),
    t("plugin", "harvest_tool.effective_tool", true),
    t("plugin", "harvest_tool.new_line", false),
    t("plugin", "harvest_tool.show_unbreakable", false),
    t("plugin", "breaking_progress", true),
    t("plugin", "crop_progress", true),
    t("plugin", "redstone", true),
    // port-only: Jade's hopper-lock line has no toggle (its provider is required) and shows
    // whenever Redstone Info or it applies.
    t("plugin", "hopper_lock", true),
    t("plugin", "tnt_stability", true),
    t("plugin", "furnace", true),
    t("plugin", "brewing_stand", true),
    t("plugin", "beehive", true),
    t("plugin", "jukebox", true),
    t("plugin", "note_block", true),
    t("plugin", "mob_spawner", true),
    t("plugin", "command_block", true),
    t("plugin", "lectern", true),
    t("plugin", "campfire", true),
    // port-only: Jade shows a dropped item's name and count always; its closest option,
    // item_tooltip ("Item Description"), is not ported (needs item tooltip reads).
    t("plugin", "item_entity", true),
    // Containers (Jade: minecraft:item_storage.*).
    t("plugin", "item_storage", true),
    r("plugin", "item_storage.items_per_line", 3.0, 27.0, 1.0, 9.0),
    r("plugin", "item_storage.normal_amount", 0.0, 54.0, 1.0, 9.0),
    r("plugin", "item_storage.detailed_amount", 0.0, 54.0, 1.0, 54.0),
    r("plugin", "item_storage.show_name_amount", 0.0, 9.0, 1.0, 5.0),
    t("plugin", "item_storage.sort", false),
    // Entities (Jade: minecraft:entity_health.*, potion_effects.*, and the mob options).
    t("plugin", "entity_health", true),
    r("plugin", "entity_health.max_for_render", 0.0, 200.0, 1.0, 40.0),
    r("plugin", "entity_health.icons_per_line", 5.0, 40.0, 1.0, 10.0),
    t("plugin", "entity_health.show_fractions", false),
    t("plugin", "entity_armor", true),
    // providers: armour turns into a number above this, as Jade.
    r("plugin", "entity_armor.max_for_render", 0.0, 200.0, 1.0, 20.0),
    t("plugin", "potion_effects", true),
    r("plugin", "potion_effects.limit", 1.0, 99.0, 1.0, 7.0),
    t("plugin", "animal_owner", true),
    t("plugin", "villager_profession", true),
    t("plugin", "mob_growth", true),
    t("plugin", "mob_breeding", true),
    t("plugin", "horse_stats", true),
    t("plugin", "item_frame", true),
    t("plugin", "painting", true),
    t("plugin", "armor_stand", true),
    t("plugin", "zombie_villager", true),
    t("plugin", "next_entity_drop", true),
];

/// The option keys the port used before it took Jade's names. `index` accepts them, so callers
/// in other files and old config files keep working.
const ALIASES: &[(&str, &str)] = &[
    ("enabled", "display_tooltip"),
    ("boss_bar_overlap", "boss_bar_overlap_mode"),
    ("hide_from_player_list", "hide_from_tab_list"),
    ("theme", "active_theme"),
    ("pos_x", "overlay_pos_x"),
    ("pos_y", "overlay_pos_y"),
    ("anchor_x", "overlay_anchor_x"),
    ("anchor_y", "overlay_anchor_y"),
    ("scale", "overlay_scale"),
    ("opacity", "alpha"),
    ("registry_id", "registry_name"),
    ("harvest_tool_creative", "harvest_tool.creative"),
    ("effective_tool", "harvest_tool.effective_tool"),
    ("harvest_tool_new_line", "harvest_tool.new_line"),
    ("show_unbreakable", "harvest_tool.show_unbreakable"),
    ("spawner", "mob_spawner"),
    ("items_per_line", "item_storage.items_per_line"),
    ("normal_amount", "item_storage.normal_amount"),
    ("detailed_amount", "item_storage.detailed_amount"),
    ("show_name_threshold", "item_storage.show_name_amount"),
    ("sort_by_count", "item_storage.sort"),
    ("max_health_for_render", "entity_health.max_for_render"),
    ("icons_per_line", "entity_health.icons_per_line"),
    ("show_fractions", "entity_health.show_fractions"),
    ("mob_effects", "potion_effects"),
    ("effects_limit", "potion_effects.limit"),
    ("next_drop", "next_entity_drop"),
];

/// Choice names the port used before: (option key, old name, new name). Old files read the same
/// value under the new name.
const CHOICE_ALIASES: &[(&str, &str, &str)] = &[("display_mode", "hold", "hold_key"), ("registry_name", "always", "on")];

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
        Kind::Range(_, _, _, d, _) => Value::Num(d),
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
    OPTS.iter()
        .position(|o| o.key == key)
        .or_else(|| ALIASES.iter().find(|(from, _)| *from == key).and_then(|(_, to)| index(to)))
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
            Kind::Range(min, max, step, d, _) => SettingSpec::range(opt.key, label.clone(), min, max, step, d),
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
            Kind::Range(_, _, _, d, _) => Value::Num(host.setting_f64(opt.key).unwrap_or(d)),
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
        Kind::Range(_, _, _, d, _) => Value::Num(host.setting_f64(key).unwrap_or(d)),
    };
    set(opt.key, value);
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
/// Keys with a dot (Jade's plugin id paths) are quoted, as TOML needs.
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
            (Kind::Choice(options, _), Value::Choice(i)) => format!(
                "\"{}\"",
                options.get(*i).copied().unwrap_or(options[0])
            ),
            (_, v) => v.setting_text(),
        };
        let key = if opt.key.contains('.') { format!("\"{}\"", opt.key) } else { opt.key.to_owned() };
        out.push_str(&format!("{key} = {text}\n"));
    }
    out
}

/// Parses the file. As Jade's codecs: the file stands for the whole set of options, a missing key
/// holds the default, and a bad or out-of-range value falls back to the default (Jade's
/// `optionalFieldOf`/`orElse`, and its plugin entries' value checks). The overlay position and
/// anchors clamp instead, as Jade's clamping setters do.
pub fn from_toml(text: &str) -> Vec<Value> {
    let mut values: Vec<Value> = OPTS.iter().map(default_of).collect();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, raw)) = line.split_once('=') else { continue };
        let (key, raw) = (key.trim().trim_matches('"'), raw.trim().trim_matches('"'));
        let Some(i) = index(key) else { continue };
        let opt = &OPTS[i];
        let parsed = match opt.kind {
            Kind::Toggle(d) => Some(Value::Bool(raw.parse::<bool>().unwrap_or(d))),
            Kind::Choice(options, d) => {
                let name = CHOICE_ALIASES
                    .iter()
                    .find(|(k, from, _)| *k == opt.key && *from == raw)
                    .map(|(_, _, to)| *to)
                    .unwrap_or(raw);
                Some(Value::Choice(options.iter().position(|o| *o == name).unwrap_or(d)))
            }
            Kind::Range(min, max, _, d, clamp) => match raw.parse::<f64>() {
                Ok(n) if n.is_finite() => {
                    let n = if n >= min && n <= max {
                        n
                    } else if clamp {
                        n.clamp(min, max)
                    } else {
                        d
                    };
                    Some(Value::Num(n))
                }
                _ => Some(Value::Num(d)),
            },
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
        v.write_file = text.trim().is_empty()
            || OPTS
                .iter()
                .any(|o| !text.contains(&format!("{} =", o.key)) && !text.contains(&format!("\"{}\" =", o.key)));
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
        values[index("overlay_scale").unwrap()] = Value::Num(1.25);
        values[index("display_tooltip").unwrap()] = Value::Bool(false);
        values[index("harvest_tool.creative").unwrap()] = Value::Bool(true);
        values[index("item_storage.show_name_amount").unwrap()] = Value::Num(7.0);
        let text = to_toml(&values);
        assert!(text.contains("display_mode = \"lite\""));
        assert!(text.contains("\"harvest_tool.creative\" = true"));
        assert_eq!(from_toml(&text), values);
    }

    #[test]
    fn bad_and_out_of_range_values_fall_back_to_the_default() {
        let text = "overlay_scale = 9\nactive_theme = \"neon\"\ndisplay_tooltip = maybe\n";
        let values = from_toml(&text);
        assert_eq!(values[index("overlay_scale").unwrap()], Value::Num(1.0));
        assert_eq!(values[index("active_theme").unwrap()], Value::Choice(0));
        assert_eq!(values[index("display_tooltip").unwrap()], Value::Bool(true));
        // The position fields clamp, as Jade's setters do.
        let text = "overlay_pos_x = 9\noverlay_anchor_y = -3\n";
        let values = from_toml(&text);
        assert_eq!(values[index("overlay_pos_x").unwrap()], Value::Num(1.0));
        assert_eq!(values[index("overlay_anchor_y").unwrap()], Value::Num(0.0));
    }

    #[test]
    fn the_old_keys_and_names_still_read() {
        let text = "enabled = false\nscale = 1.25\nopacity = 0.5\ndisplay_mode = \"hold\"\nregistry_id = \"always\"\nspawner = false\n";
        let values = from_toml(&text);
        assert_eq!(values[index("display_tooltip").unwrap()], Value::Bool(false));
        assert_eq!(values[index("overlay_scale").unwrap()], Value::Num(1.25));
        assert_eq!(values[index("alpha").unwrap()], Value::Num(0.5));
        assert_eq!(values[index("display_mode").unwrap()], Value::Choice(1));
        assert_eq!(values[index("registry_name").unwrap()], Value::Choice(1));
        assert_eq!(values[index("mob_spawner").unwrap()], Value::Bool(false));
    }
}
