//! The server half: what only the server knows about the target, sent to the client that asks.
//!
//! As Jade's server plugin does, the client asks for the block or entity under its crosshair at
//! most every 250 ms (`jade:data`, a JSON request) and the server answers with one JSON object:
//! the block entity's saved data, or the entity's type, health, effects, owner and a trimmed copy
//! of its saved data. A request for something farther than [`MAX_DISTANCE`] from the player is
//! refused, so the channel can't be used to look through walls across the map.

use mod_api::serde_json::{self, Map, Value, json};
use mod_api::*;

pub const CHANNEL: &str = "jade:data";
/// Farthest a target may be from the asking player's eyes, in blocks (Jade's default
/// `max_position_deviation`, 21, rounded up for the target's own size).
const MAX_DISTANCE: f64 = 24.0;

/// The saved-data keys of an entity the client's providers read; the rest (brains, gossip,
/// trades, inventories) is never sent.
const ENTITY_KEYS: &[&str] = &[
    "VillagerData", "Age", "ForcedAge", "InLove", "Item", "item", "block_state", "variant", "equipment", "ConversionTime",
    "EggLayTime", "scute_time", "Tame", "Variant", "Fuse", "Strength", "Sitting", "Invisible",
    "Items", "CustomName", "Owner", "ItemRotation", "Fixed", "BlockState", "Time", "Health",
    "DeathTime", "AbsorptionAmount", "RestocksToday", "LastRestock", "Brain", "SpawnData",
    "DuplicationCooldown",
];

/// A request's target: a block position, or an entity id.
pub fn request_block(pos: [i32; 3], details: bool) -> Vec<u8> {
    json!({"t": "b", "p": pos, "d": details}).to_string().into_bytes()
}

pub fn request_entity(id: u32, details: bool) -> Vec<u8> {
    json!({"t": "e", "id": id, "d": details}).to_string().into_bytes()
}

/// NBT as JSON: compounds become objects, lists and arrays become arrays, numbers numbers.
pub fn tag_json(tag: &Tag) -> Value {
    match tag {
        Tag::End => Value::Null,
        Tag::Byte(b) => json!(*b),
        Tag::Short(s) => json!(*s),
        Tag::Int(i) => json!(*i),
        Tag::Long(l) => json!(*l),
        Tag::Float(bits) => json!(f32::from_bits(*bits)),
        Tag::Double(bits) => json!(f64::from_bits(*bits)),
        Tag::ByteArray(a) => Value::Array(a.iter().map(|b| json!(*b as i8)).collect()),
        Tag::String(s) => Value::String(s.to_string_lossy()),
        Tag::List(l) => Value::Array(l.iter().map(tag_json).collect()),
        Tag::Compound(c) => compound_json(c),
        Tag::IntArray(a) => Value::Array(a.iter().map(|i| json!(*i)).collect()),
        Tag::LongArray(a) => Value::Array(a.iter().map(|i| json!(*i)).collect()),
    }
}

pub fn compound_json(c: &Compound) -> Value {
    let mut map = Map::new();
    for (k, v) in c {
        map.insert(k.to_string_lossy(), tag_json(v));
    }
    Value::Object(map)
}

fn distance_ok(host: &dyn Host, player: EntityRef, at: [f64; 3]) -> bool {
    let Some(eye) = player.eye_position(host) else { return false };
    let d = [eye[0] - at[0], eye[1] - at[1], eye[2] - at[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() <= MAX_DISTANCE
}

/// The ender chest's items as the block entity data's `Items` would hold them.
fn ender_items(host: &dyn Host, player: PlayerRef) -> Value {
    let stacks = player.ender_chest(host).unwrap_or_default();
    Value::Array(
        stacks
            .iter()
            .map(|(slot, s)| json!({"Slot": slot, "id": s.item().id(host), "count": s.count()}))
            .collect(),
    )
}

fn block_reply(host: &mut dyn ServerHost, player: PlayerRef, me: EntityRef, pos: [i32; 3]) -> Option<Value> {
    let center = [pos[0] as f64 + 0.5, pos[1] as f64 + 0.5, pos[2] as f64 + 0.5];
    if !distance_ok(host, me, center) {
        return None;
    }
    let level = player.level(host)?;
    let mut out = json!({"t": "b", "p": pos});
    let block = level.block(host, pos).map(|b| b.id(host));
    if block.as_deref() == Some("minecraft:ender_chest") {
        out["ender"] = ender_items(host, player);
    }
    // Jade's RedstoneProvider streams the signal behind a calibrated sculk sensor's back.
    if let Some(state) = level.block_state(host, pos)
        && let Some(text) = host.block_state_string(state)
        && block.as_deref() == Some("minecraft:calibrated_sculk_sensor")
        && let Some(facing) = property(&text, "facing")
    {
        let back = match facing.as_str() {
            "north" => ([pos[0], pos[1], pos[2] - 1], Direction::North),
            "south" => ([pos[0], pos[1], pos[2] + 1], Direction::South),
            "west" => ([pos[0] - 1, pos[1], pos[2]], Direction::West),
            "east" => ([pos[0] + 1, pos[1], pos[2]], Direction::East),
            "down" => ([pos[0], pos[1] - 1, pos[2]], Direction::Down),
            _ => ([pos[0], pos[1] + 1, pos[2]], Direction::Up),
        };
        if let Some(signal) = level.signal(host, back.0, back.1) {
            out["input_signal"] = json!(signal);
        }
    }
    // Jade's HopperLockProvider: a hopper turned off by redstone renames the title.
    if let Some(state) = level.block_state(host, pos)
        && let Some(text) = host.block_state_string(state)
        && block.as_deref() == Some("minecraft:hopper")
        && property(&text, "enabled") == Some("false".to_owned())
    {
        out["hopper_disabled"] = Value::Bool(true);
    }
    if let Some(be) = host.block_entity(level, pos) {
        if let Some(data) = be.data(host) {
            out["be"] = compound_json(&data);
        }
        if let Some(name) = be.name(host) {
            out["name"] = Value::String(name);
        }
        if be.lock(host).is_some_and(|l| !l.is_empty()) {
            out["locked"] = Value::Bool(true);
        }
    }
    Some(out)
}

/// One property's value from `/setblock`-style state text.
fn property(text: &str, key: &str) -> Option<String> {
    let inner = text.split_once('[')?.1.trim_end_matches(']');
    inner.split(',').find_map(|kv| kv.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().to_owned()))
}

fn entity_reply(host: &mut dyn ServerHost, me: EntityRef, id: u32) -> Option<Value> {
    let entity = host.entity(id)?;
    let at = entity.position(host)?;
    if !distance_ok(host, me, at) {
        return None;
    }
    let mut out = json!({"t": "e", "id": id});
    let kind = entity.kind(host).map(|k| k.id(host));
    if let Some(kind) = &kind {
        out["type"] = Value::String(kind.to_owned());
    }
    if let Some(living) = host.living(entity) {
        // Jade's health gate: an armour stand or a creaking shows no hearts.
        let health_visible = !matches!(
            kind.as_deref(),
            Some("minecraft:armor_stand") | Some("minecraft:creaking")
        );
        if health_visible {
            if let Some(h) = living.health(host) {
                out["health"] = json!(h);
            }
            if let Some(m) = living.max_health(host) {
                out["max_health"] = json!(m);
            }
            if let Some(a) = living.absorption(host) {
                out["absorption"] = json!(a);
            }
        }
        if let Some(a) = living.attribute_value(host, "minecraft:armor") {
            out["armor"] = json!(a);
        }
        for (key, attr) in [("speed", "minecraft:movement_speed"), ("jump", "minecraft:jump_strength")] {
            if let Some(v) = living.attribute_value(host, attr) {
                out[key] = json!(v);
            }
        }
        if let Some(effects) = living.effects(host) {
            out["effects"] = Value::Array(
                effects
                    .iter()
                    .filter(|e| e.visible)
                    .map(|e| json!({"id": e.id.id(host), "amp": e.amplifier, "dur": e.duration}))
                    .collect(),
            );
        }
        if living.is_tamed(host) == Some(true) {
            out["tamed"] = Value::Bool(true);
        }
        // Jade sends the owner's name when it resolves one and nothing at all when it doesn't.
        if let Some(owner) = living.owner(host)
            && let Some(name) = host.player(owner).and_then(|p| p.name(host))
        {
            out["owner"] = Value::String(name);
        }
    }
    if let Some(nbt) = entity.nbt(host) {
        let mut map = Map::new();
        for key in ENTITY_KEYS {
            if let Some(v) = nbt.get(&JavaString::from(*key)) {
                map.insert((*key).to_owned(), tag_json(v));
            }
        }
        out["nbt"] = Value::Object(map);
    }
    Some(out)
}

/// A client's request, answered at once.
pub struct FromClient;

impl IPayloadHandler for FromClient {
    fn handle(&self, payload: PayloadBytes, cx: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        let player = cx.player().map_err(|e| PayloadHandleError::Failed(format!("{e:?}")))?;
        let host = cx.server_host().map_err(|e| PayloadHandleError::Failed(format!("{e:?}")))?;
        let Ok(req) = serde_json::from_slice::<Value>(&payload.0) else { return Ok(()) };
        let Some(player) = host.player(player.uuid()) else { return Ok(()) };
        let me = player.entity();
        let reply = match req["t"].as_str() {
            Some("b") => {
                let p = &req["p"];
                let pos = [p[0].as_i64(), p[1].as_i64(), p[2].as_i64()];
                match pos {
                    [Some(x), Some(y), Some(z)] => block_reply(host, player, me, [x as i32, y as i32, z as i32]),
                    _ => None,
                }
            }
            Some("e") => req["id"].as_u64().and_then(|id| entity_reply(host, me, id as u32)),
            _ => None,
        };
        if let Some(reply) = reply {
            let _ = player.send_payload(host, &PayloadId(CHANNEL.into()), PayloadBytes(reply.to_string().into_bytes()));
        }
        Ok(())
    }
}

/// The server's answer, kept for the client's providers.
pub struct FromServer;

impl IPayloadHandler for FromServer {
    fn handle(&self, payload: PayloadBytes, _cx: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        if let Ok(reply) = serde_json::from_slice::<Value>(&payload.0) {
            crate::client::received(reply);
        }
        Ok(())
    }
}
