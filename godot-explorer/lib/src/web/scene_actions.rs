use crate::dcl::scene_apis::RpcCall;
use serde::Deserialize;
use tokio::sync::oneshot;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SceneAction {
    Move {
        position: [f32; 3],
        camera: Option<[f32; 3]>,
        avatar: Option<[f32; 3]>,
    },
    Emote {
        id: String,
        mask: i64,
    },
    SceneEmote {
        src: String,
        looping: bool,
        mask: i64,
    },
    StopEmote,
    Teleport {
        coordinates: Option<[i32; 2]>,
        realm: Option<String>,
    },
    ChangeRealm {
        realm: String,
        message: Option<String>,
    },
}

pub type ActionReply = oneshot::Receiver<Result<(), String>>;

impl SceneAction {
    pub fn into_rpc(self) -> Result<(RpcCall, Option<ActionReply>), String> {
        let mut reply = None;
        let rpc = match self {
            Self::Move {
                position,
                camera,
                avatar,
            } => {
                if position
                    .iter()
                    .chain(camera.iter().flatten())
                    .chain(avatar.iter().flatten())
                    .any(|v| !v.is_finite())
                {
                    return Err("Player targets must contain finite coordinates".into());
                }
                RpcCall::MovePlayerTo {
                    position_target: position,
                    camera_target: camera,
                    avatar_target: avatar,
                }
            }
            Self::Emote { id, mask } => {
                check_mask(mask)?;
                RpcCall::TriggerEmote { emote_id: id, mask }
            }
            Self::SceneEmote { src, looping, mask } => {
                check_mask(mask)?;
                RpcCall::TriggerSceneEmote {
                    emote_src: src,
                    looping,
                    mask,
                }
            }
            Self::StopEmote => RpcCall::StopEmote,
            Self::Teleport { coordinates, realm } => {
                let realm = realm.filter(|r| !r.is_empty());
                if coordinates.is_none() && realm.is_none() {
                    return Err("teleportTo requires worldCoordinates, a realm, or both".into());
                }
                let (tx, rx) = oneshot::channel();
                reply = Some(rx);
                RpcCall::TeleportTo {
                    world_coordinates: coordinates,
                    realm,
                    response: tx.into(),
                }
            }
            Self::ChangeRealm { realm, message } => {
                if realm.trim().is_empty() {
                    return Err("A realm is required".into());
                }
                let (tx, rx) = oneshot::channel();
                reply = Some(rx);
                RpcCall::ChangeRealm {
                    to: realm,
                    message,
                    response: tx.into(),
                }
            }
        };
        Ok((rpc, reply))
    }
}

fn check_mask(mask: i64) -> Result<(), String> {
    if matches!(mask, -1 | 0) {
        Ok(())
    } else {
        Err("Unsupported emote mask".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_preserves_independent_targets_and_rejects_nonfinite_values() {
        let (rpc, reply) = SceneAction::Move {
            position: [2., 3., 4.],
            camera: Some([1., 2., 3.]),
            avatar: None,
        }
        .into_rpc()
        .unwrap();
        assert!(matches!(
            rpc,
            RpcCall::MovePlayerTo {
                position_target: [2., 3., 4.],
                camera_target: Some([1., 2., 3.]),
                avatar_target: None
            }
        ));
        assert!(reply.is_none());
        assert!(SceneAction::Move {
            position: [f32::INFINITY, 0., 0.],
            camera: None,
            avatar: None
        }
        .into_rpc()
        .is_err());
    }

    #[test]
    fn teleport_preserves_optional_fields_and_reports_renderer_errors() {
        assert!(SceneAction::Teleport {
            coordinates: None,
            realm: Some(String::new())
        }
        .into_rpc()
        .is_err());
        let (rpc, reply) = SceneAction::Teleport {
            coordinates: None,
            realm: Some("test.dcl.eth".into()),
        }
        .into_rpc()
        .unwrap();
        let RpcCall::TeleportTo {
            world_coordinates,
            realm,
            response,
        } = rpc
        else {
            panic!("wrong RPC")
        };
        assert_eq!(world_coordinates, None);
        assert_eq!(realm.as_deref(), Some("test.dcl.eth"));
        response.send(Err("Primary Player is outside the scene".into()));
        assert!(reply.unwrap().try_recv().unwrap().is_err());
    }

    #[test]
    fn emote_mask_and_loop_reach_the_existing_handler() {
        let (rpc, _) = SceneAction::SceneEmote {
            src: "dance.glb".into(),
            looping: true,
            mask: 0,
        }
        .into_rpc()
        .unwrap();
        assert!(matches!(
            rpc,
            RpcCall::TriggerSceneEmote {
                looping: true,
                mask: 0,
                ..
            }
        ));
        assert!(SceneAction::Emote {
            id: "wave".into(),
            mask: 5
        }
        .into_rpc()
        .is_err());
        assert!(matches!(
            SceneAction::StopEmote.into_rpc().unwrap().0,
            RpcCall::StopEmote
        ));
    }

    #[test]
    fn utility_operations_are_not_exposed_as_gameplay_actions() {
        assert!(
            serde_json::from_str::<SceneAction>(r#"{"kind":"portable_spawn","pid":"x"}"#).is_err()
        );
        assert!(serde_json::from_str::<SceneAction>(r#"{"kind":"read_file","path":"x"}"#).is_err());
    }
}
