//! Build the web UI into `web/ui/dist` before the binary embeds it.
//!
//! The build runs only when `web/ui/node_modules` exists, so CI and other machines
//! without Node still compile. Such a binary serves a "UI not built" error page.

use std::{path::Path, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=web/ui/src");
    println!("cargo:rerun-if-changed=web/ui/index.html");
    println!("cargo:rerun-if-changed=web/ui/package.json");
    println!("cargo:rerun-if-changed=web/ui/vite.config.ts");
    println!("cargo:rerun-if-changed=web/ui/node_modules");

    let ui = Path::new("web/ui");
    if !ui.join("node_modules").is_dir() {
        println!("cargo:warning=web/ui/node_modules is missing; run `npm install` in web/ui to embed the UI");
        return;
    }
    let status = Command::new("npm")
        .args(["run", "build"])
        .current_dir(ui)
        .status()
        .expect("failed to start `npm run build` in web/ui");
    assert!(
        status.success(),
        "`npm run build` in web/ui failed with {status}"
    );
}
