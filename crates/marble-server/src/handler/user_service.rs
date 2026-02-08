use marble_proto::user::{
    login_request, GetUserRequest, GetUserResponse, GetUsersRequest, GetUsersResponse,
    LoginRequest, LoginResponse, UpdateProfileRequest, UpdateProfileResponse, UserInfo,
};
use tonic::{Request, Response, Status};

use crate::service::database::{AuthType, Database};

use super::jwt::JwtManager;

pub struct UserServiceImpl {
    database: Database,
    jwt_manager: JwtManager,
}

impl UserServiceImpl {
    pub fn new(database: Database, jwt_manager: JwtManager) -> Self {
        Self {
            database,
            jwt_manager,
        }
    }
}

#[tonic::async_trait]
impl marble_proto::user::user_service_server::UserService for UserServiceImpl {
    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let req = request.into_inner();

        let method = req
            .method
            .ok_or_else(|| Status::invalid_argument("Login method is required"))?;

        let (user, _is_new) = match method {
            login_request::Method::Anonymous(anon) => {
                if anon.display_name.is_empty() {
                    return Err(Status::invalid_argument("display_name is required"));
                }
                if anon.salt.is_empty() {
                    return Err(Status::invalid_argument("salt is required"));
                }
                if anon.fingerprint.is_empty() {
                    return Err(Status::invalid_argument("fingerprint is required"));
                }

                self.database.find_or_create_anonymous_user(
                    &anon.display_name,
                    &anon.salt,
                    &anon.fingerprint,
                )
            }
            login_request::Method::Sso(_sso) => {
                return Err(Status::unimplemented("SSO login is not yet supported"));
            }
        };

        let (token, expires_at) = self.jwt_manager.generate_token(&user.user_id);

        let user_info = UserInfo {
            user_id: user.user_id,
            display_name: user.display_name,
            auth_type: match user.auth_type {
                AuthType::Anonymous => auth_type::AuthType::Anonymous.into(),
                AuthType::Sso => auth_type::AuthType::Sso.into(),
            },
            created_at: user.created_at.to_rfc3339(),
        };

        Ok(Response::new(LoginResponse {
            user: Some(user_info),
            token,
            token_expires_at: expires_at.to_rfc3339(),
        }))
    }

    async fn get_user(
        &self,
        request: Request<GetUserRequest>,
    ) -> Result<Response<GetUserResponse>, Status> {
        let req = request.into_inner();

        let user = self
            .database
            .get_user(&req.user_id)
            .ok_or_else(|| Status::not_found("User not found"))?;

        Ok(Response::new(GetUserResponse {
            user: Some(UserInfo {
                user_id: user.user_id,
                display_name: user.display_name,
                auth_type: match user.auth_type {
                    AuthType::Anonymous => auth_type::AuthType::Anonymous.into(),
                    AuthType::Sso => auth_type::AuthType::Sso.into(),
                },
                created_at: user.created_at.to_rfc3339(),
            }),
        }))
    }

    async fn get_users(
        &self,
        request: Request<GetUsersRequest>,
    ) -> Result<Response<GetUsersResponse>, Status> {
        let req = request.into_inner();

        if req.user_ids.len() > 100 {
            return Err(Status::invalid_argument(
                "Maximum 100 user_ids per request",
            ));
        }

        let users = self.database.get_users(&req.user_ids);

        let user_infos: Vec<UserInfo> = users
            .into_iter()
            .map(|u| UserInfo {
                user_id: u.user_id,
                display_name: u.display_name,
                auth_type: match u.auth_type {
                    AuthType::Anonymous => auth_type::AuthType::Anonymous.into(),
                    AuthType::Sso => auth_type::AuthType::Sso.into(),
                },
                created_at: u.created_at.to_rfc3339(),
            })
            .collect();

        Ok(Response::new(GetUsersResponse { users: user_infos }))
    }

    async fn update_profile(
        &self,
        request: Request<UpdateProfileRequest>,
    ) -> Result<Response<UpdateProfileResponse>, Status> {
        let user_id = request
            .extensions()
            .get::<super::jwt::AuthenticatedUser>()
            .ok_or_else(|| Status::unauthenticated("Authentication required"))?
            .user_id
            .clone();

        let req = request.into_inner();

        if req.display_name.is_empty() || req.display_name.len() > 32 {
            return Err(Status::invalid_argument(
                "display_name must be 1-32 characters",
            ));
        }

        let user = self
            .database
            .update_user_profile(&user_id, &req.display_name)
            .map_err(tonic::Status::from)?;

        Ok(Response::new(UpdateProfileResponse {
            user: Some(UserInfo {
                user_id: user.user_id,
                display_name: user.display_name,
                auth_type: match user.auth_type {
                    AuthType::Anonymous => auth_type::AuthType::Anonymous.into(),
                    AuthType::Sso => auth_type::AuthType::Sso.into(),
                },
                created_at: user.created_at.to_rfc3339(),
            }),
        }))
    }
}

// Module-level helper to convert AuthType enum
mod auth_type {
    pub enum AuthType {
        Anonymous,
        Sso,
    }

    impl From<AuthType> for i32 {
        fn from(val: AuthType) -> Self {
            match val {
                AuthType::Anonymous => 1,
                AuthType::Sso => 2,
            }
        }
    }
}
