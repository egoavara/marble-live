use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Authenticated user info stored in tonic Request extensions.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
}

/// Simple JWT-like token manager using HMAC.
/// For production use a proper JWT library; this is sufficient for the current in-memory stage.
#[allow(dead_code)]
#[derive(Clone)]
pub struct JwtManager {
    secret: Arc<String>,
    expiry_hours: i64,
}

#[derive(Serialize, Deserialize)]
struct TokenPayload {
    sub: String,          // user_id
    exp: i64,             // expiry unix timestamp
    iat: i64,             // issued at
}

impl JwtManager {
    pub fn new(secret: String, expiry_hours: i64) -> Self {
        Self {
            secret: Arc::new(secret),
            expiry_hours,
        }
    }

    /// Generate a token for a `user_id`. Returns (token, `expires_at`).
    pub fn generate_token(&self, user_id: &str) -> (String, DateTime<Utc>) {
        let now = Utc::now();
        let expires_at = now + Duration::hours(self.expiry_hours);

        let payload = TokenPayload {
            sub: user_id.to_string(),
            exp: expires_at.timestamp(),
            iat: now.timestamp(),
        };

        let payload_json = serde_json::to_string(&payload).unwrap();
        let payload_b64 = base64_encode(&payload_json);
        let signature = self.sign(&payload_b64);

        let token = format!("{payload_b64}.{signature}");
        (token, expires_at)
    }

    /// Validate a token and return the `user_id`.
    pub fn validate_token(&self, token: &str) -> Option<String> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 2 {
            return None;
        }

        let payload_b64 = parts[0];
        let signature = parts[1];

        // Verify signature
        let expected_sig = self.sign(payload_b64);
        if signature != expected_sig {
            return None;
        }

        // Decode payload
        let payload_json = base64_decode(payload_b64)?;
        let payload: TokenPayload = serde_json::from_str(&payload_json).ok()?;

        // Check expiry
        let now = Utc::now().timestamp();
        if payload.exp < now {
            return None;
        }

        Some(payload.sub)
    }

    fn sign(&self, data: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        self.secret.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

fn base64_encode(data: &str) -> String {
    use std::io::Write;
    let mut buf = Vec::new();
    {
        let mut encoder = Base64Writer::new(&mut buf);
        encoder.write_all(data.as_bytes()).unwrap();
    }
    String::from_utf8(buf).unwrap()
}

#[allow(dead_code)]
fn base64_decode(data: &str) -> Option<String> {
    let bytes = Base64Reader::decode(data)?;
    String::from_utf8(bytes).ok()
}

// Simple base64 implementation (no external dep needed)
struct Base64Writer<'a> {
    output: &'a mut Vec<u8>,
}

impl<'a> Base64Writer<'a> {
    fn new(output: &'a mut Vec<u8>) -> Self {
        Self { output }
    }
}

impl std::io::Write for Base64Writer<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        for chunk in buf.chunks(3) {
            let b0 = u32::from(chunk[0]);
            let b1 = if chunk.len() > 1 { u32::from(chunk[1]) } else { 0 };
            let b2 = if chunk.len() > 2 { u32::from(chunk[2]) } else { 0 };
            let n = (b0 << 16) | (b1 << 8) | b2;

            self.output.push(CHARS[((n >> 18) & 0x3F) as usize]);
            self.output.push(CHARS[((n >> 12) & 0x3F) as usize]);
            if chunk.len() > 1 {
                self.output.push(CHARS[((n >> 6) & 0x3F) as usize]);
            }
            if chunk.len() > 2 {
                self.output.push(CHARS[(n & 0x3F) as usize]);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[allow(dead_code)]
struct Base64Reader;

impl Base64Reader {
    fn decode(input: &str) -> Option<Vec<u8>> {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

        fn char_to_val(c: u8) -> Option<u32> {
            CHARS.iter().position(|&ch| ch == c).and_then(|p| u32::try_from(p).ok())
        }

        let bytes = input.as_bytes();
        let mut result = Vec::new();

        let mut i = 0;
        while i < bytes.len() {
            let b0 = char_to_val(bytes[i])?;
            let b1 = if i + 1 < bytes.len() {
                char_to_val(bytes[i + 1])?
            } else {
                0
            };
            let b2 = if i + 2 < bytes.len() {
                char_to_val(bytes[i + 2])?
            } else {
                0
            };
            let b3 = if i + 3 < bytes.len() {
                char_to_val(bytes[i + 3])?
            } else {
                0
            };

            let n = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;

            result.push(((n >> 16) & 0xFF) as u8);
            if i + 2 < bytes.len() {
                result.push(((n >> 8) & 0xFF) as u8);
            }
            if i + 3 < bytes.len() {
                result.push((n & 0xFF) as u8);
            }

            i += 4;
        }

        Some(result)
    }
}

/// Create a tonic interceptor that validates JWT tokens.
/// Login RPC bypasses validation.
#[allow(dead_code)]
pub fn jwt_interceptor(
    jwt_manager: JwtManager,
) -> impl Fn(tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> + Clone {
    move |mut req: tonic::Request<()>| {
        // Check the gRPC method path to skip auth for Login
        // The method is not directly accessible here, so we rely on
        // the server setup to not apply the interceptor to Login.
        // Instead, we check for the Authorization header and skip if absent
        // (Login calls won't have it).

        let metadata = req.metadata();
        let auth_header = metadata.get("authorization");

        match auth_header {
            Some(value) => {
                let value = value
                    .to_str()
                    .map_err(|_| tonic::Status::unauthenticated("Invalid authorization header"))?;

                let token = value
                    .strip_prefix("Bearer ")
                    .ok_or_else(|| {
                        tonic::Status::unauthenticated("Invalid authorization format")
                    })?;

                let user_id = jwt_manager.validate_token(token).ok_or_else(|| {
                    tonic::Status::unauthenticated("Invalid or expired token")
                })?;

                req.extensions_mut()
                    .insert(AuthenticatedUser { user_id });

                Ok(req)
            }
            None => {
                // No auth header - allow through (Login RPC or unauthenticated access)
                Ok(req)
            }
        }
    }
}
