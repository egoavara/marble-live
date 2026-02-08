use marble_proto::room::{
    self, CreateRoomRequest, CreateRoomResponse, GetRoomRequest, GetRoomResponse,
    GetRoomTopologyRequest, GetRoomTopologyResponse, GetRoomUsersRequest, GetRoomUsersResponse,
    GetTopologyRequest, GetTopologyResponse, JoinRoomRequest, JoinRoomResponse, KickPlayerRequest,
    KickPlayerResponse, ListRoomsRequest, ListRoomsResponse, RegisterPeerIdRequest,
    RegisterPeerIdResponse, ReportArrivalRequest, ReportArrivalResponse, ReportConnectionRequest,
    ReportConnectionResponse, ResolvePeerIdsRequest, ResolvePeerIdsResponse, RoomRole, RoomState,
    RoomSummary, StartGameRequest, StartGameResponse,
};
use tonic::{Request, Response, Status};

use crate::{
    common::room::Room,
    service::database::{Database, DatabaseError},
    util::{self, required_str},
};

use super::jwt::AuthenticatedUser;

pub struct RoomServiceImpl {
    database: Database,
    signaling_base_url: String,
}

impl RoomServiceImpl {
    pub fn new(database: Database, signaling_base_url: String) -> Self {
        Self {
            database,
            signaling_base_url,
        }
    }

    fn get_user_id(request_extensions: &http::Extensions) -> Result<String, Status> {
        request_extensions
            .get::<AuthenticatedUser>()
            .map(|u| u.user_id.clone())
            .ok_or_else(|| Status::unauthenticated("Authentication required"))
    }
}

#[tonic::async_trait]
impl marble_proto::room::room_service_server::RoomService for RoomServiceImpl {
    async fn create_room(
        &self,
        request: Request<CreateRoomRequest>,
    ) -> Result<Response<CreateRoomResponse>, Status> {
        let user_id = Self::get_user_id(request.extensions())?;
        let req = request.into_inner();

        let max_players = util::clamp(req.max_players, 2, 32);
        let room_id = uuid::Uuid::new_v4();
        let room_name = if req.room_name.is_empty() {
            format!("Room-{}", &room_id.to_string()[..8])
        } else {
            req.room_name
        };

        let room = Room::new(
            room_id,
            room_name,
            req.map_id,
            max_players,
            req.is_public,
            user_id.clone(),
            self.signaling_base_url.clone(),
        );

        let topology = room
            .get_topology(&user_id)
            .unwrap_or_default();

        tracing::info!(room_id = %room.id(), host = %user_id, "Room created");

        let room_info = room.to_room_info();
        self.database.add_room(room);

        Ok(Response::new(CreateRoomResponse {
            room: Some(room_info),
            topology: Some(topology),
        }))
    }

    async fn get_room(
        &self,
        request: Request<GetRoomRequest>,
    ) -> Result<Response<GetRoomResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        let room = self
            .database
            .get_room(&room_id)
            .ok_or(DatabaseError::RoomNotFound)?;

        Ok(Response::new(GetRoomResponse {
            room: Some(room.to_room_info()),
        }))
    }

    async fn list_rooms(
        &self,
        request: Request<ListRoomsRequest>,
    ) -> Result<Response<ListRoomsResponse>, Status> {
        let req = request.into_inner();

        let states: Vec<RoomState> = req
            .states
            .iter()
            .filter_map(|&s| RoomState::try_from(s).ok())
            .collect();

        let map_id = if req.map_id.is_empty() {
            None
        } else {
            Some(req.map_id.as_str())
        };
        let name_query = if req.name_query.is_empty() {
            None
        } else {
            Some(req.name_query.as_str())
        };

        let page_size = if req.page_size == 0 { 20 } else { req.page_size };

        let (rooms, next_page_token, total_count) = self.database.list_rooms(
            page_size,
            &req.page_token,
            &states,
            map_id,
            name_query,
            req.has_available_slots,
        );

        let summaries: Vec<RoomSummary> = rooms.iter().map(Room::to_room_summary).collect();

        Ok(Response::new(ListRoomsResponse {
            rooms: summaries,
            next_page_token,
            total_count,
        }))
    }

    async fn join_room(
        &self,
        request: Request<JoinRoomRequest>,
    ) -> Result<Response<JoinRoomResponse>, Status> {
        let user_id = Self::get_user_id(request.extensions())?;
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        let role = req.role.and_then(|r| RoomRole::try_from(r).ok());

        tracing::info!(room_id = %room_id, user_id = %user_id, "User trying to join room");

        match self.database.join_room(&room_id, user_id, role) {
            Ok((room, topology)) => Ok(Response::new(JoinRoomResponse {
                room: Some(room.to_room_info()),
                topology: Some(topology),
            })),
            Err(err) => Err(err.into()),
        }
    }

    async fn get_room_users(
        &self,
        request: Request<GetRoomUsersRequest>,
    ) -> Result<Response<GetRoomUsersResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        let room = self
            .database
            .get_room(&room_id)
            .ok_or(DatabaseError::RoomNotFound)?;

        Ok(Response::new(GetRoomUsersResponse {
            users: room.get_room_users(),
        }))
    }

    async fn kick_player(
        &self,
        request: Request<KickPlayerRequest>,
    ) -> Result<Response<KickPlayerResponse>, Status> {
        let user_id = Self::get_user_id(request.extensions())?;
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        let room = self
            .database
            .kick_user(&room_id, &user_id, &req.target_user_id)?;

        tracing::info!(
            room_id = %room_id,
            host = %user_id,
            target = %req.target_user_id,
            "Player kicked from room"
        );

        Ok(Response::new(KickPlayerResponse {
            room: Some(room.to_room_info()),
        }))
    }

    async fn start_game(
        &self,
        request: Request<StartGameRequest>,
    ) -> Result<Response<StartGameResponse>, Status> {
        let user_id = Self::get_user_id(request.extensions())?;
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        let (newly_started, room) =
            self.database
                .start_game(&room_id, &user_id, req.start_frame)?;

        if newly_started {
            tracing::info!(
                room_id = %room_id,
                start_frame = req.start_frame,
                "Game started (marbles spawned)"
            );
        } else {
            tracing::debug!(room_id = %room_id, "Game already started");
        }

        Ok(Response::new(StartGameResponse {
            room: Some(room.to_room_info()),
        }))
    }

    async fn report_arrival(
        &self,
        request: Request<ReportArrivalRequest>,
    ) -> Result<Response<ReportArrivalResponse>, Status> {
        let user_id = Self::get_user_id(request.extensions())?;
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        let (game_ended, room) = self.database.report_arrival(
            &room_id,
            &user_id,
            &req.arrived_user_id,
            req.arrival_frame,
            req.rank,
        )?;

        tracing::info!(
            room_id = %room_id,
            arrived_user_id = %req.arrived_user_id,
            arrival_frame = req.arrival_frame,
            rank = req.rank,
            game_ended = game_ended,
            "Player arrived at hole"
        );

        Ok(Response::new(ReportArrivalResponse {
            room: Some(room.to_room_info()),
        }))
    }

    async fn register_peer_id(
        &self,
        request: Request<RegisterPeerIdRequest>,
    ) -> Result<Response<RegisterPeerIdResponse>, Status> {
        let user_id = Self::get_user_id(request.extensions())?;
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        required_str(&req.peer_id, "peer_id is required")?;

        let updated_topology = self
            .database
            .register_peer_id(&room_id, &user_id, &req.peer_id)?;

        tracing::info!(
            room_id = %room_id,
            user_id = %user_id,
            peer_id = %req.peer_id,
            "Registered peer_id for user"
        );

        Ok(Response::new(RegisterPeerIdResponse {
            updated_topology,
        }))
    }

    async fn report_connection(
        &self,
        request: Request<ReportConnectionRequest>,
    ) -> Result<Response<ReportConnectionResponse>, Status> {
        let user_id = Self::get_user_id(request.extensions())?;
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        let new_topology =
            self.database
                .report_connection(&room_id, &user_id, &req.peer_statuses)?;

        Ok(Response::new(ReportConnectionResponse {
            topology_changed: new_topology.is_some(),
            new_topology,
        }))
    }

    async fn get_topology(
        &self,
        request: Request<GetTopologyRequest>,
    ) -> Result<Response<GetTopologyResponse>, Status> {
        let user_id = Self::get_user_id(request.extensions())?;
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        let topology = self.database.get_topology(&room_id, &user_id)?;

        Ok(Response::new(GetTopologyResponse {
            topology: Some(topology),
        }))
    }

    async fn get_room_topology(
        &self,
        request: Request<GetRoomTopologyRequest>,
    ) -> Result<Response<GetRoomTopologyResponse>, Status> {
        let user_id = Self::get_user_id(request.extensions())?;
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        let topologies = self.database.get_room_topology(&room_id, &user_id)?;

        let players = topologies
            .into_iter()
            .map(|(uid, topology)| room::PlayerTopologyInfo {
                user_id: uid,
                topology: Some(topology),
                is_connected: true,
            })
            .collect();

        Ok(Response::new(GetRoomTopologyResponse { players }))
    }

    async fn resolve_peer_ids(
        &self,
        request: Request<ResolvePeerIdsRequest>,
    ) -> Result<Response<ResolvePeerIdsResponse>, Status> {
        let user_id = Self::get_user_id(request.extensions())?;
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        let peer_to_user =
            self.database
                .resolve_peer_ids(&room_id, &user_id, &req.peer_ids)?;

        Ok(Response::new(ResolvePeerIdsResponse { peer_to_user }))
    }
}
