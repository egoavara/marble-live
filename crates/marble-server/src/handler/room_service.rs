use marble_proto::room::{self, *};
use std::iter;
use tonic::{Request, Response, Status};

use crate::{
    common::{player::Player, room::Room},
    service::database::{Database, DatabaseError},
    util::{self, required_str},
};

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

    fn make_signaling_url(&self, room_id: &str) -> String {
        format!("{}/{}", self.signaling_base_url, room_id)
    }
}

#[tonic::async_trait]
impl marble_proto::room::room_service_server::RoomService for RoomServiceImpl {
    async fn create_room(
        &self,
        request: Request<CreateRoomRequest>,
    ) -> Result<Response<CreateRoomResponse>, Status> {
        let req = request.into_inner();
        let max_players = util::clamp(req.max_players, 2, 32);
        let room_id = uuid::Uuid::new_v4();
        let host = util::tonic_required!(req.host)?;
        required_str(&host.id, "Host ID is required, but got empty string")?;
        required_str(
            &host.secret,
            "Host secret is required, but got empty string",
        )?;
        let host = Player::new(host.id, host.secret);
        let room = Room::new(room_id.clone(), max_players, host);

        tracing::info!(room_id = %room.id(), "Room created");

        self.database.add_room(room);

        Ok(Response::new(CreateRoomResponse {
            room_id: room_id.to_string(),
            signaling_url: self.make_signaling_url(&room_id.to_string()),
        }))
    }

    async fn join_room(
        &self,
        request: Request<JoinRoomRequest>,
    ) -> Result<Response<JoinRoomResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;
        let player_auth = util::tonic_required!(req.player)?;
        let player = Player::new(player_auth.id, player_auth.secret);
        tracing::info!(room_id = %room_id, player_id = %player.id, "Player try to join room");
        match self.database.join_room(&room_id, player) {
            Ok((_room, topology)) => Ok(Response::new(JoinRoomResponse {
                signaling_url: self.make_signaling_url(&room_id.to_string()),
                topology: Some(topology),
            })),
            Err(err) => Err(err.into()),
        }
    }

    async fn start_room(
        &self,
        request: Request<StartRoomRequest>,
    ) -> Result<Response<StartRoomResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;
        let player_auth = util::tonic_required!(req.player)?;

        let started_at = self.database.start_room(&room_id, &player_auth)?;

        tracing::info!(room_id = %room_id, "Room started");

        Ok(Response::new(StartRoomResponse {
            started_at: started_at.to_rfc3339(),
        }))
    }

    async fn start_game(
        &self,
        request: Request<StartGameRequest>,
    ) -> Result<Response<StartGameResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;
        let player_auth = util::tonic_required!(req.player)?;

        let (newly_started, started_at) =
            self.database
                .start_game(&room_id, &player_auth, req.start_frame, req.rng_seed)?;

        if newly_started {
            tracing::info!(
                room_id = %room_id,
                start_frame = req.start_frame,
                rng_seed = req.rng_seed,
                "Game started (marbles spawned)"
            );
        } else {
            tracing::debug!(room_id = %room_id, "Game already started");
        }

        Ok(Response::new(StartGameResponse {
            success: true,
            already_started: !newly_started,
            started_at: started_at.to_rfc3339(),
        }))
    }

    async fn report_arrival(
        &self,
        request: Request<ReportArrivalRequest>,
    ) -> Result<Response<ReportArrivalResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;
        let player_auth = util::tonic_required!(req.player)?;

        let game_ended = self.database.report_arrival(
            &room_id,
            &player_auth,
            &req.arrived_player_id,
            req.arrival_frame,
            req.rank,
        )?;

        tracing::info!(
            room_id = %room_id,
            arrived_player_id = %req.arrived_player_id,
            arrival_frame = req.arrival_frame,
            rank = req.rank,
            game_ended = game_ended,
            "Player arrived at hole"
        );

        Ok(Response::new(ReportArrivalResponse {
            success: true,
            game_ended,
        }))
    }

    async fn kick_room(
        &self,
        request: Request<KickRoomRequest>,
    ) -> Result<Response<KickRoomResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;
        let player_auth = util::tonic_required!(req.player)?;
        let target_player_id = &req.target_player;

        self.database
            .kick_room(&room_id, &player_auth, target_player_id)?;

        tracing::info!(room_id = %room_id, player_id = %player_auth.id, target_player_id = %target_player_id, "Player kicked from room");

        Ok(Response::new(KickRoomResponse {}))
    }

    async fn get_room(
        &self,
        request: Request<GetRoomRequest>,
    ) -> Result<Response<GetRoomResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        let Some(room) = self.database.get_room(&room_id) else {
            return Err(DatabaseError::RoomNotFound.into());
        };

        let config = room.topology_config();
        let results: Vec<room::PlayerResult> = room
            .game_results()
            .iter()
            .map(|r| room::PlayerResult {
                player_id: r.player_id.clone(),
                rank: r.rank,
                arrival_frame: r.arrival_frame,
            })
            .collect();

        Ok(Response::new(GetRoomResponse {
            room: Some(RoomInfo {
                id: room.id().to_string(),
                max_players: room.max_players(),
                state: room.state() as i32,
                started_at: room
                    .started_at()
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
                lockstep_delay_frames: config.lockstep_delay_frames,
                gossip_ttl: config.gossip_ttl,
                mesh_group_size: config.mesh_group_size,
                peer_connections: config.peer_connections,
                start_frame: room.game_start_frame().unwrap_or(0),
                rng_seed: room.game_rng_seed().unwrap_or(0),
                results,
            }),
        }))
    }

    async fn get_room_player(
        &self,
        request: Request<GetRoomPlayerRequest>,
    ) -> Result<Response<GetRoomPlayerResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;

        let Some(room) = self.database.get_room(&room_id) else {
            return Err(DatabaseError::RoomNotFound.into());
        };

        let host = room.host_player();
        let host_iter = iter::once(PlayerInfo {
            id: host.id.clone(),
            is_host: true,
            display_id: "#224466".to_string(),
        });
        let others_iter = room.iter_other_players().map(|p| PlayerInfo {
            id: p.id.clone(),
            is_host: false,
            display_id: "#664422".to_string(),
        });

        Ok(Response::new(GetRoomPlayerResponse {
            players: host_iter.chain(others_iter).collect(),
        }))
    }

    async fn report_connection(
        &self,
        request: Request<ReportConnectionRequest>,
    ) -> Result<Response<ReportConnectionResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;
        let player_auth = util::tonic_required!(req.player)?;

        let new_topology =
            self.database
                .report_connection(&room_id, &player_auth.id, req.peer_statuses)?;

        Ok(Response::new(ReportConnectionResponse {
            topology_changed: new_topology.is_some(),
            new_topology,
        }))
    }

    async fn get_topology(
        &self,
        request: Request<GetTopologyRequest>,
    ) -> Result<Response<GetTopologyResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;
        let player_auth = util::tonic_required!(req.player)?;

        let topology = self.database.get_topology(&room_id, &player_auth.id)?;

        Ok(Response::new(GetTopologyResponse {
            topology: Some(topology),
        }))
    }

    async fn register_peer_id(
        &self,
        request: Request<RegisterPeerIdRequest>,
    ) -> Result<Response<RegisterPeerIdResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;
        let player_auth = util::tonic_required!(req.player)?;
        let peer_id = &req.peer_id;

        required_str(peer_id, "peer_id is required")?;

        let updated_topology = self
            .database
            .register_peer_id(&room_id, &player_auth, peer_id)?;

        tracing::info!(
            room_id = %room_id,
            player_id = %player_auth.id,
            peer_id = %peer_id,
            "Registered peer_id for player"
        );

        Ok(Response::new(RegisterPeerIdResponse {
            success: true,
            updated_topology,
        }))
    }

    async fn get_room_topology(
        &self,
        request: Request<GetRoomTopologyRequest>,
    ) -> Result<Response<GetRoomTopologyResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;
        let player_auth = util::tonic_required!(req.player_auth)?;

        let topologies = self.database.get_room_topology(&room_id, &player_auth)?;

        let players = topologies
            .into_iter()
            .map(|(player_id, topology)| room::PlayerTopologyInfo {
                player_id,
                topology: Some(topology),
                is_connected: true, // TODO: Track actual connection status if needed
            })
            .collect();

        Ok(Response::new(GetRoomTopologyResponse { players }))
    }

    async fn resolve_peer_ids(
        &self,
        request: Request<ResolvePeerIdsRequest>,
    ) -> Result<Response<ResolvePeerIdsResponse>, Status> {
        let req = request.into_inner();
        let room_id = util::tonic_uuid!(&req.room_id)?;
        let player_auth = util::tonic_required!(req.player)?;

        let peer_to_player =
            self.database
                .resolve_peer_ids(&room_id, &player_auth, &req.peer_ids)?;

        Ok(Response::new(ResolvePeerIdsResponse { peer_to_player }))
    }
}
