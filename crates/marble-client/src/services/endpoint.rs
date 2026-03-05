/// gRPC base URL. 컴파일 타임 `GRPC_URL` 환경변수로 절대 URL 설정 가능.
/// 미설정 시 현재 origin 사용 (prod).
pub fn grpc_base_url() -> String {
    option_env!("GRPC_URL")
        .map(String::from)
        .unwrap_or_else(|| {
            web_sys::window().unwrap().location().origin().unwrap()
        })
}

/// Signaling base URL. 컴파일 타임 `SIGNALING_URL` 환경변수로 절대 URL 설정 가능.
/// 미설정 시 현재 origin의 ws 경로 사용 (prod).
pub fn signaling_base_url() -> String {
    option_env!("SIGNALING_URL")
        .map(String::from)
        .unwrap_or_else(|| {
            let origin = web_sys::window().unwrap().location().origin().unwrap();
            origin.replacen("http", "ws", 1) + "/signaling"
        })
}
