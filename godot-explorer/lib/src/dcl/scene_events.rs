use std::collections::{HashMap, HashSet};

use ethers_core::types::H160;
use serde::Serialize;
use serde_json::{json, Value};

use super::{
    components::{
        proto_components::sdk::components::{
            common::{InputAction, PointerEventType},
            EmoteState,
        },
        SceneComponentId, SceneEntityId,
    },
    crdt::{
        grow_only_set::GenericGrowOnlySetComponentOperation,
        last_write_wins::LastWriteWinsComponentOperation, DirtyCrdtState, SceneCrdtState,
        SceneCrdtStateProtoComponents,
    },
};
use crate::godot_classes::dcl_scene_locale::{get_scene_locale, get_scene_locale_version};

#[derive(Serialize)]
pub struct SceneEvent {
    pub generic: GenericEvent,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericEvent {
    pub event_id: String,
    pub event_data: String,
}

#[derive(Default)]
pub struct SceneEvents {
    subscriptions: HashSet<String>,
    pending: Vec<SceneEvent>,
    players: Option<HashMap<SceneEntityId, String>>,
    inside: HashSet<SceneEntityId>,
    locale_version: u64,
    locale_reported: bool,
    started: bool,
}

impl SceneEvents {
    pub fn subscribe(&mut self, id: &str) -> bool {
        if !matches!(
            id,
            "playerConnected"
                | "playerDisconnected"
                | "onEnterScene"
                | "onLeaveScene"
                | "sceneStart"
                | "playerExpression"
                | "playerClicked"
                | "comms"
                | "localeChanged"
        ) {
            return false;
        }
        if !self.subscriptions.insert(id.to_owned()) {
            return false;
        }
        if id == "localeChanged" {
            self.locale_version = get_scene_locale_version();
            return !std::mem::replace(&mut self.locale_reported, true);
        }
        false
    }

    pub fn unsubscribe(&mut self, id: &str) {
        self.subscriptions.remove(id);
        self.pending.retain(|event| event.generic.event_id != id);
    }

    pub fn drain(&mut self) -> Vec<SceneEvent> {
        std::mem::take(&mut self.pending)
    }

    fn emit(&mut self, id: &str, body: Value) {
        if self.subscriptions.contains(id) {
            self.pending.push(SceneEvent {
                generic: GenericEvent {
                    event_id: id.to_owned(),
                    event_data: body.to_string(),
                },
            });
        }
    }

    pub fn process(
        &mut self,
        state: &SceneCrdtState,
        dirty: &DirtyCrdtState,
        comms: Vec<(H160, Vec<u8>)>,
    ) {
        self.players(state, dirty);
        self.expressions(state, dirty);
        self.clicks(state, dirty);
        for (sender, bytes) in comms {
            if let Some(message) = bytes.get(1..) {
                let sender = if sender == H160::from_low_u64_be(1) {
                    "authoritative-server".to_owned()
                } else {
                    format!("{sender:#x}")
                };
                self.emit(
                    "comms",
                    json!({ "sender": sender, "message": String::from_utf8_lossy(message) }),
                );
            }
        }
        let tick = SceneCrdtStateProtoComponents::get_engine_info(state)
            .get(&SceneEntityId::ROOT)
            .and_then(|entry| entry.value.as_ref())
            .map_or(0, |info| info.tick_number);
        if tick >= 4 && !self.started {
            self.started = true;
            self.emit("sceneStart", json!({}));
        }
        let version = get_scene_locale_version();
        if version != self.locale_version {
            self.locale_version = version;
            self.emit("localeChanged", json!({ "locale": get_scene_locale() }));
        }
    }

    fn players(&mut self, state: &SceneCrdtState, dirty: &DirtyCrdtState) {
        if ![
            "playerConnected",
            "playerDisconnected",
            "onEnterScene",
            "onLeaveScene",
        ]
        .iter()
        .any(|id| self.subscriptions.contains(*id))
        {
            self.players = None;
            self.inside.clear();
            return;
        }
        let identities = SceneCrdtStateProtoComponents::get_player_identity_data(state);
        let internal = state.get_internal_player_data();
        let mut players = self.players.take().unwrap_or_else(|| {
            self.inside = internal
                .values
                .iter()
                .filter_map(|(entity, entry)| {
                    entry.value.as_ref().filter(|v| v.inside).map(|_| *entity)
                })
                .collect();
            identities
                .values
                .iter()
                .filter_map(|(entity, entry)| {
                    entry.value.as_ref().map(|v| (*entity, v.address.clone()))
                })
                .collect()
        });
        if let Some(entities) = dirty.lww.get(&SceneComponentId::PLAYER_IDENTITY_DATA) {
            for entity in entities {
                if let Some(identity) = identities
                    .values
                    .get(entity)
                    .and_then(|entry| entry.value.as_ref())
                {
                    players.insert(*entity, identity.address.clone());
                    self.emit("playerConnected", json!({ "userId": identity.address }));
                } else if let Some(address) = players.remove(entity) {
                    self.emit("playerDisconnected", json!({ "userId": address }));
                    if self.inside.remove(entity) {
                        self.emit("onLeaveScene", json!({ "userId": address }));
                    }
                }
            }
        }
        if let Some(entities) = dirty.lww.get(&SceneComponentId::INTERNAL_PLAYER_DATA) {
            for entity in entities {
                let inside = internal
                    .values
                    .get(entity)
                    .and_then(|entry| entry.value.as_ref())
                    .is_some_and(|value| value.inside);
                let changed = if inside {
                    self.inside.insert(*entity)
                } else {
                    self.inside.remove(entity)
                };
                if changed {
                    if let Some(address) = players.get(entity) {
                        self.emit(
                            if inside {
                                "onEnterScene"
                            } else {
                                "onLeaveScene"
                            },
                            json!({ "userId": address }),
                        );
                    }
                }
            }
        }
        self.players = Some(players);
    }

    fn expressions(&mut self, state: &SceneCrdtState, dirty: &DirtyCrdtState) {
        if !self.subscriptions.contains("playerExpression") {
            return;
        }
        let count = dirty
            .gos
            .get(&SceneComponentId::AVATAR_EMOTE_COMMAND)
            .and_then(|entries| entries.get(&SceneEntityId::PLAYER))
            .copied()
            .unwrap_or(0);
        if let Some(commands) = SceneCrdtStateProtoComponents::get_avatar_emote_command(state)
            .get(&SceneEntityId::PLAYER)
        {
            for command in commands.iter().skip(commands.len().saturating_sub(count)) {
                if command
                    .state
                    .is_none_or(|state| state == EmoteState::EsStarted as i32)
                {
                    self.emit(
                        "playerExpression",
                        json!({ "expressionId": command.emote_urn }),
                    );
                }
            }
        }
    }

    fn clicks(&mut self, state: &SceneCrdtState, dirty: &DirtyCrdtState) {
        if !self.subscriptions.contains("playerClicked") {
            return;
        }
        let Some(entries) = dirty.gos.get(&SceneComponentId::POINTER_EVENTS_RESULT) else {
            return;
        };
        let identities = SceneCrdtStateProtoComponents::get_player_identity_data(state);
        let results = SceneCrdtStateProtoComponents::get_pointer_events_result(state);
        for (entity, count) in entries {
            let Some(identity) = identities
                .values
                .get(entity)
                .and_then(|entry| entry.value.as_ref())
            else {
                continue;
            };
            let Some(events) = results.get(entity) else {
                continue;
            };
            for event in events.iter().skip(events.len().saturating_sub(*count)) {
                if event.button() != InputAction::IaPointer
                    || event.state() != PointerEventType::PetDown
                {
                    continue;
                }
                let hit = event.hit.as_ref();
                let vector = |v: Option<&super::components::proto_components::common::Vector3>| json!({ "x": v.map_or(0., |p| p.x), "y": v.map_or(0., |p| p.y), "z": v.map_or(0., |p| p.z) });
                self.emit(
                    "playerClicked",
                    json!({ "userId": identity.address, "ray": {
                        "origin": vector(hit.and_then(|h| h.global_origin.as_ref())),
                        "direction": vector(hit.and_then(|h| h.direction.as_ref())),
                        "distance": hit.map_or(0., |h| h.length)
                    }}),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dcl::components::{
        internal_player_data::InternalPlayerData,
        proto_components::sdk::components::{
            PbAvatarEmoteCommand, PbEngineInfo, PbPlayerIdentityData, PbPointerEventsResult,
        },
    };

    #[test]
    fn unsupported_subscriptions_do_not_allocate_host_state() {
        let mut events = SceneEvents::default();
        for index in 0..1000 {
            events.subscribe(&format!("unknown-{index}"));
        }
        assert!(events.subscriptions.is_empty());
    }

    fn process(events: &mut SceneEvents, state: &mut SceneCrdtState) -> Vec<(String, Value)> {
        let dirty = state.take_dirty();
        events.process(state, &dirty, vec![]);
        events
            .drain()
            .into_iter()
            .map(|event| {
                (
                    event.generic.event_id,
                    serde_json::from_str(&event.generic.event_data).unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn player_events_track_entry_exit_and_disconnect_without_replaying_history() {
        let mut state = SceneCrdtState::from_proto();
        let mut events = SceneEvents::default();
        for id in [
            "playerConnected",
            "playerDisconnected",
            "onEnterScene",
            "onLeaveScene",
        ] {
            events.subscribe(id);
        }
        process(&mut events, &mut state);
        SceneCrdtStateProtoComponents::get_player_identity_data_mut(&mut state).put(
            SceneEntityId::PLAYER,
            Some(PbPlayerIdentityData {
                address: "0xplayer".into(),
                ..Default::default()
            }),
        );
        state.get_internal_player_data_mut().put(
            SceneEntityId::PLAYER,
            Some(InternalPlayerData { inside: true }),
        );
        assert_eq!(
            process(&mut events, &mut state),
            vec![
                ("playerConnected".into(), json!({"userId":"0xplayer"})),
                ("onEnterScene".into(), json!({"userId":"0xplayer"}))
            ]
        );
        assert!(process(&mut events, &mut state).is_empty());
        state.get_internal_player_data_mut().put(
            SceneEntityId::PLAYER,
            Some(InternalPlayerData { inside: false }),
        );
        assert_eq!(
            process(&mut events, &mut state),
            vec![("onLeaveScene".into(), json!({"userId":"0xplayer"}))]
        );
        SceneCrdtStateProtoComponents::get_player_identity_data_mut(&mut state)
            .put(SceneEntityId::PLAYER, None);
        assert_eq!(
            process(&mut events, &mut state),
            vec![("playerDisconnected".into(), json!({"userId":"0xplayer"}))]
        );
    }

    #[test]
    fn expressions_emit_new_starts_only() {
        let mut state = SceneCrdtState::from_proto();
        let mut events = SceneEvents::default();
        events.subscribe("playerExpression");
        for (name, status) in [
            ("wave", EmoteState::EsStarted),
            ("wave", EmoteState::EsFinished),
            ("dance", EmoteState::EsStarted),
        ] {
            SceneCrdtStateProtoComponents::get_avatar_emote_command_mut(&mut state).append(
                SceneEntityId::PLAYER,
                PbAvatarEmoteCommand {
                    emote_urn: name.into(),
                    state: Some(status as i32),
                    ..Default::default()
                },
            );
            let result = process(&mut events, &mut state);
            if status == EmoteState::EsStarted {
                assert_eq!(
                    result,
                    vec![("playerExpression".into(), json!({"expressionId":name}))]
                );
            } else {
                assert!(result.is_empty());
            }
        }
    }

    #[test]
    fn clicks_use_the_newest_result_and_ignore_button_releases() {
        let mut state = SceneCrdtState::from_proto();
        let mut events = SceneEvents::default();
        events.subscribe("playerClicked");
        SceneCrdtStateProtoComponents::get_player_identity_data_mut(&mut state).put(
            SceneEntityId::PLAYER,
            Some(PbPlayerIdentityData {
                address: "0xplayer".into(),
                ..Default::default()
            }),
        );
        for kind in [
            PointerEventType::PetDown,
            PointerEventType::PetUp,
            PointerEventType::PetDown,
        ] {
            SceneCrdtStateProtoComponents::get_pointer_events_result_mut(&mut state).append(
                SceneEntityId::PLAYER,
                PbPointerEventsResult {
                    button: InputAction::IaPointer as i32,
                    state: kind as i32,
                    ..Default::default()
                },
            );
            let result = process(&mut events, &mut state);
            assert_eq!(result.len(), usize::from(kind == PointerEventType::PetDown));
            if let Some((id, data)) = result.first() {
                assert_eq!(id, "playerClicked");
                assert_eq!(data["userId"], "0xplayer");
                assert_eq!(data["ray"]["distance"], 0.);
            }
        }
    }

    #[test]
    fn scene_start_fires_once_and_unsubscribe_clears_pending_events() {
        let mut state = SceneCrdtState::from_proto();
        let mut events = SceneEvents::default();
        events.subscribe("sceneStart");
        for tick in [3, 4, 5, 6] {
            SceneCrdtStateProtoComponents::get_engine_info_mut(&mut state).put(
                SceneEntityId::ROOT,
                Some(PbEngineInfo {
                    tick_number: tick,
                    ..Default::default()
                }),
            );
            assert_eq!(
                process(&mut events, &mut state).len(),
                usize::from(tick == 4)
            );
        }
        events.subscribe("comms");
        events.process(
            &state,
            &DirtyCrdtState::default(),
            vec![(H160::from_low_u64_be(1), vec![1, b'h', b'i'])],
        );
        let body: Value = serde_json::from_str(&events.drain()[0].generic.event_data).unwrap();
        assert_eq!(
            body,
            json!({"sender":"authoritative-server", "message":"hi"})
        );
        events.process(
            &state,
            &DirtyCrdtState::default(),
            vec![(H160::zero(), vec![1, b'x'])],
        );
        events.unsubscribe("comms");
        assert!(events.drain().is_empty());
    }
}
