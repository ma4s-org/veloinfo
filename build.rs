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
        .expect("Failed to build lib");
}
