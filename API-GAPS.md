# What Sunrise's mod API lacks for Jade

Gaps met while porting Jade to the Sunrise mod API. First written for build 2d81263 (Nightly
2026-11-17), then checked again on 2026-11-19 against build **520fc48** (Nightly 2026-11-19). The
check compared the two SDKs' `surface.json` (174 ids added, 22 removed, 18 changed) and read the
520fc48 docs of every API each row names.

Each row says what is missing and what Jade needs it for. Where a workaround through ordinary API
calls exists, it is named. Nothing here patches the game, uses mixins or reaches into internals.

## Closed

Both APIs below were already in 2d81263; the first pass missed them. Both are in 520fc48 and now
used.

| # | Was missing | Now done with | What Jade does now |
|---|---|---|---|
| 4 | Client reads of the boss bars on screen | The `customize_boss_event_progress` hook (client): each bar's `x`, `y` and `increment` once a frame before it is drawn, and `cancelled` to hide it | Boss Bar Overlap works in all four of Jade's modes (Push Down, Hide Tooltip, Hide Boss Bar, No Operation). Hide Boss Bar was added as a fourth choice. Shots: `29-boss-*-520.png` |
| 5 | A client entity ray (`LevelRef::ray_trace_entities` is server-side) | `LevelRef::entities_in` and `EntityRef::bounding_box`, both "Side: both"; on a client, the entities the server tracks to it. Jade intersects its own ray with their boxes | Dropped items are targets (name, icon, count). Extended Reach covers entities too. Shots: `27-dropped-item-520.png`, `28-cow-far-reach*-520.png` |

What #5 still can't do, because the client can't read it (all server-only at 520fc48):
- An entity's age (`tickCount`), so newly fired projectiles aren't left out.
- The player's vehicle (`root_vehicle`, `is_passenger`). A stand-in skips any entity whose box holds
  the player's feet.
- Invisibility (`is_invisible`).

## Still missing at 520fc48

| # | Missing | Jade needs it for | What this port does |
|---|---|---|---|
| 1 | Client reads of another entity's health, max health, absorption, armour and effects. On a client, `living()` still resolves only the local player (`LivingHost::living`, doc unchanged), and `EntityRef::kind` is a server read | The hearts and armour rows, mob effects, horse stats; Jade reads health and armour on the client and works without its server half | Asks its own server half over the `jade:data` payload (as Jade does for absorption and effects). Against a server without this mod, entities show name and mod only, no hearts |
| 2 | A way to open a mod's own page in Options > Mods from code | The Open Config key (Numpad 0) opening the config screen | The key opens Jade's own quick screen of buttons, writing the same settings |
| 3 | A fluid mode for `LevelRef::clip` (`ClipFluid` still has only `None`) | Fluid targets (Fluid Mode Any/Fallback) | A voxel walk along the view reading `LevelRef::fluid_state` per cell; it treats a source or falling cell as a full block and a flowing cell by its level |
| 6 | A toast API for the client. 520fc48 has `toast_add`, but it only filters or edits the toasts the game queues; there is no call to add one | The "Jade is now closed / press X to reopen" toast on the first toggle | No toast |
| 7 | A client read of whether an entity was hurt this tick (`hurtTime`). Only `MinecartRef::hurt_time` exists, and it is server-side | Hearts blinking white when the target takes damage | Hearts don't blink |
| 8 | A pick-block / clone-item read (`Block.getCloneItemStack`) for any block. `BlockPart::clone_item_stack` is a hook a mod's own blocks answer, not a read | The icon and the name fallback of blocks whose item is not `Block.asItem` (crops show their seeds, as vanilla's pick does; potted plants show the pot) | Uses `block_item` (`Block.asItem`), then the block's translation |
| 9 | Rendering of a block entity or a player head as a GUI icon (`BlockEntityWithoutLevelRenderer` in the GUI). `DrawContext::gui_paint`'s doc mentions player faces, but none of its ops (`Sprite`, `AtlasSprite`, `Blit` of a texture path, `Item` of a hotbar slot, fills, text) takes a game profile, a skull or a block entity; not tried in-game | Player-head faces and decorated pots as the icon | Uses the block's item icon |
| 10 | Usable client data for the block entities a server without this mod leaves out. `BlockEntityRef::data` works on the client copy, but a `JADE_DEBUG=1` run showed that copy holds defaults: a lit furnace read `Items: []` and every timer 0 while the server's had raw iron, coal and a running cook | Client-only fallbacks for signs, skulls and banners when the server lacks the mod | Every block entity line comes from the server half |
| 11 | A scale in `DrawContext::gui_nine_slice` and in `gui_draw_item_with`. 520fc48 adds `DrawContext::text_scaled`, but Jade already drew its scaled text under a `gui_paint` pose | Theme sprites and slot counts at Display Scale other than 1 | The box is drawn from fills and gradients at any scale, as vanilla's tooltip frame; item counts are drawn as scaled text |
| 12 | A read of the server's tag for "shearable" blocks and of a block's loot table. 520fc48's loot additions (`ModLootCondition`, `ModLootFunction`, `LootTypeContext`) let a mod register loot codecs, not read a block's table | Jade's shears icon and the loot-table debug line | Shears for the vanilla wool, leaves, cobweb, vines and grass; no loot-table line |

Hooks and handles this port relies on (all in the 520fc48 surface): `register_gui_layers` and
`gui_layer_draw` (the overlay as a HUD layer, so F1 hides it), `client_tick_post`,
`crosshair_target`, `LevelRef::entities_in` and `EntityRef::bounding_box` (the entity ray),
`customize_boss_event_progress` (Boss Bar Overlap), `PlayerRef::block_breaking`, `block_states()`
(correct tool, destroy speed, definitions), `block_in_tag`, `register_key_mapping` and
`consume_click`/`is_down` (the keys, rebindable in Options > Controls), `register_setting` and
`setting_changed` (Options > Mods), `register_configs` and `config_loading`/`config_reloading`
(`config/jade-client.toml`), `payload_handlers` (the server half), `mod_screen_open` and the screen
hooks (the quick screen), `RegistrationContext::mods` (mod names).
