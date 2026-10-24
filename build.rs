fn main() {
    println!("cargo:rerun-if-changed=assets/icon/app.rc");
    println!("cargo:rerun-if-changed=assets/icon/night-island.ico");
    embed_resource::compile("assets/icon/app.rc", embed_resource::NONE).manifest_required().unwrap();
}
