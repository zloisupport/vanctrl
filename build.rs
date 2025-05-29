extern crate winres;

fn main() {
    // only run if target os is windows
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() != "windows" {
        return;
    }
    let mut res = winres::WindowsResource::new();

    // only build the resource for release builds
    // as calling rc.exe might be slow
    if std::env::var("PROFILE").unwrap() == "release" {
        res.set_language(0x0409)
            .set_resource_file("app-manifest.rc")
            .set_manifest("app.exe.manifest.xml");
        if let Err(e) = res.compile() {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
    slint_build::compile("src/app_ui.slint").unwrap();
}
