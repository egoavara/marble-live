use chrono::Utc;
use marble_proto::avatar::{
    AvatarInfo, AvatarType, ColorAvatar, GetAvatarRequest, GetAvatarResponse, GetAvatarsRequest,
    GetAvatarsResponse, OutlineConfig, OutlineStyle, SetAvatarRequest, SetAvatarResponse,
};
use tonic::{Request, Response, Status};

use crate::service::database::Database;

use super::jwt::JwtManager;

pub struct AvatarServiceImpl {
    database: Database,
    #[allow(dead_code)]
    jwt_manager: JwtManager,
}

impl AvatarServiceImpl {
    pub fn new(database: Database, jwt_manager: JwtManager) -> Self {
        Self {
            database,
            jwt_manager,
        }
    }
}

fn default_avatar(user_id: &str) -> AvatarInfo {
    AvatarInfo {
        user_id: user_id.to_string(),
        avatar_type: AvatarType::Color.into(),
        avatar: Some(marble_proto::avatar::avatar_info::Avatar::Color(
            ColorAvatar {
                fill_color: 0xFFFFFFFF, // white
            },
        )),
        outline: Some(OutlineConfig {
            color: 0x000000FF, // black
            width: 1.0,
            style: OutlineStyle::Solid.into(),
        }),
        updated_at: Utc::now().to_rfc3339(),
    }
}

#[tonic::async_trait]
impl marble_proto::avatar::avatar_service_server::AvatarService for AvatarServiceImpl {
    async fn set_avatar(
        &self,
        request: Request<SetAvatarRequest>,
    ) -> Result<Response<SetAvatarResponse>, Status> {
        let user_id = request
            .extensions()
            .get::<super::jwt::AuthenticatedUser>()
            .ok_or_else(|| Status::unauthenticated("Authentication required"))?
            .user_id
            .clone();

        let req = request.into_inner();

        let avatar_type = req.avatar_type();
        if avatar_type == AvatarType::Unspecified {
            return Err(Status::invalid_argument("avatar_type is required"));
        }

        // Validate avatar matches avatar_type
        match (avatar_type, &req.avatar) {
            (AvatarType::Color, Some(marble_proto::avatar::set_avatar_request::Avatar::Color(_))) => {}
            (AvatarType::Image, Some(marble_proto::avatar::set_avatar_request::Avatar::Image(_))) => {}
            _ => {
                return Err(Status::invalid_argument(
                    "avatar field must match avatar_type",
                ));
            }
        }

        let now = Utc::now();

        // Convert set_avatar_request::Avatar to avatar_info::Avatar
        let avatar_oneof = match req.avatar {
            Some(marble_proto::avatar::set_avatar_request::Avatar::Color(c)) => {
                Some(marble_proto::avatar::avatar_info::Avatar::Color(c))
            }
            Some(marble_proto::avatar::set_avatar_request::Avatar::Image(i)) => {
                Some(marble_proto::avatar::avatar_info::Avatar::Image(i))
            }
            None => None,
        };

        let avatar_info = AvatarInfo {
            user_id: user_id.clone(),
            avatar_type: avatar_type.into(),
            avatar: avatar_oneof,
            outline: req.outline,
            updated_at: now.to_rfc3339(),
        };

        self.database.set_avatar(&user_id, avatar_info.clone());

        Ok(Response::new(SetAvatarResponse {
            avatar: Some(avatar_info),
        }))
    }

    async fn get_avatar(
        &self,
        request: Request<GetAvatarRequest>,
    ) -> Result<Response<GetAvatarResponse>, Status> {
        let req = request.into_inner();

        if req.user_id.is_empty() {
            return Err(Status::invalid_argument("user_id is required"));
        }

        let avatar = self
            .database
            .get_avatar(&req.user_id)
            .unwrap_or_else(|| default_avatar(&req.user_id));

        Ok(Response::new(GetAvatarResponse {
            avatar: Some(avatar),
        }))
    }

    async fn get_avatars(
        &self,
        request: Request<GetAvatarsRequest>,
    ) -> Result<Response<GetAvatarsResponse>, Status> {
        let req = request.into_inner();

        if req.user_ids.len() > 100 {
            return Err(Status::invalid_argument(
                "Maximum 100 user_ids per request",
            ));
        }

        let avatars: Vec<AvatarInfo> = req
            .user_ids
            .iter()
            .map(|uid| {
                self.database
                    .get_avatar(uid)
                    .unwrap_or_else(|| default_avatar(uid))
            })
            .collect();

        Ok(Response::new(GetAvatarsResponse { avatars }))
    }
}
