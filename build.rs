use std::io;

fn main() -> io::Result<()> {
    // Configura explicitamente o compilador
    prost_build::Config::new()
        .compile_protos(&["dados.proto"], &["."])?;
    Ok(())
}
