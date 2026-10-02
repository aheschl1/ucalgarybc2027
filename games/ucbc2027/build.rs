//! Generates the map file's Rust types from `map.proto`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=map.proto");
    let files = protox::compile(["map.proto"], ["."])?;
    prost_build::Config::new().compile_fds(files)?;
    Ok(())
}
