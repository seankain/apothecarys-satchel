fn main() {
    // Intentionally empty: we satisfy the `links = "alsa"` key without
    // actually linking any native library.  The Rust-level stub functions
    // in src/lib.rs provide all the symbols that downstream crates
    // (tinyaudio) expect.
}
