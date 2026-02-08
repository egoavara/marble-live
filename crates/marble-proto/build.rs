fn main() -> Result<(), Box<dyn std::error::Error>> {
    let build_server = std::env::var("CARGO_FEATURE_SERVER").is_ok();
    let build_client = std::env::var("CARGO_FEATURE_CLIENT").is_ok();

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);

    tonic_prost_build::configure()
        .build_server(build_server)
        .build_client(build_client)
        .build_transport(build_server)
        .file_descriptor_set_path(out_dir.join("marble_descriptor.bin"))
        .compile_protos(
            &[
                "../../proto/user.proto",
                "../../proto/map.proto",
                "../../proto/room.proto",
                "../../proto/play.proto",
            ],
            &["../../proto"],
        )?;
    Ok(())
}
