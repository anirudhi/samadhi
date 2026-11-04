//! Embeds the app icon and version details in the Windows program, so
//! `samadhi.exe` shows the Samādhi icon in Explorer, the taskbar and the
//! Start menu. Nothing happens for other systems.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon/samadhi.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon("assets/icon/samadhi.ico")
        .set("ProductName", "Samādhi")
        .set("FileDescription", "Samādhi")
        .set(
            "LegalCopyright",
            "© Anirudh Iyer and the Samādhi contributors (GPL-3.0)",
        );
    if let Err(err) = resource.compile() {
        // Building on Windows this must work; when only checking a Windows
        // build from elsewhere (no resource compiler), go on without it.
        let cross = std::env::var("HOST").ok() != std::env::var("TARGET").ok();
        assert!(cross, "embedding the Windows icon failed: {err}");
        println!("cargo:warning=not embedding the Windows icon: {err}");
    }
}
