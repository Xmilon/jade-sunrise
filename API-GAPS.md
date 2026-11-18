# What Sunrise's mod API lacks for Jade

Gaps met while porting Jade to the Sunrise mod API of build 2d81263 (Nightly 2026-11-17). Each row
says what is missing and what Jade needs it for. Where a workaround through ordinary API calls
exists, it is named; nothing here patches the game, uses mixins or reaches into internals.

| # | Missing | Jade needs it for | What this port does |
|---|---|---|---|
| 1 | Client reads of another entity's health, max health, absorption, armour and effects. On a client, `living()` resolves only the local player (`LivingHost::living`), and `EntityRef::kind` is a server read | The hearts and armour rows, mob effects, horse stats; Jade reads health and armour on the client and works without its server half | Asks its own server half over the `jade:data` payload (as Jade does for absorption and effects). Against a server without this mod, entities show name and mod only, no hearts |
| 2 | A way to open a mod's own page in Options > Mods from code | The Open Config key (Numpad 0) opening the config screen | The key opens Jade's own quick screen of buttons, writing the same settings |
| 3 | A fluid mode for `LevelRef::clip` (`ClipFluid` has only `None`) | Fluid targets (Fluid Mode Any/Fallback) | A voxel walk along the view reading `LevelRef::fluid_state` per cell; it treats a source or falling cell as a full block and a flowing cell by its level |
| 4 | Client reads of the boss bars on screen (count or rectangle) | Boss Bar Overlap: push the tooltip below boss bars, or hide one of them | The option is kept in the config but has no effect yet |
| 5 | A client entity ray (`LevelRef::ray_trace_entities` is server-side) | Extended Reach for entities, and dropped items as targets (the game's crosshair skips item entities; Jade runs its own ray) | Extended Reach applies to blocks only; dropped items are not targets, so the Dropped Item provider (name and count) never runs |
| 6 | A toast API for the client | The "Jade is now closed / press X to reopen" toast on the first toggle | No toast |
| 7 | A client read of whether an entity was hurt this tick (`hurtTime`) | Hearts blinking white when the target takes damage | Hearts don't blink |
| 8 | A pick-block / clone-item read (`Block.getCloneItemStack`) | The icon and the name fallback of blocks whose item is not `Block.asItem` (crops show their seeds, as vanilla's pick does; potted plants show the pot) | Uses `block_item` (`Block.asItem`), then the block's translation |
| 9 | Rendering of a block entity or a player head as a GUI icon (`BlockEntityWithoutLevelRenderer` in the GUI) | Player-head faces and decorated pots as the icon | Uses the block's item icon |
| 10 | A client read of a block entity's synced data for vanilla blocks the client keeps (`BlockEntityRef::data` on the client copy holds only what the update tag carried) | Client-only fallbacks for signs, skulls and banners when the server lacks the mod | Every block entity line comes from the server half |
| 11 | A scale in `DrawContext::gui_nine_slice` and in `gui_draw_item_with` | Theme sprites and slot counts at Display Scale other than 1 | The box is drawn from fills and gradients at any scale, as vanilla's tooltip frame; item counts are drawn as scaled text |
| 12 | A read of the server's tag for "shearable" blocks and of a block's loot table | Jade's shears icon and the loot-table debug line | Shears for the vanilla wool, leaves, cobweb, vines and grass; no loot-table line |

Hooks and handles this port relies on (all in the 2d81263 surface): `register_gui_layers` and
`gui_layer_draw` (the overlay as a HUD layer, so F1 hides it), `client_tick_post`,
`crosshair_target`, `PlayerRef::block_breaking`, `block_states()` (correct tool, destroy speed,
definitions), `block_in_tag`, `register_key_mapping` and `consume_click`/`is_down` (the keys, rebindable in
Options > Controls), `register_setting` and `setting_changed` (Options > Mods), `register_configs` and
`config_loading`/`config_reloading` (`config/jade-client.toml`), `payload_handlers` (the server half),
`mod_screen_open` and the screen hooks (the quick screen), `RegistrationContext::mods` (mod names).
