# Jade for Sunrise

"What am I looking at": a small box at the top centre of the screen naming the block, fluid or mob
under the crosshair, the mod it comes from, and what is worth knowing about it. It covers the harvest
tool, breaking progress, crop growth, furnace progress, container contents, health, effects, owner,
profession and more. It is a **Sunrise native mod** (Rust, compiled against the game) and a port of
[Jade](https://github.com/Snownee/Jade) by Snownee.

**Licence.** Jade is licensed **CC BY-NC-SA 4.0**. This port adapts Jade's code: its targeting,
tooltip layout, providers, options and formats were translated from Jade's Java source (26.3 Fabric,
`7c57c36`) to Rust, and its English strings come from Jade's `en_us.json`. It contains no Jade art.
It is offered under the same CC BY-NC-SA 4.0 ([LICENSE](LICENSE), [NOTICE](NOTICE)). It is unofficial
and not endorsed by Snownee.

- **Game:** Sunrise 26.3, pinned to Nightly 2026-11-19 (`520fc48`, SDK release
  `sdk-520fc4882de3abb8ecd8b98293f4d7263bc88181`), the newest Nightly with a published mod SDK. Mod API
  line 0.2. (Earlier pin: Nightly 2026-11-17, `2d81263`; the source needed no change for the move.)
- **Install (player):** put `jade.smod` in the profile's game folder under `sunrise-native-mods/`,
  turn it on with a line `mod.jade:true` in `sunrise-mods.txt`, and let the launcher build it
  (`sunrise-launcher native-build --instance <profile> --trust-native-code`, or Play). The launcher
  compiles native mods against the game, and native code is not sandboxed. Install it on the server
  too for the server-side lines (see "Two halves" below). How far this was tested: "Launcher install
  test" below.
- **Keys** (all in Options › Controls › Key Binds, category Jade, and rebindable there):
  - Numpad 0: Open Config
  - Numpad 1: Show Overlay (toggle)
  - Numpad 2: Toggle Fluid mode
  - Numpad 5: Narrate the target
  - Left Shift: Show Details
- **Config:** `config/jade-client.toml` in the game folder. It is written with every option and
  re-read when edited. The same options appear in Options › Mods › Jade and on Jade's own screen
  (Numpad 0, or `/jadec config`). `/jadec toggle` turns the overlay off and on.

## What it shows

**Every target**
- The icon.
- The name, in white. A block entity with a custom name shows that name.
- The mod's name in blue italics: "Minecraft" for vanilla, otherwise the mod's package name.
- Optionally:
  - the registry id (off, always, with details, or following F3+H);
  - coordinates, distance and the face looked at.

**Blocks**
- Harvest tool: the tool tier and kinds that mine the block, with a green check or red cross for the
  held tool. Shears and swords count where they apply. Hidden in creative unless "Harvest Tool in
  Creative" is on, as in Jade.
- Unbreakable blocks are marked as such.
- Breaking progress: a strip along the box's bottom edge while you mine.
- Crop growth: `Growth: 57%`, or `Mature`.
- Redstone:
  - lever state;
  - repeater delay and lock;
  - comparator mode;
  - wire and other power levels.
- Note block instrument and note (`Harp C#4`), and unstable TNT.
- Furnace, smoker and blast furnace: input, fuel, progress arrow and output.
- Brewing stand: blaze powder and brewing time.
- Container contents (chests, barrels, hoppers, shulker boxes, dispensers and others): a row of
  icons. With few kinds, each gets a named row; Show Details shows more kinds. Ender chests show the
  viewer's own items. Locked containers and unopened loot chests ("What could it be?") are marked
  as such.
- Beehive and bee nest: honey level and bees.
- Jukebox: the record playing.
- Lectern: the book. Command block: its command. Campfire: what cooks and the time left.
- Spawner: the mob, as `Zombie (Monster Spawner)`.
- Block states with Show Details held, plus explosion resistance and destroy time when "Block
  Properties" is on.

**Fluids** (Fluid Mode Any or Fallback): water and lava with the bucket icon.

**Entities**
- Health as hearts (full, half, empty and absorption, in rows of 10). Above 40 health it shows a
  number instead.
- Armour.
- Mob effects in a nested box: name, level in roman numerals, and time left, harmful ones in red.
- Owner of tamed animals.
- Villager profession (and level, with details); zombie villager conversion time.
- Baby growth time, breeding cooldown, next egg or scute.
- Horse jump, speed and strength.
- Item frame contents, painting title and artist, armour stand gear, minecart contents.
- Dropped items: the item's name and icon, and `Count: 5` for a stack. The game's own crosshair
  skips item entities, so Jade runs its own entity ray (see "Targeting").

**Targeting**
- The game's crosshair target first. Then Jade's own entity ray from the camera, over the entities the
  client tracks (`LevelRef::entities_in`, `EntityRef::bounding_box`). It finds dropped items, and
  with Extended Reach it finds mobs past the game's entity reach. A box thinner than 0.3 on an axis
  is widened to 0.3 there, as in Jade. An entity behind the block hit is not a target.
- While a block is being mined, the game's answer stands, so dropped items are not targets then
  (Jade's rule).
- Extended Reach adds to both the block reach (a clip past the game's reach) and the entity reach.
- Fluids: a walk along the view, per Fluid Mode (API-GAPS #3).

**Overlay behaviour**
- The tooltip:
  - fades and resizes smoothly when the target changes;
  - has a Disappearing Delay;
  - has Dark, Classic and Flat themes;
  - has adjustable position, anchor, scale (shrinking automatically when too tall) and opacity;
  - mirrors to the other side with a left main hand ("Flip with Main Hand").
- Boss Bar Overlap, against the boss bars drawn that frame (the `customize_boss_event_progress`
  hook):
  - Push Down (default): the tooltip goes below the bars when it would cover one;
  - Hide Tooltip: the tooltip hides while it would cover a bar;
  - Hide Boss Bar: the bars the tooltip covers are not drawn;
  - No Operation: both are drawn.
- It hides:
  - while a screen is open (Hide from GUIs);
  - with F1 (it is a HUD layer);
  - while the player list is shown;
  - when toggled off with Show Overlay.
- Display modes: Toggle, Hold and Lite (the main lines only, the rest with Show Details).
- Narrate: Numpad 5 sends the target's text to the narrator (not tested in-game: `sunrise-ctl`
  cannot press numpad keys, and offscreen runs have no narrator).

### Two halves

Most lines are read on the client. What only the server knows comes from the mod's server half over
its own payload channel `jade:data`, as Jade's server plugin does. That covers:
- container and furnace contents;
- mob health, armour and effects (the Sunrise client can't read other entities' health; see
  API-GAPS.md #1);
- owners, villager data and timers.

In single player both halves are present. On a server without the mod, blocks still show name, mod,
harvest tool, breaking progress, crop growth, redstone, note block and block states, and entities show
name and mod.

## Screenshots (in-game, offscreen runs)

All in `../jade-target/shots/`, taken with `sunrise-ctl screenshot` from the built game (1280×720,
GUI scale 3).

**On the 520fc48 build** (2026-11-19, files ending `-520`):

| What | File |
|---|---|
| Furnace: input, fuel, progress arrow | `20-furnace-520.png` |
| Stone; chest contents (named rows); wheat `Growth: 57%`; water (fluid target) | `21-stone-520.png`, `22-chest-520.png`, `23-wheat-520.png`, `24-water-520.png` |
| Zombie: hearts, armour, Slowness III, Strength, Fire Resistance with time left | `25-zombie-520.png` |
| Tamed wolf (hearts, owner); librarian villager (profession) | `26-wolf-520.png`, `30-villager-520.png` |
| Dropped item (new): `Diamond`, `Count: 5`, while the game's crosshair is on the grass | `27-dropped-item-520.png` |
| Extended Reach for entities (new): a cow 9 blocks away, nothing at reach 0, `Cow` at reach 8 | `28-cow-far-reach0-520.png`, `28-cow-far-reach8-520.png` |
| Boss Bar Overlap (new), one `/bossbar`: Push Down, No Operation, Hide Tooltip, Hide Boss Bar | `29-boss-push_down-520.png`, `29-boss-no_operation-520.png`, `29-boss-hide_tooltip-520.png`, `29-boss-hide_boss_bar-520.png` |
| Parity port check (2026-11-19 evening): coloured coordinates + `[3.9]` distance, registry id under the name | `05-coords-registry-parity.png` |
| Crop growth: wheat `57%`, beetroot `67%` | `06-crop-wheat-parity.png`, `06-crop-beetroot-parity.png` |
| Harvest marks: stone with empty hand ✗ and with a pick ✓; dirt with a shovel no mark, empty hand ✗ | `07-stone-empty-cross-parity.png`, `07-stone-pick-check-parity.png`, `07-dirt-shovel-nomark-parity.png`, `07-dirt-empty-cross-parity.png` |
| Health: cow 10 hearts, iron golem `92/100` | `08b-cow-hearts-parity.png`, `08-golem-health-parity.png` |
| Horse stats; llama `Strength` only; camel nothing | `09-horse-stats-parity.png`, `09b-llama-parity.png`, `09c-camel-parity.png` |
| Villager `Librarian - Novice` | `10-villager-parity.png` |
| Note block `Harp F♯/G♭`; empty jukebox shows nothing | `11-noteblock-parity.png`, `12-jukebox-empty-parity.png` |
| Furnace line (input, fuel, arrow, output); chest named rows; `Locked Hopper` | `13-furnace-parity.png`, `14-chest-parity.png`, `16-hopper-locked-parity.png` |

**On the 2d81263 build** (2026-11-18):

| What | File |
|---|---|
| Stone: icon, name, "Minecraft" | `01-stone.png` |
| Chest contents (named rows) | `02-chest.png` |
| Furnace: input, fuel, arrow, output | `final-furnace.png` |
| Wheat growth, note block, jukebox, spawner, beehive | `sheet-blocks.png` |
| Lever, repeater, wire power | `sheet-blocks2.png` |
| Villager (hearts, profession), tamed wolf (20 hearts, owner) | `03-villager.png`, `03-wolf.png` |
| Husk: hearts, armour, Slowness III and Strength with time left | `03-husk.png` |
| Survival: harvest tool and check or cross; breaking progress | `04-iron-ore-wood-pick.png`, `04-stone-breaking.png` |
| Show Details: block states | `05-repeater-details.png` |
| Hidden with F1, hidden with the inventory open | `06-f1-hidden.png`, `06-inventory-hidden.png` |
| Key Binds: Show Overlay rebound from Keypad 1 to O; O then hides the tooltip | `07-keybinds-rebound.png`, `sheet-toggle.png` |
| Jade's config screen with the live preview | `final-config-screen.png` |
| Water (fluid target), item frame | `11-water.png`, `final-item-frame.png` |
| Fixes after the first run: item frame contents, furnace arrow, no cross on a chest under a pickaxe | `sheet-final.png` |
| Options › Mods lists Jade, and its Config page holds the same options | `12-options-mods.png`, `12-options-mods-jade-config.png` |
| Lite mode: Show Details reveals the rest (here the block states) | `sheet-lite.png` |

## What doesn't work (yet), and why

Gaps in Sunrise's mod API are listed with what this port does instead in
**[API-GAPS.md](API-GAPS.md)**. In short:
- No hearts on a server without the mod.
- The Open Config key opens Jade's own quick screen, not Options › Mods.
- No "Jade is closed" toast.
- Hearts don't blink when the mob is hurt.
- Crops show their block item, not the pick-block item.
- No player-head or decorated-pot icons.

Limits of Jade's own entity ray here (the client can't read these):
- Newly fired projectiles (10 ticks old or less) are not left out.
- The vehicle you ride is left out by a stand-in rule: an entity whose box holds your own feet is
  skipped. (Vehicle reads are server-side only.)
- Invisible mobs are targets.

Also not ported:
- Jade's plugin API for other mods.
- The per-provider ordering/priority screen.
- The boss-bar and "more info" arrow sprites.
- Languages other than English.

## Build, check and run (developers)

Everything builds into a sibling folder, `../jade-target` (`CARGO_TARGET_DIR`), never into the
game checkout. The SDK, the Rust 1.99.1 toolchain the SDK names, and the build output all live
there.

```sh
cd jade-sunrise
tools/fetch_sdk.sh                 # once: the 520fc48 SDK (tool, api, runtime, engine slices), checksums verified
. tools/env.sh                     # CARGO_TARGET_DIR, RUSTUP_HOME, the toolchain, sunrise_mod()
CREW_AGENT=website crew run --kind build -- tools/build.sh   # check, build jade.smod, native-build, install
```

On this machine every heavy job (builds, game runs) goes through `crew run`, which admits it when
there is room (3 GB of RAM free). It replaced the old `flock /tmp/sunrise-cargo.lock` rule on
2026-11-19. Exit code 75 means crew refused the job.

`tools/build.sh` does four things:
1. Runs `cargo sunrise-mod check` (skipped with `SKIP_CHECK=1`).
2. Runs `cargo sunrise-mod build`, giving `target/sunrise-mod/jade.smod`, copied to
   `../jade-target/jade.smod`.
3. Runs the SDK's `native-build` the way the launcher does, against the pinned SDK's runtime,
   giving a game executable with Jade linked in.
4. Installs it as the launcher would in `../jade-target/install/`: `sunrise-client`, its libraries
   (`libmod_jade.so` and the others), `native-mods.json` with their checksums, and
   `sunrise-native-mods/jade/`.

`tools/run.sh` starts that game offscreen. It uses a fresh `/tmp/jade-run.*` folder for
`SUNRISE_HOME`, `SUNRISE_LAUNCHER_HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME` and the game folder, with
`SUNRISE_NO_AUDIO=1` and a control socket. Drive it with `sunrise-ctl --socket "$S/ctl.sock" …`.
Through crew, `JADE_WAIT=1` keeps the script running until the game exits, so crew accounts for
the whole run. Even offscreen, the game needs a display connection, so it runs as
`--kind game --display`. crew grants a display only while the owner is away; a `--kind harness`
job has no display, and the game stops at start.

```sh
CREW_AGENT=website JADE_WAIT=1 crew run --kind game --display -- tools/run.sh &   # prints $S
S=/tmp/jade-run.XXXXXX             # the folder it printed; the log is $S/game.out
sunrise-ctl --socket "$S/ctl.sock" open Jade 1 creative
sunrise-ctl --socket "$S/ctl.sock" setblock 169 70 160 furnace
sunrise-ctl --socket "$S/ctl.sock" look-at block 169 70 160
sunrise-ctl --socket "$S/ctl.sock" screenshot /some/path/shot.png
sunrise-ctl --socket "$S/ctl.sock" quit
```

Set `JADE_DEBUG=1` before `tools/run.sh` to print each server reply to the log, and what the
client's own copy of the targeted block entity holds (API-GAPS #10).

**Vanilla resources.** The task expected prepared 26.3 resources at
`sunrise/.local/sunrise-client/26.3`; they are not there. `tools/run.sh` therefore reads
`../jade-target/resources/26.3`, a copy of the resources the installed 2d81263 Nightly prepared,
or any folder given as `RESOURCES=…`. The 520fc48 game reads that copy as it is. Nothing is written
to the launcher's own folders. The game refuses resources inside a launcher's data folder unless
the launcher's Play started it ("no cache lease"). A copy outside the launcher's folders is fine.

## Launcher install test (Sunrise Launcher 1.1.1, 2026-11-19)

Run headless with the launcher's CLI in an isolated home (`../jade-target/launcher-1.1.1/lx.sh`
sets `SUNRISE_HOME`, `SUNRISE_LAUNCHER_HOME` and the XDG folders to `iso/`; the player's own launcher
data is never touched):

1. `install nightly-2026-11-19-520fc48`: **worked** (5 s).
2. `instance create Jade`: **worked**.
3. Adding `jade.smod`:
   - The CLI has no verb for a local `.smod`. `instance import jade.smod` refuses it ("drop a .zip,
     a .sshader or a Sunrise game build"). `mods install` takes registry ids, and `mods add-github`
     takes GitHub links.
   - Done by hand instead, as the player docs describe: `sunrise-native-mods/jade.smod` in the
     profile's folder, plus `mod.jade:true` in `sunrise-mods.txt`.
   - A copy in `sunrise-mods/` (where `docs/api/content.md` says native mods go) is not found:
     "No native mod is turned on in …/sunrise-native-mods".
4. `native-build` first stopped at "not prepared yet". After `prepare --minecraft
   ~/.local/share/ModrinthApp/meta` (the Modrinth App's 26.3, 15 s), it **worked**:
   - `native-build --instance jade --trust-native-code` fetched the 110 MB of mod tools and the
     launcher's own Rust 1.99.1;
   - it compiled `jade` against the prebuilt 520fc48 game (route `prebuilt-api`, profile
     `optimized`) in 70 s;
   - it installed `versions/native-520fc48-8bcb88c035e4` (with `libmod_jade.so` and
     `native-mods.json`) and selected it for the profile.
5. Playing it **was not possible here**:
   - `play` needs a signed-in Microsoft account that owns Minecraft, and the isolated home has none.
   - Starting the built `sunrise-client` directly with the launcher's prepared resources is refused
     (no cache lease).
   - With resources outside the launcher's folders, a run needs a display. crew refused that job
     with exit 75 ("display forbidden: the owner is present"), and a `--kind harness` run has no
     display, so the game stopped at start.
   - So the launcher-built game was not seen running with Jade. The `tools/build.sh` game, built by
     the same SDK `native-build` against the same runtime, is the one in the screenshots.

## Layout

```
jade-sunrise/
  sunrise-mod.toml     id, side "both", [native] pin (520fc48, mixins e59da90)
  Cargo.toml           crate mod-jade; sunrise-mod-api from the SDK
  src/lib.rs           hooks: keys, settings, payload channel, HUD layer, config file, screen, commands
  src/client.rs        target finding (crosshair, entity ray, extended reach, fluids), keys, visibility,
                       boss bar overlap, fading, drawing
  src/providers.rs     what each block, entity and fluid adds to its tooltip
  src/server.rs        the server half: answers jade:data requests from block entity and entity data
  src/tooltip.rs       elements, layout, themes and painting of the box
  src/config.rs        the 72 options: Options › Mods settings, config/jade-client.toml, defaults
  src/screen.rs        Jade's quick config screen with a live preview
  assets/jade/lang/en_us.json   strings (generated by tools/make_lang.py)
  tools/               env.sh, fetch_sdk.sh, build.sh, run.sh, make_lang.py
  API-GAPS.md          what the mod API lacks for Jade
  SPEC.md              the behaviour spec this was written from (by another agent; not edited here)
```

## Readings taken

The task said not to ask but to decide and write it down. These are the decisions:

- **Pin:** the newest Nightly with an SDK (520fc48, 2026-11-19; first 2d81263, 2026-11-17), since a
  native mod builds only against a published SDK.
- **Boss bar rectangle:** each bar as vanilla draws it, 182 wide, from its title 9 pixels above the
  bar to the bar's 5-pixel bottom. Push Down leaves a 2-pixel gap below the lowest bar. Hide Boss
  Bar compares against the box drawn the frame before, since the bars are drawn before the HUD layer
  that draws the tooltip. Jade's fourth mode, Hide Boss Bar, was added as the last choice, so
  existing config files read the same.
- **Entity ray:** only when the game's crosshair has no entity. It runs within the player's
  entity interaction range plus Extended Reach, and never past the block hit.
- **Open Config:** opens Jade's own screen. Sunrise has no call to open a mod's Options › Mods page
  (API-GAPS #2). The screen writes the same settings, so Options › Mods › Jade and the file stay in
  step.
- **Config file:** `config/jade-client.toml`, one file with sections general, appearance,
  formatting, blocks, storage and entities. It replaces Jade's several JSON files.
- **Default toggle key and toast:** the Show Overlay toggle is per session, as in Jade. It doesn't
  change "Enabled" in the file. No toast, since there is no API for one.
- **Harvest marks:**
  - A red cross means "this tool won't get the drop".
  - For blocks any tool harvests, the check shows only when the held tool is the effective one, and
    nothing is shown otherwise.
- **Progress arrow:** drawn from fills. The vanilla furnace arrow sprite is opaque on the furnace's
  grey, and Jade's arrow is Jade art.
- **Server data:** from the mod's own server half (`side = "both"`), not from client reads that
  Sunrise doesn't offer.
