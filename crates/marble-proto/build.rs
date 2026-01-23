fn main() -> Result<(), Box<dyn std::error::Error>> {
    let build_server = std::env::var("CARGO_FEATURE_SERVER").is_ok();

    tonic_prost_build::configure()
        .build_server(build_server)
        .build_client(build_server)
        .compile_protos(
            &["../../proto/room.proto"],
            &["../../proto"],
        )?;
    Ok(())
}
