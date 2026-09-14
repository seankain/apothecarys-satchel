//! That the licensing artefacts actually ship with the built binary.
//!
//! `crates/plantgl` is a translation of PlantGL and is CeCILL-C; every other
//! crate, this one included, is MIT. CeCILL-C Article 5.3.3 permits that
//! — Derivative Software may carry another licence — **provided the Article
//! 6.4 notice of rights is carried with it**. `build.rs` copies the notice
//! next to the executable; this checks that it arrived, and that it still says
//! what Article 6.4 requires it to say.
//!
//! `crates/plantgl/tests/licensing.rs` checks the same file in the repository.
//! This one checks the build output, which is what is actually distributed —
//! and it runs under `--release` as readily as under `--debug`, because it
//! reads the directory `build.rs` resolved rather than guessing at a profile.

use std::path::{Path, PathBuf};

/// Where `build.rs` put the artefacts — `<target>/<profile>`.
fn dist_dir() -> PathBuf {
    PathBuf::from(env!("APOTHECARYS_DIST_DIR"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "{}: {e}\nThis file is a CeCILL-C Article 6.4 obligation and must \
             ship beside the binary; crates/game/build.rs copies it there.",
            path.display()
        )
    })
}

#[test]
fn the_third_party_notice_ships_beside_the_binary() {
    let notice = read(&dist_dir().join("THIRD-PARTY-LICENSES"));

    for required in [
        "Copyright CIRAD/INRIA/INRA",
        "CeCILL-C FREE SOFTWARE LICENSE AGREEMENT",
        "https://github.com/openalea/plantgl",
        "limited warranty",
        "limited\n  liability",
        "Pradal C., Boudon F., Nouguier C., Chopard J., Godin C. 2009.",
    ] {
        assert!(
            notice.contains(required),
            "the shipped THIRD-PARTY-LICENSES is missing {required:?}"
        );
    }
}

#[test]
fn the_shipped_notice_matches_the_one_in_the_repository() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/game sits two levels below the workspace root")
        .to_path_buf();

    for name in ["THIRD-PARTY-LICENSES", "LICENSE"] {
        let shipped = read(&dist_dir().join(name));
        let source = read(&workspace.join(name));
        assert_eq!(
            shipped, source,
            "the shipped {name} has drifted from the one in the repository"
        );
    }
}

/// The notice has to point at the port's source, because Article 5.3.3 asks
/// for it to stay available for as long as the larger work is distributed.
#[test]
fn the_notice_points_at_the_ported_source() {
    let notice = read(&dist_dir().join("THIRD-PARTY-LICENSES"));
    assert!(
        notice.contains("crates/plantgl"),
        "the notice does not say where the ported source is"
    );
}
