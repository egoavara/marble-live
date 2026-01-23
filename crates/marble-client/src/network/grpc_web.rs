//! gRPC-Web client for WASM
//!
//! Simple HTTP-based gRPC-Web client that works in browser.

use gloo::net::http::Request;
use prost::Message;
use tracing::debug;

const GRPC_WEB_CONTENT_TYPE: &str = "application/grpc-web+proto";

#[derive(Debug)]
pub enum GrpcError {
    Network(String),
    Decode(String),
    GrpcStatus { code: u32, message: String },
}

impl std::fmt::Display for GrpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(e) => write!(f, "Network error: {e}"),
            Self::Decode(e) => write!(f, "Decode error: {e}"),
            Self::GrpcStatus { code, message } => {
                write!(f, "gRPC error (code={code}): {message}")
            }
        }
    }
}

fn encode_grpc_message<M: Message>(msg: &M) -> Vec<u8> {
    let encoded = msg.encode_to_vec();
    let len = encoded.len() as u32;

    let mut frame = Vec::with_capacity(5 + encoded.len());
    frame.push(0); // compression flag
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&encoded);
    frame
}

fn decode_grpc_message<M: Message + Default>(data: &[u8]) -> Result<M, GrpcError> {
    if data.len() < 5 {
        return Err(GrpcError::Decode("Response too short".to_string()));
    }

    let _compression = data[0];
    let len = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;

    if data.len() < 5 + len {
        return Err(GrpcError::Decode(format!(
            "Response truncated: expected {} bytes, got {}",
            5 + len,
            data.len()
        )));
    }

    M::decode(&data[5..5 + len]).map_err(|e| GrpcError::Decode(e.to_string()))
}

pub async fn call<Req: Message, Resp: Message + Default>(
    base_url: &str,
    service: &str,
    method: &str,
    request: &Req,
) -> Result<Resp, GrpcError> {
    let url = format!("{base_url}/{service}/{method}");
    let body = encode_grpc_message(request);

    debug!("gRPC request: POST {url}");

    let response = Request::post(&url)
        .header("Content-Type", GRPC_WEB_CONTENT_TYPE)
        .header("Accept", GRPC_WEB_CONTENT_TYPE)
        .header("x-grpc-web", "1")
        .body(body)
        .map_err(|e| GrpcError::Network(e.to_string()))?
        .send()
        .await
        .map_err(|e| GrpcError::Network(e.to_string()))?;

    let status = response.status();
    debug!("gRPC response status: {status}");

    // Non-200 status means something went wrong
    if status != 200 {
        let text = response.text().await.unwrap_or_default();
        return Err(GrpcError::Network(format!(
            "HTTP {status}: {text}"
        )));
    }

    // Check for grpc-status in headers (trailer)
    if let Some(grpc_status) = response.headers().get("grpc-status") {
        let code: u32 = grpc_status.parse().unwrap_or(2);
        if code != 0 {
            let message = response
                .headers()
                .get("grpc-message")
                .unwrap_or_default();
            return Err(GrpcError::GrpcStatus { code, message });
        }
    }

    let bytes = response
        .binary()
        .await
        .map_err(|e| GrpcError::Network(e.to_string()))?;

    debug!("gRPC response body: {} bytes", bytes.len());

    if bytes.is_empty() {
        return Err(GrpcError::Decode("Empty response body".to_string()));
    }

    decode_grpc_message(&bytes)
}

/// Room service client
pub mod room {
    use super::*;
    use marble_proto::room::*;

    const SERVICE: &str = "room.RoomService";

    pub struct RoomClient {
        base_url: String,
    }

    impl RoomClient {
        pub fn new(base_url: impl Into<String>) -> Self {
            Self {
                base_url: base_url.into(),
            }
        }

        pub async fn create_room(&self, name: &str, max_players: u32) -> Result<CreateRoomResponse, GrpcError> {
            let req = CreateRoomRequest {
                name: name.to_string(),
                max_players,
            };
            call(&self.base_url, SERVICE, "CreateRoom", &req).await
        }

        pub async fn join_room(
            &self,
            room_id: &str,
            player_name: &str,
            fingerprint: &str,
            color_r: u32,
            color_g: u32,
            color_b: u32,
        ) -> Result<JoinRoomResponse, GrpcError> {
            let req = JoinRoomRequest {
                room_id: room_id.to_string(),
                player_name: player_name.to_string(),
                fingerprint: fingerprint.to_string(),
                color_r,
                color_g,
                color_b,
            };
            call(&self.base_url, SERVICE, "JoinRoom", &req).await
        }

        pub async fn leave_room(&self, room_id: &str, player_id: &str) -> Result<LeaveRoomResponse, GrpcError> {
            let req = LeaveRoomRequest {
                room_id: room_id.to_string(),
                player_id: player_id.to_string(),
            };
            call(&self.base_url, SERVICE, "LeaveRoom", &req).await
        }

        pub async fn list_rooms(&self, offset: u32, limit: u32) -> Result<ListRoomsResponse, GrpcError> {
            let req = ListRoomsRequest { offset, limit };
            call(&self.base_url, SERVICE, "ListRooms", &req).await
        }

        pub async fn get_room(&self, room_id: &str) -> Result<GetRoomResponse, GrpcError> {
            let req = GetRoomRequest {
                room_id: room_id.to_string(),
            };
            call(&self.base_url, SERVICE, "GetRoom", &req).await
        }

        pub async fn start_game(&self, room_id: &str, player_id: &str) -> Result<StartGameResponse, GrpcError> {
            let req = StartGameRequest {
                room_id: room_id.to_string(),
                player_id: player_id.to_string(),
            };
            call(&self.base_url, SERVICE, "StartGame", &req).await
        }
    }
}
