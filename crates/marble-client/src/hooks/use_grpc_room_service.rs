use marble_proto::room::room_service_client::RoomServiceClient;
use std::cell::RefCell;
use std::rc::Rc;
use tonic_web_wasm_client::Client;
use yew::prelude::*;

use crate::services::endpoint::grpc_base_url;

#[hook]
pub fn use_grpc_room_service() -> Rc<RefCell<RoomServiceClient<Client>>> {
    let client = use_mut_ref(|| {
        let client = Client::new(grpc_base_url());
        RoomServiceClient::new(client)
    });
    client
}
