//! Web stand-in for the deno scene thread.
//!
//! The scene's JS runs in a dedicated Web Worker (`scene_worker.js`) that reuses the same
//! `dcl/js/js_modules`. This side mirrors the CRDT path of the deno ops in `dcl/js/engine.rs`:
//! frames from the Worker are applied to the scene's `SceneCrdtState` and sent to the renderer,
//! and each renderer reply is serialized back to the Worker. Player data and signed fetch answer
//! `call` messages through `dcl/scene_ops`; gameplay actions use the renderer's RPC queue.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::time::Instant;

use dcl_web_scene::EventKind;
use serde::Deserialize;
use tokio::sync::mpsc::Receiver;

use crate::auth::ephemeral_auth_chain::EphemeralAuthChain;
use crate::dcl::crdt::message::{
    append_gos_component, process_many_messages, put_or_delete_lww_component,
};
use crate::dcl::crdt::DirtyCrdtState;
use crate::dcl::scene_apis::{ContentMapping, GetSceneInformationResponse, RpcCall};
use crate::dcl::scene_events::SceneEvents;
use crate::dcl::scene_ops::{get_player_data, get_players, signed_fetch_headers};
use crate::dcl::serialization::reader::DclReader;
use crate::dcl::serialization::writer::DclWriter;
use crate::dcl::{
    DclSceneRealmData, RendererResponse, SceneId, SceneResponse, SharedSceneCrdtState,
    SpawnDclSceneData,
};
use crate::godot_classes::{dcl_global_time::DclGlobalTime, dcl_scene_locale::get_scene_locale};
use crate::realm::scene_definition::SceneEntityDefinition;

const WORKER_SRC: &str = include_str!("scene_worker.js");
const LOADER_SRC: &str = include_str!("../dcl/js/js_modules/main.js");
const MODULES: &[(&str, &str)] = &[
    (
        "~system/AdaptationLayerHelper",
        include_str!("../dcl/js/js_modules/AdaptationLayerHelper.js"),
    ),
    (
        "~system/CommsApi",
        include_str!("../dcl/js/js_modules/CommsApi.js"),
    ),
    (
        "~system/CommunicationsController",
        include_str!("../dcl/js/js_modules/CommunicationsController.js"),
    ),
    (
        "~system/EngineApi",
        include_str!("../dcl/js/js_modules/EngineApi.js"),
    ),
    (
        "~system/EnvironmentApi",
        include_str!("../dcl/js/js_modules/EnvironmentApi.js"),
    ),
    (
        "~system/EthereumController",
        include_str!("../dcl/js/js_modules/EthereumController.js"),
    ),
    (
        "~system/Players",
        include_str!("../dcl/js/js_modules/Players.js"),
    ),
    (
        "~system/PortableExperiences",
        include_str!("../dcl/js/js_modules/PortableExperiences.js"),
    ),
    (
        "~system/RestrictedActions",
        include_str!("../dcl/js/js_modules/RestrictedActions.js"),
    ),
    (
        "~system/Runtime",
        include_str!("../dcl/js/js_modules/Runtime.js"),
    ),
    (
        "~system/Scene",
        include_str!("../dcl/js/js_modules/Scene.js"),
    ),
    (
        "~system/SignedFetch",
        include_str!("../dcl/js/js_modules/SignedFetch.js"),
    ),
    (
        "~system/Testing",
        include_str!("../dcl/js/js_modules/Testing.js"),
    ),
    (
        "~system/UserActionModule",
        include_str!("../dcl/js/js_modules/UserActionModule.js"),
    ),
    (
        "~system/UserIdentity",
        include_str!("../dcl/js/js_modules/UserIdentity.js"),
    ),
    ("fetch", include_str!("../dcl/js/js_modules/fetch.js")),
    ("ws", include_str!("../dcl/js/js_modules/ws.js")),
    (
        "text_encoding",
        include_str!("../dcl/js/js_modules/text_encoding.js"),
    ),
];

struct SceneSlot {
    scene_id: SceneId,
    scene_crdt: SharedSceneCrdtState,
    to_renderer: SyncSender<SceneResponse>,
    start: Instant,
    wallet: Option<EphemeralAuthChain>,
    definition: Arc<SceneEntityDefinition>,
    realm: DclSceneRealmData,
    comms_connected: bool,
    actions: Vec<RpcCall>,
    events: SceneEvents,
    fixed_skybox_time: bool,
}

thread_local! {
    static SCENES: RefCell<HashMap<i32, SceneSlot>> = RefCell::new(HashMap::new());
}

pub fn comms_connected(scene_id: SceneId) -> bool {
    SCENES.with(|s| {
        s.borrow()
            .get(&scene_id.0)
            .is_some_and(|slot| slot.comms_connected)
    })
}

pub fn spawn(
    data: SpawnDclSceneData,
    scene_crdt: SharedSceneCrdtState,
    from_renderer: Receiver<RendererResponse>,
) {
    let scene_id = data.scene_id;
    let to_renderer = data.thread_sender_to_main.clone();

    // Flush initial components before the first worker frame can clear their dirty flags.
    let main_crdt = read_file(&data.local_main_crdt_file_path).unwrap_or_default();
    if !main_crdt.is_empty() {
        let dirty = {
            let mut state = scene_crdt.lock().unwrap_or_else(|e| e.into_inner());
            process_many_messages(&mut DclReader::new(&main_crdt), &mut state);
            state.take_dirty()
        };
        let _ = to_renderer.try_send(SceneResponse::Ok {
            scene_id,
            dirty_crdt_state: Box::new(dirty),
            logs: vec![],
            delta: 0.0,
            rpc_calls: vec![],
            deno_memory_stats: None,
        });
    }

    let Some(scene_js) = read_file(&data.local_main_js_file_path) else {
        tracing::warn!("scene {scene_id:?}: no main.js, showing main.crdt only");
        spawn_static(scene_id, scene_crdt, to_renderer, from_renderer);
        return;
    };

    let definition = &data.scene_entity_definition;
    let scene_info = GetSceneInformationResponse {
        urn: definition.id.clone(),
        content: definition
            .content_mapping
            .files()
            .iter()
            .map(|(file, hash)| ContentMapping {
                file: file.clone(),
                hash: hash.clone(),
            })
            .collect(),
        metadata_json: serde_json::to_string(&definition.scene_meta_scene).unwrap_or_default(),
        base_url: definition.content_mapping.base_url.clone(),
    };
    let init = serde_json::json!({
        "loader": LOADER_SRC,
        "modules": MODULES.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<HashMap<_, _>>(),
        "sceneInfo": scene_info,
        "realm": data.realm_info,
        "worldTime": if data.fixed_skybox_time { 54000.0 } else { DclGlobalTime::get_world_time() },
        "locale": get_scene_locale(),
        "sceneComms": definition.id.starts_with("b64-"),
        "sceneCode": format!(
            "var module = {{ exports: {{}} }};(function(){{{}}})();module.exports.__after__ = async function() {{}};module.exports",
            String::from_utf8_lossy(&scene_js)
        ),
    });

    SCENES.with(|s| {
        s.borrow_mut().insert(
            scene_id.0,
            SceneSlot {
                scene_id,
                scene_crdt: scene_crdt.clone(),
                to_renderer: to_renderer.clone(),
                start: Instant::now(),
                wallet: data.ephemeral_wallet.clone(),
                definition: data.scene_entity_definition.clone(),
                realm: data.realm_info.clone(),
                comms_connected: false,
                actions: Vec::new(),
                events: SceneEvents::default(),
                fixed_skybox_time: data.fixed_skybox_time,
            },
        )
    });
    dcl_web_scene::spawn(scene_id.0, WORKER_SRC, &init.to_string(), on_worker_event);
    tracing::info!(
        "scene {scene_id:?}: started web worker ({} bytes of JS)",
        scene_js.len()
    );

    crate::scene_runner::tokio_runtime::TokioRuntime::spawn(async move {
        let mut main_crdt = Some(main_crdt);
        let mut from_renderer = from_renderer;
        while let Some(message) = from_renderer.recv().await {
            match message {
                RendererResponse::Ok {
                    dirty_crdt_state, ..
                } => {
                    SCENES.with(|scenes| {
                        if let Some(slot) = scenes.borrow_mut().get_mut(&scene_id.0) {
                            let state = scene_crdt.lock().unwrap_or_else(|e| e.into_inner());
                            slot.events.process(&state, &dirty_crdt_state, vec![]);
                            let context = serde_json::json!({
                                "worldTime": if slot.fixed_skybox_time { 54000.0 } else { DclGlobalTime::get_world_time() },
                                "locale": get_scene_locale(),
                                "events": slot.events.drain(),
                            });
                            dcl_web_scene::scene_state(scene_id.0, &context.to_string());
                        }
                    });
                    let data = serialize_for_scene(&dirty_crdt_state, &scene_crdt);
                    let main = main_crdt.take().unwrap_or_default();
                    let mut frame = Vec::with_capacity(4 + main.len() + data.len());
                    frame.extend_from_slice(&(main.len() as u32).to_le_bytes());
                    frame.extend_from_slice(&main);
                    frame.extend_from_slice(&data);
                    dcl_web_scene::post(scene_id.0, &frame);
                }
                RendererResponse::Kill => break,
            }
        }
        dcl_web_scene::kill(scene_id.0);
        SCENES.with(|s| s.borrow_mut().remove(&scene_id.0));
        let _ = to_renderer.try_send(SceneResponse::RemoveGodotScene(scene_id, vec![]));
    });
}

fn on_worker_event(scene_id: i32, kind: EventKind, payload: Vec<u8>) {
    match kind {
        EventKind::Crdt => SCENES.with(|s| {
            let mut scenes = s.borrow_mut();
            let Some(slot) = scenes.get_mut(&scene_id) else {
                return;
            };
            let dirty = {
                let mut state = slot.scene_crdt.lock().unwrap_or_else(|e| e.into_inner());
                process_many_messages(&mut DclReader::new(&payload), &mut state);
                state.take_dirty()
            };
            let _ = slot.to_renderer.try_send(SceneResponse::Ok {
                scene_id: slot.scene_id,
                dirty_crdt_state: Box::new(dirty),
                logs: vec![],
                delta: slot.start.elapsed().as_secs_f32(),
                rpc_calls: std::mem::take(&mut slot.actions),
                deno_memory_stats: None,
            });
        }),
        EventKind::Log => {
            tracing::info!("[scene {scene_id}] {}", String::from_utf8_lossy(&payload))
        }
        EventKind::Error => {
            tracing::warn!("[scene {scene_id}] {}", String::from_utf8_lossy(&payload))
        }
        EventKind::Exit => tracing::warn!("[scene {scene_id}] worker stopped"),
        EventKind::Call => on_call(scene_id, &payload),
        EventKind::CommsState => SCENES.with(|s| {
            if let Some(slot) = s.borrow_mut().get_mut(&scene_id) {
                slot.comms_connected = payload.first() == Some(&1);
            }
        }),
        EventKind::Comms => SCENES.with(|s| {
            if let Some(slot) = s.borrow().get(&scene_id) {
                if let Some(message) = super::scene_comms::decode(&slot.definition.id, &payload) {
                    dcl_web_scene::comms_receive(scene_id, &message);
                }
            }
        }),
        EventKind::CommsAdapter => SCENES.with(|s| {
            let scenes = s.borrow();
            let Some(slot) = scenes.get(&scene_id) else {
                return;
            };
            let (wallet, definition, realm) = (
                slot.wallet.clone(),
                slot.definition.clone(),
                slot.realm.clone(),
            );
            crate::scene_runner::tokio_runtime::TokioRuntime::spawn(async move {
                let result =
                    super::scene_comms::adapter(&definition.id, &realm.realm_name, wallet).await;
                let message = match result {
                    Ok(adapter) => serde_json::json!({ "adapter": adapter }),
                    Err(error) => serde_json::json!({ "error": error }),
                };
                dcl_web_scene::comms_adapter(scene_id, &message.to_string());
            });
        }),
    }
}

#[derive(Deserialize)]
struct Call {
    id: u64,
    op: String,
    #[serde(default)]
    args: Vec<serde_json::Value>,
}

/// The web side of the deno ops in `dcl/js/players.rs` and `dcl/js/fetch/signed_fetch.rs`.
fn on_call(scene_id: i32, payload: &[u8]) {
    let Ok(call) = serde_json::from_slice::<Call>(payload) else {
        return;
    };
    let id = call.id;
    let arg = |i: usize| call.args.get(i).and_then(|v| v.as_str()).map(str::to_owned);
    SCENES.with(|s| {
        let mut scenes = s.borrow_mut();
        let Some(slot) = scenes.get_mut(&scene_id) else {
            return;
        };
        if matches!(call.op.as_str(), "subscribe" | "unsubscribe") {
            let event = arg(0).unwrap_or_default();
            if call.op == "subscribe" {
                slot.events.subscribe(&event);
            } else {
                slot.events.unsubscribe(&event);
            }
            reply(scene_id, id, Ok(serde_json::Value::Null));
            return;
        }
        if call.op == "scene_action" {
            let result = if slot.actions.len() >= 128 {
                Err("Too many scene actions in one frame".to_owned())
            } else {
                serde_json::from_value::<super::scene_actions::SceneAction>(
                    call.args.first().cloned().unwrap_or_default(),
                )
                .map_err(|error| error.to_string())
                .and_then(|action| action.into_rpc())
            };
            match result {
                Ok((action, response)) => {
                    slot.actions.push(action);
                    if let Some(response) = response {
                        crate::scene_runner::tokio_runtime::TokioRuntime::spawn(async move {
                            let result = response
                                .await
                                .map_err(|error| error.to_string())
                                .and_then(|result| result)
                                .map(|()| serde_json::Value::Null);
                            reply(scene_id, id, result);
                        });
                    } else {
                        reply(scene_id, id, Ok(serde_json::Value::Null));
                    }
                }
                Err(error) => reply(scene_id, id, Err(error)),
            }
            return;
        }
        if call.op == "signed_fetch_headers" {
            let (wallet, definition, realm) = (
                slot.wallet.clone(),
                slot.definition.clone(),
                slot.realm.clone(),
            );
            let (uri, method) = (arg(0).unwrap_or_default(), arg(1));
            crate::scene_runner::tokio_runtime::TokioRuntime::spawn(async move {
                let headers = signed_fetch_headers(uri, method, wallet, &definition, realm).await;
                reply(
                    scene_id,
                    id,
                    headers
                        .map(|h| serde_json::json!(h))
                        .map_err(|e| e.to_string()),
                );
            });
            return;
        }
        if call.op == "comms_send" {
            let result = call
                .args
                .first()
                .cloned()
                .ok_or_else(|| "Missing scene message".to_owned())
                .and_then(|data| {
                    serde_json::from_value::<Vec<u8>>(data)
                        .map_err(|_| "Invalid scene message".to_owned())
                })
                .and_then(|data| super::scene_comms::encode(&slot.definition.id, data))
                .and_then(|packet| {
                    if dcl_web_scene::comms_send(scene_id, &packet, &arg(1).unwrap_or_default()) {
                        Ok(serde_json::Value::Null)
                    } else {
                        Err("Scene communications unavailable or queue full".to_owned())
                    }
                });
            reply(scene_id, id, result);
            return;
        }
        let state = slot.scene_crdt.lock().unwrap_or_else(|e| e.into_inner());
        let result = match call.op.as_str() {
            "get_player_data" => Ok(serde_json::json!(get_player_data(
                arg(0).unwrap_or_default(),
                &state
            ))),
            "get_players_in_scene" => Ok(serde_json::json!(get_players(&state, true))),
            "get_connected_players" => Ok(serde_json::json!(get_players(&state, false))),
            op => Err(format!("unknown call {op}")),
        };
        reply(scene_id, id, result);
    });
}

fn reply(scene_id: i32, id: u64, result: Result<serde_json::Value, String>) {
    let message = match result {
        Ok(value) => serde_json::json!({ "id": id, "ok": value }),
        Err(err) => serde_json::json!({ "id": id, "err": err }),
    };
    dcl_web_scene::reply(scene_id, &message.to_string());
}

/// The renderer-to-scene half of `op_crdt_recv_wait`, without logging, events or comms.
fn serialize_for_scene(dirty: &DirtyCrdtState, scene_crdt: &SharedSceneCrdtState) -> Vec<u8> {
    let state = scene_crdt.lock().unwrap_or_else(|e| e.into_inner());
    let mut buf = Vec::new();
    let mut writer = DclWriter::new(&mut buf);
    for (component_id, entities) in dirty.lww.iter() {
        for entity_id in entities {
            if !dirty.entities.died.contains(entity_id) {
                let _ = put_or_delete_lww_component(&state, entity_id, component_id, &mut writer);
            }
        }
    }
    for (component_id, entities) in dirty.gos.iter() {
        for (entity_id, count) in entities {
            if !dirty.entities.died.contains(entity_id) {
                let _ = append_gos_component(&state, entity_id, component_id, count, &mut writer);
            }
        }
    }
    drop(writer);
    buf
}

/// No JS: hand over main.crdt once, then answer every renderer tick with an empty update.
fn spawn_static(
    scene_id: SceneId,
    scene_crdt: SharedSceneCrdtState,
    to_renderer: SyncSender<SceneResponse>,
    mut from_renderer: Receiver<RendererResponse>,
) {
    crate::scene_runner::tokio_runtime::TokioRuntime::spawn(async move {
        let start = Instant::now();
        loop {
            let dirty = scene_crdt
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take_dirty();
            let _ = to_renderer.try_send(SceneResponse::Ok {
                scene_id,
                dirty_crdt_state: Box::new(dirty),
                logs: vec![],
                delta: start.elapsed().as_secs_f32(),
                rpc_calls: vec![],
                deno_memory_stats: None,
            });
            match from_renderer.recv().await {
                Some(RendererResponse::Ok { .. }) => {}
                _ => break,
            }
        }
        let _ = to_renderer.try_send(SceneResponse::RemoveGodotScene(scene_id, vec![]));
    });
}

fn read_file(path: &str) -> Option<Vec<u8>> {
    if path.is_empty() {
        return None;
    }
    let mut file = godot::classes::FileAccess::open(
        &godot::prelude::GString::from(path),
        godot::classes::file_access::ModeFlags::READ,
    )?;
    Some(file.get_buffer(file.get_length() as i64).to_vec())
}
