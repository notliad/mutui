fn main() {
    // On macOS with Homebrew, libmpv is installed to a non-standard prefix that the
    // linker won't search by default. Run `brew --prefix mpv` to find it at build time.
    if cfg!(target_os = "macos") {
        if let Ok(output) = std::process::Command::new("brew")
            .args(["--prefix", "mpv"])
            .output()
        {
            if output.status.success() {
                let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
                println!("cargo:rustc-link-search=native={prefix}/lib");
            }
        }
    }
}
