#[cfg(windows)]
fn main() {
    use image::imageops::FilterType;
    use image::ImageEncoder;
    use std::env;
    use std::fs::File;
    use std::path::{Path, PathBuf};

    println!("cargo:rerun-if-changed=assets/logo.png");

    let logo_path = Path::new("assets/logo.png");
    if !logo_path.exists() {
        println!("cargo:warning=assets/logo.png was not found; skipping Windows executable icon");
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is not set"));
    let icon_path = out_dir.join("nexiscore_icon.ico");

    let icon = image::open(logo_path)
        .expect("failed to open assets/logo.png")
        .resize_to_fill(256, 256, FilterType::Lanczos3)
        .to_rgba8();

    let icon_file = File::create(&icon_path).expect("failed to create generated icon file");
    image::codecs::ico::IcoEncoder::new(icon_file)
        .write_image(&icon, icon.width(), icon.height(), image::ColorType::Rgba8)
        .expect("failed to encode assets/logo.png as an .ico file");

    let mut resource = winres::WindowsResource::new();
    resource.set_icon(
        icon_path
            .to_str()
            .expect("generated icon path is not valid UTF-8"),
    );
    resource.set("FileDescription", "NEXISCORE GUI");
    resource.set("ProductName", "NEXISCORE GUI");
    resource
        .compile()
        .expect("failed to embed Windows executable resources");
}

#[cfg(not(windows))]
fn main() {}
