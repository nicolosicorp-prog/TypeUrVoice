fn main() {
    // Used by local_llm::server to find the un-bundled llama-server sidecar
    // (binaries/llama-server-<triple>) in debug builds.
    println!(
        "cargo:rustc-env=TYPEURVOICE_TARGET_TRIPLE={}",
        std::env::var("TARGET").unwrap()
    );
    tauri_build::build()
}
