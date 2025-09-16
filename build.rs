use std::process::Command;

fn main() -> std::io::Result<()> {
    Command::new("npx")
        .args([
            "tailwindcss",
            "-i",
            "./pub/tailwind.css",
            "-o",
            "./pub/index.css",
        ])
        .status()
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to build tailwind",
                ))
            }
        })?;

    Command::new("esbuild")
        .args([
            "--bundle",
            "index.js",
            "--outfile=dist/index.js",
            "--format=esm",
            "--sourcemap",
        ])
        .status()
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to npm build",
                ))
            }
        })?;

    Ok(())
}
