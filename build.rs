fn main() {
    // Windows default stack is 1 MB, which overflows in debug builds
    // due to the large Commands enum (30+ Clap variants) and match routing.
    // Set 8 MB stack to match Unix defaults.
    #[cfg(target_os = "windows")]
    {
        let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
        if target_env == "gnu" {
            println!("cargo:rustc-link-arg=-Wl,--stack,8388608");
        } else {
            println!("cargo:rustc-link-arg=/STACK:8388608");
        }
    }
}
