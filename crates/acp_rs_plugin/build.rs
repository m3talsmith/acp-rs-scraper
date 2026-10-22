//! Records `rustc --version` for the plugin pin. Offline: no rustc_version crate.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let output = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .expect("rustc --version");
    if !output.status.success() {
        panic!("rustc --version failed");
    }
    let text = String::from_utf8(output.stdout).expect("rustc --version is utf-8");
    println!("cargo:rustc-env=ACP_RUSTC_VERSION={}", text.trim());
}
