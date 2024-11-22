use std::process::Command;

fn main() {
    Command::new("npx")
        .args([
            "tailwindcss",
            "-i",
            "./pub/tailwind.css",
            "-o",
            "./pub/index.css",
        ])
        .status()
        .expect("Failed to build tailwind");

    Command::new("esbuild")
        .args([
            "--bundle",
            "index.js",
            "--outfile=dist/index.js",
            "--format=esm",
        ])
        .status()
        .expect("Failed to npm build");
}
