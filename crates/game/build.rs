//! Ships the licensing artefacts next to the game binary.
//!
//! `crates/plantgl` is CeCILL-C and the rest of the workspace is MIT, so the
//! shipped binary is Derivative Software under CeCILL-C Article 5.3.3. That is
//! allowed under another licence **provided the Article 6.4 notice of rights
//! travels with it** — which means `THIRD-PARTY-LICENSES` has to be beside the
//! executable, not merely in the repository.
//!
//! Copying it here rather than in a packaging script is deliberate: a step
//! that only runs at release time is a step that is discovered to be missing
//! at release time. `tests/licensing.rs` asserts the result, so an ordinary
//! `cargo test` catches it.

use std::path::{Path, PathBuf};
use std::{env, fs};

/// The files that have to sit beside the binary.
const ARTEFACTS: [&str; 2] = ["THIRD-PARTY-LICENSES", "LICENSE"];

fn main() {
    let manifest = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
    );
    let workspace = manifest
        .parent()
        .and_then(Path::parent)
        .expect("crates/game sits two levels below the workspace root")
        .to_path_buf();

    // OUT_DIR is `<target>/<profile>/build/<pkg>-<hash>/out`; the binary lands
    // three levels up, in `<target>/<profile>`.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let dist = out_dir
        .ancestors()
        .nth(3)
        .expect("OUT_DIR is nested inside the profile directory")
        .to_path_buf();

    for name in ARTEFACTS {
        let source = workspace.join(name);
        println!("cargo:rerun-if-changed={}", source.display());
        match fs::copy(&source, dist.join(name)) {
            Ok(_) => {}
            // A missing or unwritable artefact must not break an unrelated
            // build; `tests/licensing.rs` is what turns it into a failure, and
            // it says something more useful than a copy error would.
            Err(e) => println!(
                "cargo:warning=could not ship {}: {e}",
                source.display()
            ),
        }
    }

    // So the test can find the directory without guessing at the profile or at
    // CARGO_TARGET_DIR.
    println!("cargo:rustc-env=APOTHECARYS_DIST_DIR={}", dist.display());
}
