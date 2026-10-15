use crate::{
    dcl::{
        crdt::{DirtyCrdtState, SceneCrdtState},
        scene_apis::RpcCall,
        scene_events::{SceneEvent, SceneEvents},
    },
    godot_classes::dcl_scene_locale::get_scene_locale,
};
use deno_core::{op2, OpDecl, OpState};
use ethers_core::types::H160;

pub fn ops() -> Vec<OpDecl> {
    vec![op_subscribe(), op_unsubscribe(), op_send_batch()]
}

fn events(state: &mut OpState) -> &mut SceneEvents {
    if !state.has::<SceneEvents>() {
        state.put(SceneEvents::default());
    }
    state.borrow_mut::<SceneEvents>()
}

#[op2(fast)]
fn op_subscribe(state: &mut OpState, #[string] id: &str) {
    if events(state).subscribe(id) {
        state
            .borrow_mut::<Vec<RpcCall>>()
            .push(RpcCall::SceneLocaleRequested {
                locale: get_scene_locale(),
            });
    }
}

#[op2(fast)]
fn op_unsubscribe(state: &mut OpState, #[string] id: &str) {
    events(state).unsubscribe(id);
}

#[op2]
#[serde]
fn op_send_batch(state: &mut OpState) -> Vec<SceneEvent> {
    events(state).drain()
}

pub fn process_events(
    state: &mut OpState,
    crdt: &SceneCrdtState,
    dirty: &DirtyCrdtState,
    comms: Vec<(H160, Vec<u8>)>,
) {
    events(state).process(crdt, dirty, comms);
}
