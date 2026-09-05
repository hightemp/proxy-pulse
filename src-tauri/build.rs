fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../VERSION");
    let version = std::fs::read_to_string("../VERSION")?;
    if version.trim() != std::env::var("CARGO_PKG_VERSION")? {
        return Err(
            "VERSION differs from Cargo metadata. Run make version before building.".into(),
        );
    }
    tauri_build::build();
    Ok(())
}
