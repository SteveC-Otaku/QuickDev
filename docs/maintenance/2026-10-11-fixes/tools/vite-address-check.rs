#[path = "../../../../src-tauri/src/config/vite.rs"]
mod vite;
fn main() {
    let Some(root) = std::env::args().nth(1) else {
        eprintln!("Usage: vite-address-check <directory-containing-vite-config>");
        std::process::exit(2);
    };
    match vite::read_address(std::path::Path::new(&root)) {
        Ok((port,url)) => println!("port={port} url={url}"),
        Err(error) => { eprintln!("{error}"); std::process::exit(1); }
    }
}