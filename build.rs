fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set("FileDescription", "Ruststrap");
        res.set("ProductName", "Ruststrap");
        res.set("LegalCopyright", "MIT License");
        if std::path::Path::new("ruststrap.ico").exists() {
            res.set_icon("ruststrap.ico");
        }
        let _ = res.compile();
    }
}
