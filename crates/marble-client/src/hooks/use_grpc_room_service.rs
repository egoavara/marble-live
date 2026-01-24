use marble_proto::room::room_service_client::RoomServiceClient;
use std::cell::RefCell;
use std::rc::Rc;
use tonic_web_wasm_client::Client;
use yew::prelude::*;

#[hook]
pub fn use_grpc_room_service() -> Rc<RefCell<RoomServiceClient<Client>>> {
    let client = use_mut_ref(|| {
        let Some(window) = web_sys::window() else {
            panic!("No window object available");
        };
        let origin = window.location().origin().unwrap();
        let client = Client::new(format!("{}/grpc", origin));
        RoomServiceClient::new(client)
    });
    client
}
