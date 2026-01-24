use marble_proto::room::{PlayerAuth, PlayerInfo};

#[derive(Debug, Clone)]
pub struct Player {
    pub id: String,
    pub secret: String,
}

impl Player {
    pub fn new(id: String, secret: String) -> Self {
        Self { id, secret }
    }
}
impl From<PlayerAuth> for Player {
    fn from(auth: PlayerAuth) -> Self {
        Player::new(auth.id, auth.secret)
    }
}
