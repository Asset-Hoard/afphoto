//! Extract the embedded preview from an Affinity file and write it as a PNG.
//!
//! ```text
//! cargo run --example dump_preview -- document.afphoto preview.png
//! ```

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("usage: dump_preview <input> [output.png]");
    let output = args.next().unwrap_or_else(|| "preview.png".to_string());

    let bytes = std::fs::read(&input).expect("failed to read input file");
    match afphoto::extract_preview(&bytes) {
        Ok(preview) => {
            std::fs::write(&output, &preview.data).expect("failed to write preview");
            println!(
                "{} -> {} ({}x{}, {} bytes)",
                input,
                output,
                preview.width,
                preview.height,
                preview.data.len()
            );
        }
        Err(e) => {
            eprintln!("{input}: {e}");
            std::process::exit(1);
        }
    }
}
