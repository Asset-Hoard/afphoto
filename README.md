# afphoto

Read embedded preview images from [Affinity](https://affinity.serif.com/) files
, Photo (`.afphoto`), Designer (`.afdesign`) and Publisher (`.afpub`) , in pure,
dependency-free Rust.

## Why

Affinity's file format is proprietary, undocumented and zstd-compressed, so
fully parsing it is impractical. But the apps embed a flattened **PNG preview
uncompressed, in the clear**. This crate scans for that preview and hands it
back, so you can show a thumbnail without reverse engineering anything.

```rust
let bytes = std::fs::read("document.afphoto")?;
match afphoto::extract_preview(&bytes) {
    Ok(preview) => {
        println!("{}x{} preview", preview.width, preview.height);
        std::fs::write("preview.png", &preview.data)?;
    }
    Err(e) => eprintln!("no preview: {e}"),
}
```

There's also a cheap header check for content sniffing:

```rust
if afphoto::is_affinity(&bytes) { /* ... */ }
```

## What it does (v0.1)

- Detects the Affinity magic (`00 FF 4B 41`).
- Finds every embedded PNG stream and returns the **largest by pixel area**.
- Reports the preview's width and height (read from the PNG `IHDR`).
- Returns the raw PNG bytes , decode them with whatever image library you like.

It does **not** decode the image, and it does **not** parse the document object
graph (layers, colour mode, text, effects). That data lives in the compressed
format and is out of scope.

## Known limitations / help wanted

- **JPEG previews.** Affinity sometimes embeds a larger JPEG preview alongside
  the PNG. v0.1 only extracts PNG. JPEG support (scan `FF D8 FF`, read
  dimensions from the `SOFn` marker, end at `FF D9`) is a natural next step ,
  PRs welcome.
- **Format coverage.** Tested against Affinity Photo 2 (`.afphoto`). The magic
  and embedded-PNG approach should hold for `.afdesign`/`.afpub` and v1 files,
  but those aren't yet covered by fixtures.

## License

Licensed under [MIT license](LICENSE-MIT)
