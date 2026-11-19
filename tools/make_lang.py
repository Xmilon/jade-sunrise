#!/usr/bin/env python3
"""Writes assets/jade/lang/en_us.json: the option labels read from src/config.rs's table, plus the
tooltip's own strings. Run it after adding an option: `python3 tools/make_lang.py`."""
import json, pathlib, re

ROOT = pathlib.Path(__file__).resolve().parent.parent
src = (ROOT / "src/config.rs").read_text()

LABELS = {
    "enabled": ("Enabled", "Show the overlay at all."),
    "display_blocks": ("Display Blocks", "Describe the block under the crosshair."),
    "display_entities": ("Display Entities", "Describe mobs and other entities."),
    "display_bosses": ("Display Bosses", "Describe the Ender Dragon and the Wither."),
    "display_mode": ("Display Mode", "Toggle: the Show Overlay key turns it on and off. Hold: shown while the key is held. Lite: only the main lines, the rest with Show Details held."),
    "fluid_mode": ("Fluid Mode", "Any: fluids are targets. None: never. Fallback: only when no block is hit."),
    "boss_bar_overlap": ("Boss Bar Overlap", "What to do when a boss bar is where the tooltip goes. Push Down: put the tooltip below the bars. Hide Tooltip: hide the tooltip. Hide Boss Bar: hide the bars it covers."),
    "extended_reach": ("Extended Reach", "Blocks and entities beyond your reach that the overlay still describes."),
    "hide_from_guis": ("Hide from GUIs", "Hide the overlay while a screen is open."),
    "hide_from_player_list": ("Hide from Player List", "Hide the overlay while the player list is shown."),
    "theme": ("Theme", "The box's look."),
    "pos_x": ("Position X", "Where the tooltip sits across the screen, 0 left to 1 right."),
    "pos_y": ("Position Y", "Where the tooltip sits up the screen, 0 bottom to 1 top."),
    "anchor_x": ("Anchor X", "Which point of the box sits at the position, across."),
    "anchor_y": ("Anchor Y", "Which point of the box sits at the position, down."),
    "scale": ("Display Scale", "The tooltip's size."),
    "auto_scale_threshold": ("Auto-Scale Threshold", "A tooltip taller than this share of the screen is shrunk."),
    "opacity": ("Background Opacity", "How opaque the box is."),
    "icon_mode": ("Icon Position", "Where the target's icon goes."),
    "animation": ("Overlay Animation", "Fade the overlay and morph the box when the target changes."),
    "disappearing_delay": ("Disappearing Delay", "Seconds the tooltip stays after you look away."),
    "flip_main_hand": ("Flip with Main Hand", "Mirror the position when your main hand is the left one."),
    "mod_name": ("Mod Name", "The last line: the mod the target comes from."),
    "registry_id": ("Registry Name", "The target's id, like minecraft:stone."),
    "harvest_tool": ("Harvest Tool", "The tools that mine the block, and whether yours harvests it."),
    "harvest_tool_creative": ("Harvest Tool in Creative", "Show the harvest tool in creative and spectator mode too."),
    "effective_tool": ("Effective Tool", "Show the check or cross for blocks any tool can harvest too."),
    "harvest_tool_new_line": ("Harvest Tool on New Line", "Put the tool icons on a line of their own."),
    "show_unbreakable": ("Show Unbreakable", "Say when a block can't be broken."),
    "breaking_progress": ("Breaking Progress", "A bar along the box's bottom edge while you mine."),
    "block_states": ("Block States", "The block's states, with Show Details held."),
    "block_properties": ("Block Properties", "Explosion resistance and destroy time, with Show Details held."),
    "crop_progress": ("Crop Growth", "How grown a crop is."),
    "redstone": ("Redstone", "Levers, repeaters, comparators and powered blocks."),
    "furnace": ("Furnace", "A furnace's, smoker's or blast furnace's items and progress."),
    "brewing_stand": ("Brewing Stand", "Blaze powder left and the brewing time."),
    "beehive": ("Beehive", "Honey and bees in a hive or nest."),
    "jukebox": ("Jukebox", "The disc that plays."),
    "note_block": ("Note Block", "Its instrument and note."),
    "spawner": ("Mob Spawner", "The mob a spawner makes, in its name."),
    "command_block": ("Command Block", "The command it runs."),
    "lectern": ("Lectern", "The book on it."),
    "campfire": ("Campfire", "What cooks on it and how long is left."),
    "tnt_stability": ("TNT Stability", "TNT that goes off when punched."),
    "hopper_lock": ("Hopper Lock", "Say when a hopper's inventory is locked."),
    "block_face": ("Block Face", "The face you look at, after the name."),
    "coordinates": ("Coordinates", "The block's position."),
    "distance": ("Distance", "How far the block is, after the name."),
    "item_storage": ("Container Contents", "What is inside chests, barrels, hoppers and other containers."),
    "items_per_line": ("Items per Line", "Icons in each row of a container's contents."),
    "normal_amount": ("Normal Amount", "Kinds of items shown without Show Details."),
    "detailed_amount": ("Detailed Amount", "Kinds of items shown with Show Details held."),
    "show_name_threshold": ("Show Name Threshold", "With this many kinds or fewer, each gets a row with its name."),
    "sort_by_count": ("Sort by Count", "Most plentiful items first."),
    "entity_health": ("Entity Health", "Hearts, or a number for very healthy mobs."),
    "max_health_for_render": ("Max Health for Hearts", "Above this much health, show a number instead of hearts."),
    "icons_per_line": ("Hearts per Line", "Hearts in each row."),
    "show_fractions": ("Show Fractions", "Health as a decimal number, not rounded up."),
    "entity_armor": ("Entity Armor", "Armor points."),
    "mob_effects": ("Mob Effects", "The mob's active effects."),
    "effects_limit": ("Effects Limit", "Effects listed at most."),
    "animal_owner": ("Animal Owner", "Who tamed it."),
    "villager_profession": ("Villager Profession", "A villager's job (and level, with Show Details held)."),
    "mob_growth": ("Mob Growth", "Time until a baby grows up."),
    "mob_breeding": ("Mob Breeding", "In love, or time until it can breed again."),
    "horse_stats": ("Horse Stats", "Jump height, speed and a llama's strength."),
    "item_frame": ("Item Frame", "The item in a frame."),
    "item_entity": ("Dropped Item", "A dropped item's name and count."),
    "painting": ("Painting", "A painting's title and artist."),
    "armor_stand": ("Armor Stand", "What an armor stand wears and holds."),
    "zombie_villager": ("Zombie Villager", "Time until a cured zombie villager converts."),
    "next_drop": ("Next Drop", "Time to a chicken's next egg or an armadillo's next scute."),
}

CHOICES = {
    "display_mode": {"toggle": "Toggle", "hold": "Hold", "lite": "Lite"},
    "fluid_mode": {"any": "Any", "none": "None", "fallback": "Fallback"},
    "boss_bar_overlap": {"push_down": "Push Down", "no_operation": "No Operation", "hide_tooltip": "Hide Tooltip", "hide_boss_bar": "Hide Boss Bar"},
    "theme": {"dark": "Dark", "classic": "Classic", "flat": "Flat"},
    "icon_mode": {"top": "Top", "centered": "Centered", "inline": "In-line", "hide": "Hide"},
    "mod_name": {"on": "On", "smaller": "Smaller", "off": "Off"},
    "registry_id": {"off": "Off", "always": "Always", "details": "With Show Details", "advanced_tooltips": "Follow Advanced Tooltips"},
}

STRINGS = {
    "key.categories.jade": "Jade",
    "key.jade.config": "Open Config",
    "key.jade.show_overlay": "Show Overlay",
    "key.jade.toggle_liquid": "Toggle Fluid",
    "key.jade.narrate": "Narrate Target",
    "key.jade.show_details": "Show Details",
    "jade.config.title": "Jade: Click to change, right-click to go back, Shift steps faster",
    "jade.spawner": "%s (%s)",
    "jade.unbreakable": "Unbreakable",
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
    "jade.tnt.unstable": "Unstable",
    "jade.beehive.honey": "Honey",
    "jade.beehive.bees": "Bees",
    "jade.empty": "Empty",
    "jade.locked": "Inventory is locked",
    "jade.not_generated": "What could it be?",
    "jade.count": "Count",
    "jade.infinite": "Infinite",
    "jade.owner": "Owner",
    "jade.profession": "Profession",
    "jade.zombie_conversion": "Conversion time",
    "jade.mob_growth": "Growing time",
    "jade.mob_breeding": "Breeding CD",
    "jade.mob_breeding.in_love": "In Love",
    "jade.next_egg": "Next egg",
    "jade.next_scute": "Next scute",
    "jade.horse_jump": "Jump height",
    "jade.horse_speed": "Speed",
    "jade.llama_strength": "Strength",
    "jade.block_properties.explosion_resistance": "Explosion resistance",
    "jade.block_properties.destroy_time": "Destroy time",
}
for d in ["down", "up", "north", "south", "west", "east"]:
    STRINGS[f"jade.direction.{d}"] = d.capitalize()
for inst in ["harp", "basedrum", "snare", "hat", "bass", "flute", "bell", "guitar", "chime", "xylophone",
             "iron_xylophone", "cow_bell", "didgeridoo", "bit", "banjo", "pling", "zombie", "skeleton",
             "creeper", "dragon", "wither_skeleton", "piglin", "custom_head", "trumpet", "trumpet_exposed",
             "trumpet_weathered", "trumpet_oxidized"]:
    names = {"basedrum": "Bass Drum", "hat": "Hi-Hat", "bit": "Bit", "cow_bell": "Cow Bell", "pling": "Pling"}
    STRINGS[f"jade.instrument.{inst}"] = names.get(inst, inst.replace("_", " ").title())

keys = re.findall(r'^\s*[tcr]\("[a-z]+", "([a-z_]+)"', src, re.M)
out = {}
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
