use std::{cell::RefCell, rc::Rc, sync::Arc};

use deno_core::{error::AnyError, op2, OpState};

use crate::{
    auth::ephemeral_auth_chain::EphemeralAuthChain,
    dcl::{scene_ops::signed_fetch_headers, DclSceneRealmData},
    realm::scene_definition::SceneEntityDefinition,
};

#[op2(async)]
#[serde]
pub async fn op_signed_fetch_headers(
    op_state: Rc<RefCell<OpState>>,
    #[string] uri: String,
    #[string] method: Option<String>,
) -> Result<Vec<(String, String)>, AnyError> {
    let (wallet, scene_entity_definition, realm_info) = {
        let state = op_state.borrow();
        (
            state.borrow::<Option<EphemeralAuthChain>>().clone(),
            state.borrow::<Arc<SceneEntityDefinition>>().clone(),
            state.borrow::<DclSceneRealmData>().clone(),
        )
    };
    signed_fetch_headers(uri, method, wallet, &scene_entity_definition, realm_info).await
}
