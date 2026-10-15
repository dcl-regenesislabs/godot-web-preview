use std::time::Duration;

use prost::Message;
use serde::Deserialize;

use crate::{
    auth::{ephemeral_auth_chain::EphemeralAuthChain, wallet::sign_request},
    dcl::components::proto_components::kernel::comms::rfc4,
};

const MAX_PACKET: usize = 64 * 1024;

pub async fn adapter(
    scene_id: &str,
    realm_name: &str,
    wallet: Option<EphemeralAuthChain>,
) -> Result<String, String> {
    let wallet = wallet.ok_or("Scene communications require a guest identity")?;
    let url = crate::urls::comms_gatekeeper_local();
    let uri = url.parse().map_err(|_| "Invalid scene gatekeeper URL")?;
    let metadata = serde_json::json!({ "sceneId": scene_id, "realmName": realm_name });
    let headers = sign_request("POST", &uri, &wallet, &metadata).await;
    let mut request = reqwest::Client::new()
        .post(url)
        .timeout(Duration::from_secs(20))
        .json(&metadata);
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let response = request
        .send()
        .await
        .map_err(|_| "Scene gatekeeper unavailable")?;
    if !response.status().is_success() {
        return Err(format!("Scene gatekeeper returned {}", response.status()));
    }
    #[derive(Deserialize)]
    struct Adapter {
        adapter: String,
    }
    response
        .json::<Adapter>()
        .await
        .map(|value| value.adapter)
        .map_err(|_| "Invalid scene gatekeeper response".to_owned())
}

pub fn encode(scene_id: &str, data: Vec<u8>) -> Result<Vec<u8>, String> {
    if data.is_empty() || !matches!(data[0], 1 | 2) || data.len() > MAX_PACKET {
        return Err("Invalid scene message".to_owned());
    }
    let packet = rfc4::Packet {
        protocol_version: 100,
        message: Some(rfc4::packet::Message::Scene(rfc4::Scene {
            scene_id: scene_id.to_owned(),
            data,
        })),
    };
    if packet.encoded_len() > MAX_PACKET {
        return Err("Scene message exceeds the packet limit".to_owned());
    }
    Ok(packet.encode_to_vec())
}

pub fn decode(scene_id: &str, payload: &[u8]) -> Option<Vec<u8>> {
    let sender_len = *payload.first()? as usize;
    let sender = payload.get(1..1 + sender_len)?;
    let bytes = payload.get(1 + sender_len..)?;
    if bytes.len() > MAX_PACKET {
        return None;
    }
    let packet = rfc4::Packet::decode(bytes).ok()?;
    let Some(rfc4::packet::Message::Scene(scene)) = packet.message else {
        return None;
    };
    if scene.scene_id != scene_id || !matches!(scene.data.first(), Some(1 | 2)) {
        return None;
    }
    let mut result = Vec::with_capacity(1 + sender_len + scene.data.len());
    result.push(scene.data[0]);
    result.push(sender_len as u8);
    result.extend_from_slice(sender);
    result.extend_from_slice(&scene.data[1..]);
    Some(result)
}
