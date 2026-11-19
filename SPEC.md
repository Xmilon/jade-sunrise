# Jade — Behaviour Spec for Clean-Room Rewrite (Sunrise / Rust MC 26.3)

> **Historical.** This spec started the port as a clean-room rewrite. Since 2026-11-19 the port is matched against Jade's own source (26.3 Fabric, `7c57c36`) and adapts its code; see NOTICE. Where this spec and Jade's code disagree, Jade's code wins.

Behaviour specification of **Jade** by Snownee (HWYLA/WAILA successor), reverse-described from its public
distribution and behaviour for a clean-room reimplementation on a different engine.
**No Jade code or art is copied.** All numbers, strings and defaults below are observable behaviour/facts.

Researched against Jade **26.3.5** (the latest release on Minecraft **26.3**, 2026-10-02), repo
<https://github.com/Snownee/Jade> (default branch), Modrinth project `jade`
(<https://modrinth.com/mod/jade>), docs <https://jademc.readthedocs.io>.

Legend: **[core]** = required for parity with a first playable version; **[extra]** = later/nice-to-have.
Items marked *(unverified)* could not be confirmed from sources; do not guess further — pick a sensible
behaviour and note the divergence.

---

## 1. What Jade is (one paragraph)

A client-side HUD overlay ("the tooltip") that continuously ray-traces from the camera and shows a
small box near the top-centre of the screen describing **what you are looking at**: its item icon,
display name, owning mod, and a body of contextual facts (harvest tool, progress bars, container
contents, entity health, etc.) produced by a prioritised provider pipeline. Blocks, entities and
(optionally) fluids are targets. Many richer facts require a matching server-side component; Jade
ships both halves and degrades gracefully to a client-only feature set.

---

## 2. Overlay window — when, where, how it looks **[core]**

### 2.1 When it is shown
- A target must exist under the crosshair (see §5) and the global "Enabled" switch must be on.
- Display modes (config `displayMode`, default **Toggle**):
  - **Toggle** — Show-Overlay key (Numpad 1) toggles the overlay on/off. First-ever off shows a
    toast: "Jade is now closed / Press ⟨key⟩ again to reopen".
  - **Hold** — overlay visible only while Show-Overlay key is held.
  - **Lite** — overlay always on but only "headline" providers render (see §4.8); holding
    Show-Details reveals the rest; when hidden body content exists, a small animated "more info"
    arrow is drawn at the bottom-centre edge of the box (7×5 px sprite, "breathing" up/down,
    ~1 px amplitude, ~12 s period loop — a period of 12 *ticks* per cycle *unverified*).
- Hidden when any of: a GUI screen is open (config "Hide from GUIs", default **on**; chat, inventory
  and every other screen hide it; exception: Jade's own config preview screen, and "pin" mode);
  the HUD is hidden (**F1**); a full-screen HUD message/progress screen is up; the player list
  (**Tab**) is visible (config "Hide from Player List", default **on**); a boss bar occupies the
  tooltip position and Boss-Bar-Overlap = *Hide Tooltip*.
- Boss-Bar-Overlap modes (default **Push Down**): *No Operation* / *Hide Boss Bar* (hides the boss
  bar while tooltip shows) / *Hide Tooltip* / *Push Down* (tooltip is placed fully below the boss
  bar rect).
- Works in survival/adventure/creative/spectator; creative + spectator additionally suppress the
  harvest-tool line unless "Show in Creative Mode" is enabled.
- Refresh: target re-ray-traced and tooltip rebuilt every client tick; server facts re-requested at
  most every **250 ms** (client rate limiter).

### 2.2 Position & size
- Anchor arithmetic (GUI-scaled pixels): the tooltip's position point is
  `x = screenWidth × posX`, `y = screenHeight × (1 − posY)`; the box is then offset so that
  `anchorX × boxWidth` / `anchorY × boxHeight` sits at that point.
- Defaults: `posX 0.5, posY 1.0, anchorX 0.5, anchorY 0.0` ⇒ **top-centre**, box hanging *down*
  from the top edge. All four values are 0–1 floats, user-adjustable by dragging the box in a
  special "Adjust Position" mode of the config screen (or clicking a screen edge).
- **Mirror the position horizontally when the main hand is left** — accessibility option, default off.
- Scale: `overlayScale` default **1.0**, clamped 0.2–2.0. Auto-shrink: if `boxHeight × scale`
  exceeds `screenHeight × autoScaleThreshold` (default **0.4**), scale reduces to at most
  `threshold / boxHeight` (never below half the configured scale).
- A config screen offers a live **Preview** of the overlay (a fake grass-block tooltip) — first-time
  users see it once (persisted "seen preview" flag).

### 2.3 Appearance / theme system **[core for defaults; theme data-driving is extra]**
- The box is drawn as a **9-slice sprite**. Built-in themes (data-driven JSON; rewrite can hard-code
  equivalents):
  - **Dark** (id `jade:dark`, default) — looks exactly like a vanilla item tooltip: near-black
    background **#131313**, thin gradient border in the classic vanilla tooltip purple/blue style,
    rounded corners. Rendered as background+frame sprite pair, each drawn 9 px larger than the box
    on every side (border+margin lives in that 9 px apron).
  - **Classic** (`jade:waila`) — HWYLA look: background **#100010** (16,0,16), same tooltip-style frame.
  - **One Probe** (`jade:top`) — flat 82×82 nine-slice sprite, border 2 px, padding 5/4/5/4 px, square corners.
  - **Create** (`jade:create`) — factory-panel look (black panel with beige 48,45,42 edging), 100×100 nine-slice.
  - "Slim" style variants exist for Dark/Classic/Create (narrower 82×82 sprite, 1 px border).
- Content padding inside the box: default **3 px** on all sides (theme may override; One Probe uses 5/4).
- Final background alpha = `config alpha × fade`; config **Background Opacity** default **0.7** (0–1).
- Text colours come from a semantic palette (defaults):
  `normal #A0A0A0`, `info #FFFFFF`, `title #FFFFFF`, `success #55FF55`, `warning #FFC107`,
  `danger #FF5555`, `failure #AA0000`. Registry-ID line: **grey** (#555555-style `GRAY`); when a
  light colour scheme is detected, dark grey instead.
- Box progress bar (breaking progress, furnace-style box fills): a **1 px** horizontal strip along
  the bottom edge of the box, colour = semantic type colour (default palette mapped per message
  type), alpha-faded like the box; fades out over ~5 ticks (0.1 alpha/tick) when the progress source
  stops providing.

### 2.4 Animation
- Config "Overlay Animation" default **on**.
- Fade in/out: alpha ramps linearly at **0.6 / tick** (0.1 in debug mode) toward 0 or 1; below 0.1
  the overlay is skipped entirely.
- Size/position morph: when the target changes and the box size or origin differs, the box animates
  from old rect to new rect over **75 ms** (linear per-axis chase; alpha floor 0.55 during morph).
  With animation off, the box snaps instantly.
- "Disappearing Delay" (default **0 s**, 0–∞ float): when the target is lost, keep drawing the last
  tooltip for N seconds (linger).

### 2.5 Icon
- Target item icon rendered left of the text lines, 16 px nominal (item scale 1.0; some internal
  icons use 0.75×).
- Icon modes: **Top** (default; icon top-aligned, centred vertically only if taller than text),
  **Centered**, **In-line** (small 8 px icon drawn on the name line before the name), **Hide**.
- Optional theme "icon slot": icon drawn inside a nested box sprite with inflation padding.

---

## 3. Targeting rules **[core]**

- Ray-trace starts at the **camera** (perspective mode *Camera*, default; *Eye* uses the player's eye
  position — better accuracy while moving, needed for compatibility with freecam-type camera mods).
- Reach = player's own block/entity interaction range + **Extended Reach** config (default **0**,
  range 0–20 blocks). A server gamerule `jade:max_position_deviation` (default **21**, 0–1000)
  bounds how far the ray may deviate from the real view before the server refuses data requests
  (anti-freecam/x-ray); 0 disables extended reach server-side.
- Entities are hit-tested first: an entity is picked when the ray enters its AABB (small AABBs
  < 0.3 inflated to 0.3; starting inside counts as a hit), unless occluded by a block closer than
  the entity. Newly-spawned projectiles (≤ 10 ticks) are excluded; dropped items are excluded while
  the player is actively breaking a block.
- Blocks: outline-shape clip; fluids included per **Fluid Mode**: **Any** (default, fluids are
  valid targets), **None**, **Fallback** (only when nothing solid was hit).
- Boss entity targets are shown unless "Display Bosses" is off (default on).
- Server-side "hide/pick overrides" (datapack-driven re-targeting, e.g. camouflage) — **[extra]**;
  built-in "camouflage" retargeting flag defaults on.

---

## 4. Tooltip content — lines and built-in providers

### 4.1 Line assembly **[core]**
Providers run in priority order into three zones: **HEAD** (−10000…), **BODY** (0), **TAIL**
(+10000). Same-priority providers append to the same rendered line; providers may also *replace* the
name line (block-face, spawner rename) or insert elements into the first line (harvest tool).
Canonical default order of what you actually see, top to bottom:

1. **Item icon** (left column, per §2.5)
2. **Name line** — target display name, white (title colour); plus optional suffixes on this line:
   harvest-tool icons + can-harvest ✔/✕ (same line by default), block face " (East)" (when enabled),
   distance " [12]" (when enabled)
3. **Body providers** in priority order, one line each unless noted
4. **Health / armour rows** for entities (rendered between body and tail)
5. **Mod name line** — last line, mod display name (e.g. "Minecraft"), default style **blue + italic**
   (user-configurable style); "Smaller" mode renders it at 0.75× text scale; modes On / Smaller / Off
   (default On); translated display names preferred (option, default on)
6. **Registry ID line** (optional, see 4.3) — grey, e.g. `minecraft:gold_ore`; special extra IDs
   (BlockEntity/Fluid/POI/Painting variants) each on their own grey line, e.g. `BlockEntity: minecraft:chest`

### 4.2 Global switches
`Display Blocks` (default on), `Display Entities` (on), `Display Bosses` (on), `Fluid Mode` (Any).

### 4.3 Name, mod name, registry ID **[core]**
- **Name**: block/entity display name; custom-named entities show their custom name; nameables'
  custom names come from the server (double chests use the container's display name); unnamed
  untranslatable blocks fall back to pick-result item name, then the translation key literal.
  Falls back to the pick item for pick-override targets. Max name length 100 chars.
- **Mod name**: owning mod's display name of the block/entity (see 4.1 #5).
- **Registry ID**: mode Off (default) / Always / **Follow Advanced Tooltips** (shows when F3+Q
  advanced-tooltips option is on). Format: plain `namespace:path`. Optional "special IDs" adds
  `BlockEntity: <id>`, `Fluid: <id>`, `POI: <id>`, `Painting: <id>` lines (default off).

### 4.4 Built-in providers — blocks

Default states shown as **on/off**; all toggles are per-provider config booleans unless stated.

| Provider | Default | Behaviour / exact text |
|---|---|---|
| **Harvest tool** `[core]` | **on** | Right after name: one small (0.75×, 10 px) tool icon per effective tool class (pickaxe/axe/shovel/hoe tiers wood→netherite; shears; sword lists wooden only). If the tool matters (`requiresCorrectToolForDrops`) or "Effective Tool" option (default **on**) is set: a ✔ (success green) or ✕ (danger red) glyph at 0.75× indicates whether the current tool can harvest. If block unbreakable and "Show 'Unbreakable'" (default **off**): red "Unbreakable" line. Hidden in creative/spectator unless "Show in Creative Mode" (**off**). Option "Start in New Line" (**off**) moves icons to their own line. |
| **Breaking progress** `[core]` | **on** | While the player mines the block: box bottom-edge progress bar (see §2.3), coloured title/success when the player *can* harvest, failure-red otherwise; tracks the real server destroy progress + partial-tick prediction. |
| **Block states** `[extra]` | **off** (debug) | One grey line per non-default blockstate property, `key: value`. |
| **Block properties** `[extra]` | **off** (debug) | Lines like `Explosion resistance: 6`, `Destroy time: …`, `Jump factor`, `Speed factor`, `Ignite odds`, `Burn odds`. |
| **Loot table** `[extra]` | **off** (debug, server) | `Loot Table: <id>`, `Seed: <n>`; requires OP. |
| **Crop progress** `[core]` | **on** | `Growth: 47%` (percent in info-white); mature ⇒ `Growth: Mature` (green). Works for crops (age/maxAge) and any block with an AGE_2/3/4/5/7/15 property; farmland-hydrated special cases treat as mature. |
| **Redstone info** `[core]` | **on** | Lever ⇒ `State: On/Off` (green/red). Repeater ⇒ `Delay: <1-4>`. Comparator ⇒ `Mode: Comparator/Subtractor` + `Power: <n>` (server data). Calibrated sculk sensor ⇒ `Input: <n>`. Any block with a POWER property ⇒ `Power: <n>` (client-visible states only). Redstone wire is not otherwise shown *(wire power line unverified)*. |
| **Hopper lock** `[extra]` | **on** | Locked hoppers (NBT-locked) show `Inventory is locked`; suppressed when redstone info already shows. |
| **Furnace contents** `[core]` | **on** | Single line: input item icon, fuel item icon, **arrow progress element** (cooking progress, ~3 px side padding), output item icon. Shown only when any slot is non-empty. Data from server. |
| **Brewing stand** `[core]` | **on** | Blaze-powder icon + fuel count (info), then clock icon + remaining brew time as `1m 23s`-style short duration. Server data. |
| **Beehive** `[core]` | **on** | `Honey: 3 / 5` (info; green when full 5) and `Bees: 3 / 3` (green when full). Server data. |
| **Jukebox record** `[core]` | **on** | Empty ⇒ `Empty`; with disc ⇒ vanilla "Now Playing: <song>" line. Server data. |
| **Note block** `[core]` | **on** | Translated instrument name + note, e.g. `Harp <note>` (client state only; exact note notation *(unverified)* — pick e.g. `F♯4` style). |
| **Command block** `[extra]` | **on** | `> <command>` (command string from server; needs server Jade). |
| **Mob spawner** `[core]` | **on** | **Replaces the name line** with `<Mob Name> (Mob Spawner)` — i.e. spawned mob display name + container type; trial spawner cooldown adds `Cooldown: <time>`; works for spawner minecarts too. Mob identity needs server Jade for non-cage cases; name itself resolves client-side. |
| **Enchantment power** `[extra]` | **on** | Bookshelf-style blocks near enchanting reach: `Ench Power: 1.0` (info). Value = top-level boost of the block itself. |
| **Total enchantment power** `[extra]` | **on** | Looking at an enchanting table: `Ench Power: 15` — total bookshelf power around it (info). |
| **Player head** `[extra]` | **on** | Skulls render their owner's player **face texture as the icon** (replaces block icon). |
| **Item BERT icons** `[extra]` | **on** | Decorated pots / skulls use their block-entity renderer output as the tooltip icon. |
| **Shelf** `[extra]` | **on** | Shows stored items like a mini container; Shift shows per-slot detail. |
| **Lectern** `[extra]` | **on** | Book icon + book title (strip colour). Server data. |
| **Campfire** `[extra]` | **on** | Food items on campfire with remaining cook time as amount text (e.g. `45s`). |
| **TNT stability** `[extra]` | **on** | `Unstable` (danger red) for TNT that ignites when hit. |
| **Waxed state** `[extra]` | **on** | Waxed signs / copper golems: honeycomb prefix icon on the name line ("Waxed <name>" narration); normal (unwaxed) shows nothing. |
| **Sulfur cube** `[extra]` | **on** | Charge state display for the 26.x sulfur/cube mob *(exact text unverified)*. |
| **Fluid target** (vanilla fluids as target) `[core]` | on | Name + mod name like a block; body typically empty. |

### 4.5 Built-in providers — entities

| Provider | Default | Behaviour / exact text |
|---|---|---|
| **Entity health** `[core]` | **on** | Row of vanilla **hearts** (9×9 sprites, 8 px pitch, wrapping). Defaults: max 20 (10 hearts); **> 40 HP total (incl. absorption) switches to text** `12,345 / 20,000` (comma-grouped). "Icons per Line" = 10 (5–40); "Max Health for Render" = 40 (0–200); "Show Fractions" = off (ceil HP in text mode). Absorption hearts drawn yellow; frozen entity shows frozen heart type; hearts **blink white** on damage (recent-change tracking). Narration: `Health: <n>`. Server data for absorption. |
| **Entity armour** `[core]` | **on** | Vanilla armour icons row, same wrapping/pitch; only when armour > 0; **> 20 switches to a plain number** `12` (comma-grouped). Health and armour rows are joined on one line with a 4 px gap when both render as text. |
| **Mob effects** `[extra]` | **on** | Nested sub-box listing up to **7** (option `Effects Limit`, 1–99) visible effects: `Speed (II) 1:23` — name + Roman/numeric amplifier + duration (or `Infinite`), text tinted by effect category (harmful ⇒ red tones). Server data; hidden effects excluded. |
| **Horse stats** `[extra]` | **on** | `Jump height: 1.1` and `Speed: 0.3` (info, comma-grouped decimals); Shift shows `value / max` fraction form; llamas get `Strength: 3 / 5`. Client data. |
| **Animal owner** `[extra]` | **on** | `Owner: <player name>` when tamed (server data resolves the UUID→name). |
| **Mob growth** `[extra]` | **on** | Babies: `Growing time: 45s` (short duration, info); frozen (no-tick) shows `Paused`. |
| **Mob breeding** `[extra]` | **on** | In love ⇒ `Breeding CD: In Love`; cooling down ⇒ `Breeding CD: 30s`; allay duplication ⇒ `Duplication CD: 12s`. |
| **Villager profession** `[core]` | **on** | `Profession: Farmer` (profession display name; unemployed/nitwit named too). Client data. |
| **Villager restock** `[extra]` | **on** | `Ready to restock` (green) / `Restock in 45s` / `Restock limit reached, next in 2m`. Server data. |
| **Zombie villager conversion** `[extra]` | **on** | `Conversion time: 30s`. Server data. |
| **Next entity drop** `[extra]` | **on** | Chicken `Next egg: 1m`; armadillo `Next scute: 30s`; sniffer `Dig CD: 8s`. Server data. |
| **Pet armour** `[extra]` | **on** | Small armour item icon + name (+ durability narration); modes Show All / Show Damageable (default **Show Damageable**). Server data. |
| **Item frame / glazed item** `[core]` | **on** | Adds the framed item's display name as a line (invisible frames included). Client data. |
| **Painting** `[extra]` | **on** | Painting variant name as a line. |
| **Armour stand** `[extra]` | **on** | Lists equipped items (icon + name rows). |
| **Item entity / item display** `[core]` | **on** | Ground items show the item name; hovered item tooltip text can be appended ("Item Description", **on**, truncated to width with `..`). |
| **Container entity** (minecart chest etc.) `[extra]` | **on** | Uses the item-storage provider (below) on entities. |
| **Falling block** `[extra]` | **on** | Falling-block entity icon replaced by the contained block icon. |
| **Entity variants/description** (access plugin) | see §9 | |

### 4.6 Universal providers (modded containers, cross-cutting) **[core for vanilla surfaces]**

| Provider | Default | Behaviour / exact text |
|---|---|---|
| **Item storage** (container contents preview) | **on** | Grid of item icons, 9 per line (option `Items per Line`, 3–27). **Normal: max 9** stacks (`Normal Amount`, 0–54); **Shift: max 54** (`Detailed Amount`, 0–54). If ≤ **5** distinct stacks (`Show Name Threshold`), rows render as `<icon>  12× Dirt` with count-column alignment (counts via human-readable suffixes k/M/B beyond 10 000; `99+` capping at 99+ per stack sprite). "Sort by Count" (**off**). Chests with ungenerated loot show italic *"What could it be?"*; locked containers show `Inventory is locked`; **ender chests show the player's own ender chest**; armour stands excluded; furnaces excluded (own provider). Multi-group inventories (modded) get a horizontal divider + 0.5× group title header. Server data (contents snapshot) — the defining server-side feature. |
| **Fluid storage (tanks)** | **on** | Display styles: **Progress Bar** (default; nested-box bar with fill ratio + `Water: 900 mB` text overlay), **Icon** (bucket icon + text), **Plain Text**. Normal mode shows `900 mB` only; Shift (or non-bar style) shows `900 mB / 1000 mB`. Empty tank ⇒ `Empty`. Extra hidden tanks ⇒ `2 more tanks`. Bucket = 1000 mB. "Detailed-Only" option (**off**) hides entirely unless Shift. Server data. |
| **Energy storage** (modded FE etc.) | **on** | Same three styles; default progress bar; text `<cur> / <max>` (`jade.fe` = `%s / %s`); "Detailed-Only" (**off**). Server data; unit string comes from the platform integration — for the rewrite, pick `FE`. |
| **Progress** (universal machine progress arrow) | **on** | Generic progress arrow element for modded machines that expose a progress ratio; rendered like the furnace arrow. Server data. |

### 4.7 Distance / coordinates / block face **[extra]**
- **Distance** (**off**): appended to the name line as ` [7]` (metres, 1 decimal); alternative standalone `Distance: 7.2` form *(which form is used in which mode unverified — implement the suffix form)*.
- **Coordinates** (**off**): target block position as `123 64 -321` line; **Relative Coordinates** (**off**) variant while holding Ctrl; narration format `X {0} Y {1} Z {2}`.
- **Block face** (**off**): appended to name: ` (East)` / North/South/East/West/Up/Down.

### 4.8 "Lite" display-mode filtering **[extra]**
In Lite mode only providers with priority magnitude > 5000 render without Shift: name, health (−8000),
armour (−4499), harvest tool (+8000), mod name (+9999). Everything else is Shift-gated, with the
breathing arrow indicator (§2.1) when hidden content exists.

---

## 5. Key bindings (defaults) **[core]**

Category "Jade". Defaults (rebindable; shown in Options → Controls):

| Action | Default key |
|---|---|
| Open Config | **Numpad 0** |
| Show Overlay (toggle / hold, per Display Mode) | **Numpad 1** |
| Toggle Fluid (cycles fluid mode) | **Numpad 2** |
| Show Recipes (recipe-viewer integration) | **Numpad 3** |
| Show Uses (recipe-viewer integration) | **Numpad 4** |
| Narrate Target (TTS press mode) | **Numpad 5** |
| Show Details (Shift-held detail pass; also used as sneak-style gate) | **Left Shift** |
| Use Profile #0–#3 | unbound |

Narration on toggle keys: "Overlay shown/hidden", "Fluid shown/hidden". Toggling narrator first time
shows a toast ("Jade's narrator is now open / Press ⟨key⟩ again to disable").

---

## 6. Configuration **[core]**

JSON config file (client), sections and defaults:

- **General**: Enabled (on) · Display Blocks (on) · Display Entities (on) · Display Bosses (on) ·
  Display Mode (Toggle) · Fluid Mode (Any) · Boss Bar Overlap (Push Down) · Perspective (Camera) ·
  Extended Reach (0, 0–20) · Item Mod Name on Item Tooltips (on) · Hide from Player List (on) ·
  Hide from GUIs (on) · Debug Mode (off).
- **Appearance**: Theme (Dark; Classic/One Probe/Create + slim styles) · Overlay Position (0.5, 1.0)
  + drag-to-adjust · Overlay Anchor (0.5, 0.0) · Display Scale (1.0, 0.2–2.0) · Auto-Scale
  Threshold (0.4) · Background Opacity (0.7) · Icon Position (Top) · Overlay Animation (on) ·
  Disappearing Delay (0 s).
- **Formatting**: Mod-Name style (blue + italic).
- **Accessibility**: Text to Speech (off; mode Toggle/Press) · Enable Accessibility Plugin
  (off; auto-enabled when a screen-reader/accessibility mod is detected) · Text Background Opacity
  (0) · Flip Position with Main Hand (off) · Narrate Key Binds (off) · Always Use List Mode (off).
- **Profiles**: Enable Profiles (off; 4 profiles #0–#3; presets "Accessibility Mode", "Minimalism";
  key binds not saved per profile).
- **Plugin Settings**: per-provider toggles/limits as listed in §4 (surfaced grouped by category
  "Minecraft – Block", "Minecraft – Entity", "Core Features", "Debug", "Accessibility"…).
- Config GUI: home screen with category cards, **search box**, live preview, per-section screens,
  "Save & Quit", Danger Zone (Reset Settings with confirm; Reload Plugins), credits button
  ("Made with ♥ by Snownee" style attribution — replace with Sunrise's own), list/card layout toggle.
- Server-controlled plugin options render as "This option is controlled by the server".

---

## 7. Server-side data **[core: degradation; extra: full protocol]**

### 7.1 Which facts need the server
Anything not derivable from client-visible state: **container/item-storage contents**, **fluid tanks**,
**energy**, **furnace cooking progress**, **brewing fuel/time**, **beehive honey/bees**, **jukebox disc**,
**lectern book**, **command block command**, **comparator/calibrated-sensor signal**, **hopper lock**,
**entity absorption**, **mob effects**, **owner names**, **growth/breeding/restock/drop timers**,
**zombie-villager conversion**, **pet armour**, **trial-spawner cooldown**, **loot-table debug**.
Client-only (work everywhere): names, mod names, block states, redstone states, note block, harvest
tool + ✔/✕, crop %, entity health/armour (except absorption), horse stats, item frame, painting,
profession, breaking progress, enchanting power, coords/distance.

### 7.2 Protocol (when both sides run Jade)
1. On join, client sends handshake with a protocol version string. Mismatch ⇒ server chat message:
   "You are using a different version of Jade than the server. Please update Jade to <ver>." and no
   further data flow.
2. Server replies handshake: server plugin-config **overrides** (forced on/off options from
   `config/jade/server-plugin-overrides.json`, generable with a command from a regex pattern),
   a synced list of shearable blocks (derived from loot tables), and the registry of server data
   provider IDs.
3. Client requests data for the current target at most every **250 ms**, listing wanted provider IDs;
   server rate-limits per player, verifies the player can actually see the target within the
   `jade:max_position_deviation` gamerule distance, and answers one compound payload; data is
   validated against the target position on arrival.
4. Server operators can force-show/hide the overlay per player: `/jade show|hide` pushes a config
   change to the client.

### 7.3 Without server Jade
Overlay works in pure client-only mode: only client-side providers render; server-gated facts are
simply absent (no placeholders). The config screen marks server-required features with
"This feature requires the server to have Jade installed". Recommend the rewrite do the same, with
an optional Sunrise-native protocol for its own server.

---

## 8. Accessibility **[extra]**

- Narrator: full tooltip narration (name, all lines, fractions spoken as "one half", "two thirds";
  numbers with units "5 Buckets"; positions; distances "D12"). Toggle or press mode; min interval
  500 ms with de-duplication.
- Accessibility plugin (auto-on with accessibility mods installed): rewrites the target name to a
  descriptive sentence — "Closed Oak Door", "Waterlogged …", "Lit …", "Charged Creeper", "Baby Pig",
  "White Horse", "Small Slime", pose names ("Sleeping", "Sitting"), held items
  ("Main Hand: Iron Sword"), leashed-to, passengers, sign contents line by line, entity variant
  names; adds block details (facing, layers, bites, hatch, rail shapes…).
- Always-list-mode config screen layout; flip position for left hand.

---

## 9. Debug & commands **[extra]**

- Debug Mode config flag: slows fades to 0.1/tick; **Ctrl+hover** draws element layout boxes
  (green child borders, magenta boss-bar outline, red border around failed elements) — great for
  porting/verification.
- Commands: `/jadec handlers` (dump handler registration), `/jadec config`, `/jadec use_profile <0-3>`,
  `/jadec pin` (freeze the overlay, make it mouse-interactive at cursor), `/jadec generate_server_overrides <regex>`;
  server: `/jade show|hide`.
- Ignore list: JSON list of registry IDs to never target.
- Debug providers (off by default): registry name lines, block states, block properties, loot table
  (OP only).

---

## 10. Licence / legal notes for the rewrite **[core]**

- Jade **code** and **assets** are licensed **CC-BY-NC-SA-4.0** (repo `LICENSE.md`; Modrinth agrees).
  Consequences for Sunrise:
  1. **Do not copy code or art** (this spec is behavioural, which is fine). Implement from this
     document, not from Jade sources. Avoid decompiling Java for reference.
  2. CC-BY-NC-SA is **non-commercial**: a downstream that *did* copy must be non-commercial and
     share-alike — safest to treat anything Jade-derived as off-limits for a commercially licensed game.
  3. Do **not** reuse Jade's sprites (`waila*.png`, `top.png`, popup/nested-box/progress textures) or
     logo. Draw new 9-slice art; replicate only *dimensions* and *colours* documented above (facts are
     not copyrightable, but stay original in execution).
  4. Vanilla-derived facts (heart/armour sprite metrics, vanilla tooltip style, "Now Playing" phrasing)
     belong to the game, not Jade — mirror freely.
  5. Jade embeds permissively-licensed ideas from The One Probe, Create, Modern Industrialization, JEI
     (see its `licenses/` folder); their *concepts* (progress arrows, tank bars) are unprotectable, but
     again, implement fresh.
  6. Attribution: do not brand the rewrite as Jade or imply endorsement; credit "behaviour spec after
     Jade by Snownee (CC-BY-NC-SA-4.0)" in docs if desired.
  7. Jade's own source header warns its license is deliberately incompatible with some open-source
     licenses — another reason to keep Sunrise independent.

---

## 11. Unverified / open points
- Exact vanilla-tooltip **border gradient colours** of the Dark theme (sprite frame is a tinted
  texture; matches vanilla tooltip border visually — sample in-game when porting).
- Note-block note notation, distance suffix vs standalone form usage, sulfur-cube text, exact
  sneaky-arrow period units (ticks vs seconds).
- "Overlay hidden while any HUD overlay is active": implemented via the engine's overlay container;
  on Sunrise, mirror as "hide on death screen / any modal HUD layer".
- Recipe-viewer integrations (Show Recipes/Uses keys) are mod-ecosystem features — out of scope for
  Sunrise v1; keep the key binds reserved.

## 12. Suggested v1 parity cut **[core set]**
Overlay box (top-centre, dark tooltip style, 0.7 alpha, 75 ms morph + fade), icon + name + mod name
lines, harvest tool icons + ✔/✕, crop %, redstone basics, breaking-progress bar, furnace row,
container contents grid (9 icons, 5-name threshold), entity hearts/armour with 40/20 text switch,
mob effects (7 max), profession, item frame/painting names, spawner rename, keybinds per §5, config
options per §6, fluid-mode targeting, graceful no-server degradation. Everything else in this doc is
**[extra]**.
