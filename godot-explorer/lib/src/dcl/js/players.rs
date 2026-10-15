use std::{cell::RefCell, rc::Rc};

use deno_core::{error::AnyError, op2, OpDecl, OpState};

use crate::dcl::scene_apis::{LocalCall, UserData};

pub fn ops() -> Vec<OpDecl> {
    vec![
        op_get_player_data(),
        op_get_connected_players(),
        op_get_players_in_scene(),
    ]
}

#[op2(async)]
#[serde]
async fn op_get_player_data(
    op_state: Rc<RefCell<OpState>>,
    #[string] user_id: String,
) -> Result<Option<UserData>, AnyError> {
    let (sx, rx) = tokio::sync::oneshot::channel::<Option<UserData>>();

    op_state
        .borrow_mut()
        .borrow_mut::<Vec<LocalCall>>()
        .push(LocalCall::PlayersGetPlayerData {
            user_id,
            response: sx.into(),
        });

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

#[op2(async)]
#[serde]
async fn op_get_players_in_scene(op_state: Rc<RefCell<OpState>>) -> Result<Vec<String>, AnyError> {
    let (sx, rx) = tokio::sync::oneshot::channel::<Vec<String>>();

    op_state.borrow_mut().borrow_mut::<Vec<LocalCall>>().push(
        LocalCall::PlayersGetPlayersInScene {
            response: sx.into(),
        },
    );

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

#[op2(async)]
#[serde]
async fn op_get_connected_players(op_state: Rc<RefCell<OpState>>) -> Result<Vec<String>, AnyError> {
    let (sx, rx) = tokio::sync::oneshot::channel::<Vec<String>>();

    op_state.borrow_mut().borrow_mut::<Vec<LocalCall>>().push(
        LocalCall::PlayersGetConnectedPlayers {
            response: sx.into(),
        },
    );

    rx.await.map_err(|e| anyhow::anyhow!(e))
}
