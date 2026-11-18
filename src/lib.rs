//! Jade for Sunrise: "what am I looking at". A clean-room port of the behaviour of Jade
//! (Snownee's HWYLA successor, CC BY-NC-SA 4.0) on Sunrise's native mod API: no Jade code or art.
//!
//! - `client`: the crosshair target, the keys, the tooltip built each tick and drawn as a HUD layer.
//! - `providers`: what each block, entity and fluid adds to its tooltip.
//! - `server`: the server half, answering what only the server knows (`jade:data` payloads).
//! - `tooltip`: the tooltip's elements, layout and drawing.
//! - `config`: the options, shared by Options > Mods, `config/jade-client.toml` and Jade's screen.
//! - `screen`: the quick option screen (Numpad 0).

mod client;
mod config;
mod providers;
mod screen;
mod server;
mod tooltip;

use mod_api::*;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) const ID: &str = "jade";
const LAYER: &str = "jade:tooltip";

/// The quick screen's Done was clicked: closed on the next tick (the click hook has no `&mut`).
static CLOSE_SCREEN: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct Mod;

#[mod_api::sunrise_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, cx: &RegistrationContext) {
        for (id, key, label) in client::KEYS {
            let context = if *id == "show_details" { KeyConflictContext::Universal } else { KeyConflictContext::InGame };
            r.register_key_mapping(KeyRegistration::new(id, key).label(label).context(context));
        }
        config::register(r);
        r.payload_handlers().registrar("1").play_bidirectional(
            PayloadId(server::CHANNEL.into()),
            std::sync::Arc::new(server::FromClient),
            Some(std::sync::Arc::new(server::FromServer)),
        );
        // The mod-name line: each namespace's package name.
        let mut names = std::collections::HashMap::new();
        for package in cx.mods().iter() {
            let name = package.name();
            if name.is_empty() {
                continue;
            }
            for ns in package.namespaces() {
                names.entry(ns.to_owned()).or_insert_with(|| name.to_owned());
            }
        }
        client::set_mod_names(names);
    }

    fn register_gui_layers(&self, _cx: &RegistrationContext, event: &mut RegisterGuiLayersEvent<'_>) {
        event.layers.push(GuiLayer { id: LAYER.into(), placement: GuiLayerPlacement::Below("minecraft:chat".into()) });
    }

    fn gui_layer_draw(&self, host: &dyn ClientHost, event: &mut GuiLayerDrawEvent<'_>) {
        if event.layer == LAYER && screen::open_now(host).is_none() {
            client::draw(host, &mut *event.draw);
        }
    }

    fn register_configs(&self, _side: HostSide, event: &mut RegisterConfigsEvent<'_>) {
        let mut c = Config::new(ID, ConfigKind::Client, &config::default_file());
        c.file = config::FILE.into();
        event.configs.push(c);
    }

    fn config_loading(&self, _side: HostSide, event: &mut ConfigLoadingEvent<'_>) {
        if event.config.mod_id == ID && event.config.file == config::FILE {
            config::file_loaded(&event.config.text);
        }
    }

    fn config_reloading(&self, _side: HostSide, event: &mut ConfigReloadingEvent<'_>) {
        if event.config.mod_id == ID && event.config.file == config::FILE {
            config::file_loaded(&event.config.text);
        }
    }

    fn setting_changed(&self, host: &mut dyn ClientHost, event: &mut SettingChangedEvent<'_>) {
        if event.mod_id == ID {
            config::setting_changed(host, event.key);
        }
    }

    fn client_player_logging_in(&self, _host: &mut dyn ClientHost, _event: &mut ClientPlayerLoggingInEvent) {
        client::logging_in();
    }

    fn client_player_logging_out(&self, _host: &mut dyn ClientHost, _event: &mut ClientPlayerLoggingOutEvent) {
        client::logging_out();
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _event: &mut ClientTickPostEvent) {
        if CLOSE_SCREEN.swap(false, Ordering::Relaxed) && screen::open_now(host).is_some() {
            host.mod_screen_close(ID);
        }
        client::tick(host);
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if screen::open_now(host).is_some() {
            screen::render(host, &mut *event.draw, event.screen.mouse);
            // The live preview, over the screen.
            client::draw(host, &mut *event.draw);
        }
    }

    fn screen_mouse_button_pressed_pre(&self, host: &dyn ClientHost, event: &mut ScreenMouseButtonPressedPreEvent<'_>) -> Flow {
        if screen::open_now(host).is_none() {
            return Flow::Continue;
        }
        let size = screen::size();
        if screen::on_done(size, event.screen.mouse) {
            CLOSE_SCREEN.store(true, Ordering::Relaxed);
        } else {
            screen::click(host, size, event.screen.mouse, event.button as u32);
        }
        Flow::Cancel
    }

    fn screen_mouse_scrolled_pre(&self, host: &dyn ClientHost, event: &mut ScreenMouseScrolledPreEvent<'_>) -> Flow {
        if screen::open_now(host).is_none() {
            return Flow::Continue;
        }
        screen::scroll(screen::size(), event.scroll[1] as f32);
        Flow::Cancel
    }

    fn register_client_commands(&self, _host: &mut dyn ClientHost, event: &mut RegisterClientCommandsEvent<'_>) {
        for usage in ["jadec config", "jadec toggle"] {
            event.commands.push(Command::new(usage));
        }
    }

    /// `/jadec config` opens the quick screen; `/jadec toggle` flips the overlay on or off.
    fn command(&self, host: &mut dyn ClientHost, event: &mut CommandEvent<'_>) -> Flow {
        let line = event.command.trim_start_matches('/').trim();
        match line {
            "jadec config" => screen::open(host),
            "jadec toggle" => {
                let on = !config::on("enabled");
                config::set("enabled", config::Value::Bool(on));
                host.set_setting(ID, "enabled", if on { "true" } else { "false" });
            }
            _ => return Flow::Continue,
        }
        Flow::Cancel
    }
}
