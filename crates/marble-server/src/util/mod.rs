pub fn clamp<T: Ord>(value: T, min: T, max: T) -> T {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

macro_rules! inner_tonic_uuid {
    ($s:expr) => {
        uuid::Uuid::parse_str($s)
            .map_err(|cause| tonic::Status::invalid_argument(format!("Invalid UUID: {}", cause)))
    };
    ($s:expr, $msg:expr) => {
        uuid::Uuid::parse_str($s)
            .map_err(|cause| tonic::Status::invalid_argument(format!($msg, cause)))
    };
}
pub(crate) use inner_tonic_uuid as tonic_uuid;

pub fn required_str(s: &str, msg: &'static str) -> Result<(), tonic::Status> {
    if s.is_empty() {
        Err(tonic::Status::invalid_argument(msg))
    } else {
        Ok(())
    }
}

macro_rules! inner_tonic_required {
    ($opt:expr) => {
        match $opt {
            std::option::Option::Some(value) => std::result::Result::Ok(value),
            std::option::Option::None => std::result::Result::Err(tonic::Status::invalid_argument(
                format!("Required field '{}' missing ", stringify!($opt)),
            )),
        }
    };
    ($opt:expr, $msg:expr) => {
        match $opt {
            std::option::Option::Some(value) => std::result::Result::Ok(value),
            std::option::Option::None => {
                std::result::Result::Err(tonic::Status::invalid_argument($msg))
            }
        }
    };
}
pub(crate) use inner_tonic_required as tonic_required;
