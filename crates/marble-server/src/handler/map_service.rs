use marble_proto::map::{
    CreateMapRequest, CreateMapResponse, DeleteMapRequest, DeleteMapResponse, GetMapRequest,
    GetMapResponse, ListMapsRequest, ListMapsResponse, MapDetail, MapInfo, UpdateMapRequest,
    UpdateMapResponse,
};
use tonic::{Request, Response, Status};

use crate::service::database::Database;

use super::jwt::AuthenticatedUser;

pub struct MapServiceImpl {
    database: Database,
}

impl MapServiceImpl {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

fn stored_to_map_detail(m: &crate::service::database::StoredMap) -> MapDetail {
    MapDetail {
        map_id: m.map_id.clone(),
        name: m.name.clone(),
        description: m.description.clone(),
        creator_id: m.creator_id.clone(),
        tags: m.tags.clone(),
        created_at: m.created_at.to_rfc3339(),
        updated_at: m.updated_at.to_rfc3339(),
        data: m.data.clone(),
    }
}

fn stored_to_map_info(m: &crate::service::database::StoredMap) -> MapInfo {
    MapInfo {
        map_id: m.map_id.clone(),
        name: m.name.clone(),
        description: m.description.clone(),
        creator_id: m.creator_id.clone(),
        tags: m.tags.clone(),
        created_at: m.created_at.to_rfc3339(),
        updated_at: m.updated_at.to_rfc3339(),
    }
}

#[tonic::async_trait]
impl marble_proto::map::map_service_server::MapService for MapServiceImpl {
    async fn create_map(
        &self,
        request: Request<CreateMapRequest>,
    ) -> Result<Response<CreateMapResponse>, Status> {
        let user_id = request
            .extensions()
            .get::<AuthenticatedUser>()
            .ok_or_else(|| Status::unauthenticated("Authentication required"))?
            .user_id
            .clone();

        let req = request.into_inner();

        if req.name.is_empty() || req.name.len() > 64 {
            return Err(Status::invalid_argument("name must be 1-64 characters"));
        }

        let map = self
            .database
            .create_map(&user_id, &req.name, &req.description, req.tags, &req.data);

        tracing::info!(map_id = %map.map_id, creator = %user_id, "Map created");

        Ok(Response::new(CreateMapResponse {
            map: Some(stored_to_map_detail(&map)),
        }))
    }

    async fn get_map(
        &self,
        request: Request<GetMapRequest>,
    ) -> Result<Response<GetMapResponse>, Status> {
        let req = request.into_inner();

        let map = self
            .database
            .get_map(&req.map_id)
            .ok_or_else(|| Status::not_found("Map not found"))?;

        Ok(Response::new(GetMapResponse {
            map: Some(stored_to_map_detail(&map)),
        }))
    }

    async fn update_map(
        &self,
        request: Request<UpdateMapRequest>,
    ) -> Result<Response<UpdateMapResponse>, Status> {
        let user_id = request
            .extensions()
            .get::<AuthenticatedUser>()
            .ok_or_else(|| Status::unauthenticated("Authentication required"))?
            .user_id
            .clone();

        let req = request.into_inner();

        let name = if req.name.is_empty() {
            None
        } else {
            Some(req.name.as_str())
        };
        let description = if req.description.is_empty() {
            None
        } else {
            Some(req.description.as_str())
        };
        let data = if req.data.is_empty() {
            None
        } else {
            Some(req.data.as_str())
        };
        let tags = if req.update_tags {
            Some(req.tags)
        } else {
            None
        };

        let map = self
            .database
            .update_map(&req.map_id, &user_id, name, description, tags, data)
            .map_err(tonic::Status::from)?;

        tracing::info!(map_id = %req.map_id, "Map updated");

        Ok(Response::new(UpdateMapResponse {
            map: Some(stored_to_map_detail(&map)),
        }))
    }

    async fn delete_map(
        &self,
        request: Request<DeleteMapRequest>,
    ) -> Result<Response<DeleteMapResponse>, Status> {
        let user_id = request
            .extensions()
            .get::<AuthenticatedUser>()
            .ok_or_else(|| Status::unauthenticated("Authentication required"))?
            .user_id
            .clone();

        let req = request.into_inner();

        let map = self
            .database
            .delete_map(&req.map_id, &user_id)
            .map_err(tonic::Status::from)?;

        tracing::info!(map_id = %req.map_id, "Map deleted");

        Ok(Response::new(DeleteMapResponse {
            map: Some(stored_to_map_detail(&map)),
        }))
    }

    async fn list_maps(
        &self,
        request: Request<ListMapsRequest>,
    ) -> Result<Response<ListMapsResponse>, Status> {
        let req = request.into_inner();

        let creator_id = if req.creator_id.is_empty() {
            None
        } else {
            Some(req.creator_id.as_str())
        };
        let name_query = if req.name_query.is_empty() {
            None
        } else {
            Some(req.name_query.as_str())
        };

        let page_size = if req.page_size == 0 { 20 } else { req.page_size };

        let (maps, next_page_token, total_count) = self.database.list_maps(
            page_size,
            &req.page_token,
            creator_id,
            name_query,
            &req.tags,
        );

        let map_infos: Vec<MapInfo> = maps.iter().map(stored_to_map_info).collect();

        Ok(Response::new(ListMapsResponse {
            maps: map_infos,
            next_page_token,
            total_count,
        }))
    }
}
