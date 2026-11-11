fn main() {
    println!("cargo:rerun-if-changed=assets/app_icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() == Some("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/app_icon.ico")
            .set("ProductName", "Background Remover AI")
            .set("FileDescription", "AI Image Background Remover")
            .set("InternalName", "rustbackremover.exe")
            .set("OriginalFilename", "rustbackremover.exe")
            .set("ProductVersion", "0.2.1")
            .set("FileVersion", "0.2.1");
        resource
            .compile()
            .expect("failed to embed Windows application icon");
    }
}
