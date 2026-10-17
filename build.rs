//! On Windows, embeds the icon and version information in rsampler.exe, so Explorer shows them.

fn main() {
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/rsampler.ico").set("ProductName", "rsampler").set("FileDescription", "rsampler");
        res.compile().expect("Windows resources");
    }
}
