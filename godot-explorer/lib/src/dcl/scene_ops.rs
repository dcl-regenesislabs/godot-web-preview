//! Scene API bodies shared by the deno ops (`dcl/js`) and the web scene host (`web/scene_host`).

use http::Uri;
use serde::Serialize;

use crate::{
    auth::{ephemeral_auth_chain::EphemeralAuthChain, wallet::sign_request},
    dcl::{
        components::{proto_components::common::Color3, SceneEntityId},
        crdt::{SceneCrdtState, SceneCrdtStateProtoComponents},
        scene_apis::{AvatarForUserData, UserData},
        DclSceneRealmData,
    },
    realm::scene_definition::SceneEntityDefinition,
};

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct SignedFetchMetaRealm {
    hostname: String,
    protocol: String,
    server_name: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct SignedFetchMeta {
    origin: Option<String>,
    scene_id: Option<String>,
    parcel: Option<String>,
    tld: Option<String>,
    network: Option<String>,
    is_guest: Option<bool>,
    realm: SignedFetchMetaRealm,
    signer: String,
}

pub async fn signed_fetch_headers(
    uri: String,
    method: Option<String>,
    wallet: Option<EphemeralAuthChain>,
    scene_entity_definition: &SceneEntityDefinition,
    realm_info: DclSceneRealmData,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    if let Some(ephemeral_wallet) = wallet {
        // get host name from url
        let realm_uri = Uri::try_from(realm_info.base_url)?;
        let hostname = realm_uri
            .host()
            .ok_or(anyhow::Error::msg("Invalid host name"))?;
        let network = "mainnet"; // TODO: this could be taken from `network_id`?

        let meta: SignedFetchMeta = SignedFetchMeta {
            origin: Some("https://decentraland.org".to_owned()),
            scene_id: Some(scene_entity_definition.id.clone()),
            parcel: Some(format!(
                "{},{}",
                scene_entity_definition.get_base_parcel().x,
                scene_entity_definition.get_base_parcel().y
            )),
            tld: Some("org".to_owned()),
            network: Some(network.to_owned()),
            is_guest: Some(false),
            realm: SignedFetchMetaRealm {
                hostname: hostname.to_owned(),
                protocol: "v3".to_owned(),
                server_name: realm_info.realm_name,
            },
            signer: "decentraland-kernel-scene".to_owned(),
        };

        // The metadata must reach the server unfolded: services behind an SDK7 `signedFetch`
        // authorize on `authMetadata.sceneId` / `.realm.serverName`, and folding the metadata
        // would deliver those keys lowercased — read back as undefined, after a successful
        // verification.
        let headers = sign_request(
            method.as_deref().unwrap_or("get"),
            &Uri::try_from(uri)?,
            &ephemeral_wallet,
            meta,
        )
        .await;

        Ok(headers)
    } else {
        Err(anyhow::Error::msg("There is no wallet to sign headers."))
    }
}

pub fn get_players(crdt_state: &SceneCrdtState, only_in_scene: bool) -> Vec<String> {
    let player_identity_data_component =
        SceneCrdtStateProtoComponents::get_player_identity_data(crdt_state);
    let internal_player_data = crdt_state.get_internal_player_data();

    player_identity_data_component
        .values
        .iter()
        .filter(|(entity_id, entry)| {
            let Some(_) = entry.value.as_ref() else {
                return false;
            };
            let Some(internal_player_data_entry) = internal_player_data.values.get(entity_id)
            else {
                return false;
            };
            if only_in_scene {
                let Some(value) = internal_player_data_entry.value.as_ref() else {
                    return false;
                };

                return value.inside;
            }
            true
        })
        .map(|v| {
            v.1.value
                .as_ref()
                .expect("previously acceded to filter")
                .address
                .clone()
        })
        .collect::<Vec<String>>()
}

pub fn get_player_data(user_id: String, crdt_state: &SceneCrdtState) -> Option<UserData> {
    let player_identity_data_component =
        SceneCrdtStateProtoComponents::get_player_identity_data(crdt_state);
    let avatar_base_component = SceneCrdtStateProtoComponents::get_avatar_base(crdt_state);
    let avatar_equipped_data_component =
        SceneCrdtStateProtoComponents::get_avatar_equipped_data(crdt_state);

    let (player_entity_id, player_entry) =
        player_identity_data_component
            .values
            .iter()
            .find(|(entity_id, entry)| {
                if user_id.is_empty() {
                    if entity_id == &&SceneEntityId::PLAYER {
                        return true;
                    }
                } else if let Some(data) = entry.value.as_ref() {
                    return data.address == user_id;
                }
                false
            })?;

    let player_identity_data_value = player_entry.value.as_ref()?;
    let avatar_base_value = avatar_base_component
        .values
        .get(player_entity_id)?
        .value
        .as_ref()?;
    let avatar_equipped_data_value = avatar_equipped_data_component
        .values
        .get(player_entity_id)?
        .value
        .as_ref()?;

    let user_data = UserData {
        display_name: avatar_base_value.name.clone(),
        public_key: if player_identity_data_value.is_guest {
            None
        } else {
            Some(player_identity_data_value.address.clone())
        },
        has_connected_web3: !player_identity_data_value.is_guest,
        user_id: player_identity_data_value.address.clone(),
        // TODO: implement this when version is in the avatar components
        version: 0, // TODO: we don't have this information in the avatar components
        avatar: Some(AvatarForUserData {
            body_shape: avatar_base_value.body_shape_urn.clone(),
            skin_color: avatar_base_value
                .skin_color
                .as_ref()
                .unwrap_or(&Color3::black())
                .to_color_string(),
            hair_color: avatar_base_value
                .hair_color
                .as_ref()
                .unwrap_or(&Color3::black())
                .to_color_string(),
            eye_color: avatar_base_value
                .eyes_color
                .as_ref()
                .unwrap_or(&Color3::black())
                .to_color_string(),
            wearables: avatar_equipped_data_value.wearable_urns.clone(),
            snapshots: None, // TODO: we don't have this information in the avatar components
        }),
    };
    Some(user_data)
}
