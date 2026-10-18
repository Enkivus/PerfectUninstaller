fn main() {
    tauri_build::build();

    // The compiled front end is embedded into the binary by `generate_context!`,
    // but the crate's own Rust sources have not changed when only `app/dist`
    // does — without this, `npm run build && cargo build` reports "Finished"
    // instantly and the app keeps serving the *previous* UI. Watching the
    // directory makes Cargo recompile whenever the embedded assets change.
    println!("cargo:rerun-if-changed=../dist");
}
