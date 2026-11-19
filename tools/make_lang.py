#!/usr/bin/env python3
"""Writes assets/jade/lang/en_us.json: the option labels read from src/config.rs's table, plus the
tooltip's own strings. Run it after adding an option: `python3 tools/make_lang.py`.

Labels and choice names are Jade's own English (its assets/jade/lang/en_us.json) wherever Jade has
the option; port-only options keep the port's wording. Provider strings are added only when the
port's code uses them in a way Jade's value fits (its `%s`/`{0}` forms kept where the code
formats). Keys that are vanilla's (`jukebox_song.*.desc`, `merchant.level.*`, `painting.*`,
`block.minecraft.player_head.named`, `effect.duration.infinite`) stay out: the game's own language
already answers `host.translate` for them, as does Jade, whose lang file does not carry them
either."""
import json, pathlib, re

ROOT = pathlib.Path(__file__).resolve().parent.parent
src = (ROOT / "src/config.rs").read_text()

LABELS = {
    # General (Jade: config.jade.display_tooltip and friends).
    "display_tooltip": ("Enabled", "The global switch to enable or disable the tooltip."),
    "display_entities": ("Display Entities", "Describe mobs and other entities."),
    "display_bosses": ("Display Bosses", "Describe the Ender Dragon and the Wither."),
    "display_blocks": ("Display Blocks", "Describe the block under the crosshair."),
    "fluid_mode": ("Fluid Mode", "Any: fluids are targets. None: never. Fallback: only when no block is hit."),
    "display_mode": ("Display Mode", "Toggle: the Show Overlay key turns it on and off. Hold: shown while the key is held. Lite: only the main lines, the rest with Show Details held."),
    "item_mod_name": ("Mod Name on Item Tooltip", "Show which mod is an item from on its tooltip. The port has no item tooltip hook; this option does nothing."),
    "hide_from_guis": ("Hide from GUIs", "Hide the tooltip on most GUI displays."),
    "boss_bar_overlap_mode": ("Boss Bar Overlap", "What to do when a boss bar is where the tooltip goes. Push Down Tooltip: put the tooltip below the bars. Hide Tooltip: hide the tooltip. Hide Boss Bar: hide the bars it covers."),
    "extended_reach": ("Extended Reach", "Increase the distance limit to the target."),
    "perspective_mode": ("Perspective Mode", "Whether the ray starts at the camera or at the eye."),
    "debug": ("Debug Mode", "Jade's dev switch. The port's debug switch is the JADE_DEBUG=1 environment variable; this option does nothing."),
    "builtin_camouflage": ("Builtin Camouflage", "Jade swaps disguised blocks (infested, powder snow, suspicious gravel). The port has no block-swap hook; this option does nothing."),
    "hide_from_tab_list": ("Hide from Player List", "Hide the tooltip when displaying the player list."),
    # Appearance (Jade: overlay).
    "active_theme": ("Theme", "The box's look."),
    "overlay_pos_x": ("Position X", "Where the tooltip sits across the screen, 0 left to 1 right."),
    "overlay_pos_y": ("Position Y", "Where the tooltip sits up the screen, 0 bottom to 1 top."),
    "overlay_anchor_x": ("Anchor X", "Which point of the box sits at the position, across."),
    "overlay_anchor_y": ("Anchor Y", "Which point of the box sits at the position, down."),
    "overlay_scale": ("Display Scale", "The tooltip's size."),
    "auto_scale_threshold": ("Auto-Scale Threshold", "A tooltip taller than this share of the screen is shrunk."),
    "alpha": ("Background Opacity", "How opaque the box is."),
    "icon_mode": ("Icon Position", "Where the target's icon goes."),
    "animation": ("Overlay Animation", "Animate the overlay when it appears and disappears. Morph the box when size changes."),
    "disappearing_delay": ("Disappearing Delay", "Ticks (1/20 s) the tooltip stays after you look away."),
    "flip_main_hand": ("Flip Position with Main Hand", "Mirror tooltip position when main hand is left."),
    # Plugin settings (Jade: Core Features, Debug and the Minecraft plugin entries).
    "object_name": ("Target Name", "The target's name line."),
    "mod_name": ("Mod Name", "The last line: the mod the target comes from."),
    "mod_name.translated": ("Translate Mod Name", "Jade translates mod names through its language files; the port reads package names, so this option does nothing."),
    "distance": ("Distance", "Show the distance between the player and the target."),
    "coordinates": ("Coordinates", "Show the position of the target."),
    "coordinates.rel": ("Relative Coordinates", "Requires holding Ctrl."),
    "block_face": ("Block Face", "Show the face of the block the player touches."),
    "registry_name": ("Registry Name", "The target's id, like minecraft:stone."),
    "registry_name.special": ("Special IDs", "Show special IDs like BlockEntity and Fluid. The port has no block-entity or fluid reads; this option does nothing."),
    "block_states": ("Block States", "The block's states, with Show Details held."),
    "block_properties": ("Block Properties", "Destroy time, explosion resistance and more, with Show Details held."),
    "harvest_tool": ("Harvest Tool", "The tools that mine the block, and whether yours harvests it."),
    "harvest_tool.creative": ("Show in Creative Mode", "Show harvest information in creative or spectator mode as well"),
    "harvest_tool.effective_tool": ("Effective Tool", "Show the check or cross for blocks any tool can harvest too."),
    "harvest_tool.new_line": ("Start in New Line", "Put the tool icons on a line of their own."),
    "harvest_tool.show_unbreakable": ("Show \"Unbreakable\"", "Say when a block can't be broken."),
    "breaking_progress": ("Breaking Progress", "A bar along the box's bottom edge while you mine."),
    "crop_progress": ("Crop Growth", "How grown a crop is."),
    "redstone": ("Redstone", "Levers, repeaters, comparators and powered blocks."),
    "hopper_lock": ("Hopper Lock", "Say when a hopper's inventory is locked. Port-only: Jade's lock line has no toggle."),
    "tnt_stability": ("TNT Stability", "Warn player of special TNT that ignites when hit."),
    "furnace": ("Furnace", "A furnace's, smoker's or blast furnace's items and progress."),
    "brewing_stand": ("Brewing Stand", "Blaze powder left and the brewing time."),
    "beehive": ("Beehive", "Honey and bees in a hive or nest."),
    "jukebox": ("Jukebox", "The disc that plays."),
    "note_block": ("Note Block", "Its instrument and note."),
    "mob_spawner": ("Mob Spawner", "The mob a spawner makes, in its name."),
    "command_block": ("Command Block", "The command it runs."),
    "lectern": ("Lectern", "The book on it."),
    "campfire": ("Campfire", "What cooks on it and how long is left."),
    "item_entity": ("Dropped Item", "A dropped item's name and count. Port-only: Jade shows these always."),
    # Containers (Jade: minecraft:item_storage.*).
    "item_storage": ("Container Contents", "What is inside chests, barrels, hoppers and other containers."),
    "item_storage.items_per_line": ("Items per Line", "Icons in each row of a container's contents."),
    "item_storage.normal_amount": ("Normal Amount", "The amount of items to show when Show Details is not pressed."),
    "item_storage.detailed_amount": ("Detailed Amount", "The amount of items to show when Show Details is pressed."),
    "item_storage.show_name_amount": ("Show Name Threshold", "The maximum amount of items to show item names instead of just item icons."),
    "item_storage.sort": ("Sort by Count", "Most plentiful items first."),
    # Entities (Jade: minecraft:entity_health.*, potion_effects.* and the mob options).
    "entity_health": ("Entity Health", "Hearts, or a number for very healthy mobs."),
    "entity_health.max_for_render": ("Max Health for Render", "Above this much health, show a number instead of hearts."),
    "entity_health.icons_per_line": ("Icons per Line", "Hearts in each row."),
    "entity_health.show_fractions": ("Show Fractions", "Health as a decimal number, not rounded up."),
    "entity_armor": ("Entity Armor", "Armor points."),
    "entity_armor.max_for_render": ("Max Armors for Render", "Above this much armor, show a number instead of armor icons."),
    "potion_effects": ("Entity Effects", "The mob's active effects."),
    "potion_effects.limit": ("Effects Limit", "Effects listed at most."),
    "animal_owner": ("Animal Owner", "Who tamed it."),
    "villager_profession": ("Villager Profession", "A villager's job, and its level."),
    "mob_growth": ("Mob Growth", "Time until a baby grows up."),
    "mob_breeding": ("Mob Breeding", "In love, or time until it can breed again."),
    "horse_stats": ("Horse Stats", "Jump height, speed and a llama's strength."),
    "item_frame": ("Item Frame", "The item in a frame."),
    "painting": ("Painting", "A painting's title and artist."),
    "armor_stand": ("Armor Stand", "What an armor stand wears and holds."),
    "zombie_villager": ("Zombie Villager", "Time until a cured zombie villager converts."),
    "next_entity_drop": ("Next Entity Drop", "Show the time until the next entity drop, such as egg or armadillo scute."),
}

CHOICES = {
    "display_mode": {"toggle": "Toggle", "hold_key": "Hold", "lite": "Lite"},
    "fluid_mode": {"any": "Any", "none": "None", "fallback": "Fallback"},
    "boss_bar_overlap_mode": {"push_down": "Push Down Tooltip", "no_operation": "No Operation", "hide_tooltip": "Hide Tooltip", "hide_boss_bar": "Hide Boss Bar"},
    "active_theme": {"dark": "Dark", "classic": "Classic", "flat": "Flat"},
    "icon_mode": {"top": "Top", "centered": "Centered", "inline": "In-line", "hide": "Hide"},
    "mod_name": {"on": "On", "smaller": "Smaller", "off": "Off"},
    "registry_name": {"off": "Off", "on": "On", "details": "With Show Details", "advanced_tooltips": "Advanced Tooltips"},
    "perspective_mode": {"camera": "From Camera", "eye": "From Eye"},
}

# The quick screen's section titles (screen.rs translates config.jade.<section>).
SECTIONS = {"general": "General", "overlay": "Appearance", "plugin": "Plugin Settings"}

STRINGS = {
    "key.categories.jade": "Jade",
    "key.jade.config": "Open Config",
    "key.jade.show_overlay": "Show Overlay",
    "key.jade.toggle_liquid": "Toggle Fluid",
    "key.jade.narrate": "Narrate Target",
    "key.jade.show_details": "Show Details",
    "jade.config.title": "Jade: Click to change, right-click to go back, Shift steps faster",
    # Jade's own strings (its en_us.json), for the keys providers.rs looks up. Kept in the forms
    # the port's code formats: trf() fills %s, the locked-title code fills {0}.
    "jade.spawner": "%s (%s)",
    "jade.unbreakable": "Unbreakable",
    "jade.harvest_tool.unbreakable": "Unbreakable",
    "jade.crop_growth": "Growth",
    "jade.crop_mature": "Mature",
    "jade.lever.state": "State",
    "jade.on": "On",
    "jade.off": "Off",
    "jade.repeater.delay": "Delay",
    "jade.repeater.locked": "Locked",
    "jade.comparator.mode": "Mode",
    "jade.comparator.comparator": "Comparator",
    "jade.comparator.subtractor": "Subtractor",
    "jade.power": "Power",
    "jade.input_signal": "Input: %s",
    "jade.block_destroy_time": "Destroy time: %s",
    "jade.block_explosion_resistance": "Explosion resistance: %s",
    "jade.tnt.unstable": "Unstable",
    "jade.beehive.honey": "Honey",
    "jade.beehive.bees": "Bees",
    "jade.empty": "Empty",
    "jade.locked": "Inventory is locked",
    "jade.access.block.locked": "Locked {0}",
    "jade.not_generated": "What could it be?",
    "jade.count": "Count",
    "jade.infinite": "Infinite",
    "jade.owner": "Owner",
    "jade.profession": "Profession",
    "jade.zombie_conversion": "Conversion time",
    "jade.mob_growth": "Growing time",
    "jade.mob_breeding": "Breeding CD",
    "jade.mob_breeding.in_love": "In Love",
    "jade.mobgrowth.paused": "Paused",
    "jade.mobduplication.time": "Duplication CD: %s",
    "jade.next_egg": "Next egg",
    "jade.next_scute": "Next scute",
    "jade.horseStat.jump": "Jump height: %s",
    "jade.horseStat.speed": "Speed: %s",
    "jade.llamaStrength": "Strength: %s",
    "jade.blockpos": "%s %s %s",
    "jade.distance1": " [%s]",
    "jade.distance2": "Distance: %s",
}
# Jade's direction names (providers.rs looks up jade.<face> after the name).
for d, name in {"down": "Down", "up": "Up", "north": "North", "south": "South", "west": "West", "east": "East"}.items():
    STRINGS[f"jade.{d}"] = name
# Jade's note-block instrument names; the trumpet keys are the port's additions (not in Jade).
INSTRUMENTS = {
    "harp": "Harp", "basedrum": "Bass Drum", "snare": "Snare Drum", "hat": "Clicks and Sticks",
    "bass": "Bass", "flute": "Flute", "bell": "Bells", "guitar": "Guitar", "chime": "Chimes",
    "xylophone": "Xylophone", "iron_xylophone": "Vibraphone", "cow_bell": "Cow Bell",
    "didgeridoo": "Didgeridoo", "bit": "Square Wave", "banjo": "Banjo", "pling": "Electric Piano",
    "zombie": "Zombie", "skeleton": "Skeleton", "creeper": "Creeper", "dragon": "Dragon",
    "wither_skeleton": "Wither Skeleton", "piglin": "Piglin", "custom_head": "Custom Head",
}
for inst in INSTRUMENTS:
    STRINGS[f"jade.instrument.{inst}"] = INSTRUMENTS[inst]
for inst in ["trumpet", "trumpet_exposed", "trumpet_weathered", "trumpet_oxidized"]:
    STRINGS[f"jade.instrument.{inst}"] = inst.replace("_", " ").title()

# The option table: `t`/`c`/`r`/`rc(section, key, ...)`. Dotted keys (Jade's plugin id paths) included.
keys = re.findall(r'^\s*[tcr]{1,2}\("[a-z]+", "([a-z_.]+)"', src, re.M)
out = {f"config.jade.{section}": title for section, title in SECTIONS.items()}
for k in keys:
    label, tip = LABELS[k]
    out[f"config.jade.{k}"] = label
    out[f"config.jade.{k}.tooltip"] = tip
    for opt, name in CHOICES.get(k, {}).items():
        out[f"config.jade.{k}.{opt}"] = name
out.update(STRINGS)
path = ROOT / "assets/jade/lang/en_us.json"
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(json.dumps(out, indent=2, ensure_ascii=False) + "\n")
print(f"{len(keys)} options, {len(out)} strings -> {path.relative_to(ROOT)}")
