fn main() {
    println!("cargo:rerun-if-changed=assets/icon/app.rc");
    println!("cargo:rerun-if-changed=assets/icon/night-island.ico");
    println!("cargo:rerun-if-env-changed=NI_BACKEND_HOST");
    let host = std::env::var("NI_BACKEND_HOST").unwrap_or_else(|_| "backend.invalid".into());
    println!("cargo:rustc-env=NI_BACKEND_HOST={host}");
    embed_resource::compile("assets/icon/app.rc", embed_resource::NONE).manifest_required().unwrap();
}
