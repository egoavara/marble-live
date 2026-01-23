//! RoomService gRPC implementation

use crate::room_state::{RoomStatus, RoomStore};
use marble_proto::room::{
    room_service_server::RoomService, CreateRoomRequest, CreateRoomResponse, GetRoomRequest,
    GetRoomResponse, JoinRoomRequest, JoinRoomResponse, LeaveRoomRequest, LeaveRoomResponse,
    ListRoomsRequest, ListRoomsResponse, PlayerInfo, RoomInfo, StartGameRequest, StartGameResponse,
};
use tonic::{Request, Response, Status};

pub struct RoomServiceImpl {
    store: RoomStore,
    signaling_base_url: String,
}

impl RoomServiceImpl {
    pub fn new(store: RoomStore, signaling_base_url: String) -> Self {
        Self {
            store,
            signaling_base_url,
        }
    }

    fn room_to_proto(&self, room: &crate::room_state::Room) -> RoomInfo {
        // Get players sorted by join_order for deterministic ordering
        let players_sorted = room.players_by_join_order();

        RoomInfo {
            id: room.id.clone(),
            name: room.name.clone(),
            player_count: room.players.len() as u32,
            max_players: room.max_players,
            status: match room.status {
                RoomStatus::Waiting => marble_proto::room::RoomStatus::Waiting.into(),
                RoomStatus::Playing => marble_proto::room::RoomStatus::Playing.into(),
                RoomStatus::Finished => marble_proto::room::RoomStatus::Finished.into(),
            },
            players: players_sorted
                .iter()
                .map(|p| PlayerInfo {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    is_host: p.is_host,
                    is_connected: p.is_connected,
                    color_r: p.color.r as u32,
                    color_g: p.color.g as u32,
                    color_b: p.color.b as u32,
                    join_order: p.join_order,
                })
                .collect(),
            seed: room.seed,
        }
    }

    fn make_signaling_url(&self, room_id: &str) -> String {
        format!("{}/{}", self.signaling_base_url, room_id)
    }
}

#[tonic::async_trait]
impl RoomService for RoomServiceImpl {
    async fn create_room(
        &self,
        request: Request<CreateRoomRequest>,
    ) -> Result<Response<CreateRoomResponse>, Status> {
        let req = request.into_inner();
        let name = if req.name.is_empty() {
            "Unnamed Room".to_string()
        } else {
            req.name
        };
        // max_players: default to 4, cap at 8 to prevent DoS
        let max_players = if req.max_players == 0 { 4 } else { req.max_players.min(8) };

        let room = self.store.create_room(name, max_players);

        tracing::info!(room_id = %room.id, "Room created");

        Ok(Response::new(CreateRoomResponse {
            room_id: room.id.clone(),
            join_code: room.id.clone(), // For simplicity, use room_id as join_code
            signaling_url: self.make_signaling_url(&room.id),
        }))
    }

    async fn join_room(
        &self,
        request: Request<JoinRoomRequest>,
    ) -> Result<Response<JoinRoomResponse>, Status> {
        let req = request.into_inner();

        // Extract color from request
        let color = crate::room_state::Color::rgb(
            req.color_r as u8,
            req.color_g as u8,
            req.color_b as u8,
        );

        let Some((room, player, is_reconnect)) = self.store.join_room(&req.room_id, req.player_name, req.fingerprint, color) else {
            return Ok(Response::new(JoinRoomResponse {
                success: false,
                player_id: String::new(),
                room: None,
                error_message: "Room not found or full".to_string(),
                signaling_url: String::new(),
            }));
        };

        if is_reconnect {
            tracing::info!(room_id = %room.id, player_id = %player.id, "Player reconnected to room");
        } else {
            tracing::info!(room_id = %room.id, player_id = %player.id, "Player joined room");
        }

        Ok(Response::new(JoinRoomResponse {
            success: true,
            player_id: player.id,
            room: Some(self.room_to_proto(&room)),
            error_message: String::new(),
            signaling_url: self.make_signaling_url(&room.id),
        }))
    }

    async fn leave_room(
        &self,
        request: Request<LeaveRoomRequest>,
    ) -> Result<Response<LeaveRoomResponse>, Status> {
        let req = request.into_inner();
        let success = self.store.leave_room(&req.room_id, &req.player_id);

        if success {
            tracing::info!(room_id = %req.room_id, player_id = %req.player_id, "Player left room");
        }

        Ok(Response::new(LeaveRoomResponse { success }))
    }

    async fn list_rooms(
        &self,
        request: Request<ListRoomsRequest>,
    ) -> Result<Response<ListRoomsResponse>, Status> {
        let req = request.into_inner();
        let limit = if req.limit == 0 { 20 } else { req.limit };
        let (rooms, total) = self.store.list_rooms(req.offset, limit);

        Ok(Response::new(ListRoomsResponse {
            rooms: rooms.iter().map(|r| self.room_to_proto(r)).collect(),
            total,
        }))
    }

    async fn get_room(
        &self,
        request: Request<GetRoomRequest>,
    ) -> Result<Response<GetRoomResponse>, Status> {
        let req = request.into_inner();

        let room = self.store.get_room(&req.room_id).map(|r| self.room_to_proto(&r));

        Ok(Response::new(GetRoomResponse { room }))
    }

    async fn start_game(
        &self,
        request: Request<StartGameRequest>,
    ) -> Result<Response<StartGameResponse>, Status> {
        let req = request.into_inner();

        match self.store.start_game(&req.room_id, &req.player_id) {
            Ok(room) => {
                tracing::info!(room_id = %room.id, "Game started by host");
                Ok(Response::new(StartGameResponse {
                    success: true,
                    error_message: String::new(),
                    room: Some(self.room_to_proto(&room)),
                }))
            }
            Err(e) => Ok(Response::new(StartGameResponse {
                success: false,
                error_message: e,
                room: None,
            })),
        }
    }
}
