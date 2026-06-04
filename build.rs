use std::path::Path;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Only re-run if these files change
    println!("cargo:rerun-if-changed=proto/peerman.proto");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=frontend/src");
    println!("cargo:rerun-if-changed=frontend/package.json");
    println!("cargo:rerun-if-changed=frontend/vite.config.ts");

    // 1. Compile proto → Rust (tonic + prost)
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/peerman.proto"], &["proto"])?;

    // 2. Build frontend (skip if no package.json or if SKIP_FRONTEND_BUILD is set)
    if std::env::var("SKIP_FRONTEND_BUILD").is_ok() {
        println!("cargo:warning=SKIP_FRONTEND_BUILD set, skipping frontend build");
        return Ok(());
    }

    let frontend_dir = Path::new("frontend");
    if frontend_dir.join("package.json").exists() {
        println!("cargo:warning=Building frontend with pnpm...");
        // Install dependencies
        let install = Command::new("pnpm")
            .args(["install", "--frozen-lockfile"])
            .current_dir(frontend_dir)
            .status();
        if install.is_err() {
            println!(
                "cargo:warning=pnpm not available, skipping frontend build (use pre-built dist/)"
            );
            return Ok(());
        }

        // Build
        let status = Command::new("pnpm")
            .args(["run", "build"])
            .current_dir(frontend_dir)
            .status();

        match status {
            Ok(s) if s.success() => println!("cargo:warning=Frontend build succeeded"),
            Ok(s) => panic!("Frontend build failed with exit code: {:?}", s.code()),
            Err(_) => panic!("pnpm build command failed"),
        }
    }

    Ok(())
}
