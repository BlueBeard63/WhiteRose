//! `cargo run --example icon_probe -- NAME...`: what the icon lookup finds
//! on this machine (file names and icon names).
fn main() {
    let icons = whiterose_kit::icons::shared();
    for a in std::env::args().skip(1) {
        let as_file = icons.for_file(&a, a.ends_with('/'), 48, 1);
        let mime = icons.mime.type_of(&a).to_string();
        println!("{a:28} {mime:36} {}", as_file.map(|p| p.display().to_string()).unwrap_or("—".into()));
    }
}
