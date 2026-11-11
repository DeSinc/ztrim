fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon.ico");

    #[cfg(windows)]
    {
        if std::path::Path::new("assets/icon.ico").exists() {
            let mut res = winres::WindowsResource::new();
            res.set_icon("assets/icon.ico");
            let _ = res.compile();
        }
    }
}

