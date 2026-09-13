//! Guards on the CeCILL-C obligations.
//!
//! The port's licensing duties are cheap to discharge now and expensive to
//! retrofit, and the notice file is the part that is easy to forget — so they
//! are asserted here rather than left to a release checklist.
//!
//! Part of the `plantgl` crate and therefore licensed CeCILL-C; see
//! crates/plantgl/LICENSE.

use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(Path::parent)
        .expect("crates/plantgl sits two levels below the workspace root")
        .to_path_buf()
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_crate_ships_the_cecill_c_agreement() {
    let license = read(crate_root().join("LICENSE"));
    assert!(license.starts_with("CeCILL-C FREE SOFTWARE LICENSE AGREEMENT"));
    assert!(license.contains("Version 1.0 dated 2006-09-05."));
    // The two articles the port's obligations rest on.
    assert!(license.contains("5.3.2 DISTRIBUTION OF MODIFIED SOFTWARE"));
    assert!(license.contains("5.3.3 DISTRIBUTION OF DERIVATIVE SOFTWARE"));
    assert!(license.contains("6.4 NOTICE OF RIGHTS"));
}

#[test]
fn the_manifest_declares_the_spdx_identifier() {
    let manifest = read(crate_root().join("Cargo.toml"));
    assert!(
        manifest.contains(r#"license = "CECILL-C""#),
        "crates/plantgl must declare license = \"CECILL-C\""
    );
}

#[test]
fn the_workspace_carries_the_article_6_4_notice() {
    let notice = read(workspace_root().join("THIRD-PARTY-LICENSES"));
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
            "THIRD-PARTY-LICENSES is missing {required:?}"
        );
    }
}

#[test]
fn the_root_readme_explains_the_mixed_licensing() {
    let readme = read(workspace_root().join("README.md"));
    assert!(readme.contains("CeCILL-C"));
    assert!(readme.contains("crates/plantgl"));
    assert!(readme.contains("THIRD-PARTY-LICENSES"));
}

/// Article 5.3.3 asks that Integrated Contributions be clearly identified and
/// documented. Every source file in the crate carries a header naming the
/// upstream file it derives from, or saying plainly that it derives from none.
#[test]
fn every_source_file_carries_a_provenance_header() {
    let mut checked = 0;
    let mut missing = Vec::new();

    for path in rust_files(&crate_root().join("src")) {
        let head = read(path.clone())
            .lines()
            .take(24)
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();
        let has_provenance = ["ported from plantgl", "corresponds to plantgl", "replaces plantgl",
            "original to this port", "ported from [openalea/plantgl]"]
            .iter()
            .any(|phrase| head.contains(phrase));
        let names_the_license = head.contains("cecill-c");
        if !has_provenance || !names_the_license {
            missing.push(path);
        }
        checked += 1;
    }

    assert!(checked > 0, "found no source files to check");
    assert!(
        missing.is_empty(),
        "missing a provenance header naming the upstream file and the license: {missing:#?}"
    );
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            found.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
    found.sort();
    found
}
