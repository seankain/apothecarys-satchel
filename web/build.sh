#!/usr/bin/env bash
#
# Assembles the static site the GitLab Pages job publishes.
#
#   web/build.sh [output-directory]   # defaults to ./public
#
# There is no bundler and no wasm-bindgen: `crates/web-demo` exports a plain C
# ABI, so the build is one `cargo build` plus a handful of copies, and the
# result is the same whether it runs here or in CI.
#
# To look at it locally:
#
#   web/build.sh && python3 -m http.server --directory public 8000
#
# It has to be served over HTTP. Opening `public/index.html` as a `file://`
# URL fails: ES modules and `fetch` are both blocked on that origin.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:-$root/public}"
target=wasm32-unknown-unknown

if ! rustup target list --installed | grep -qx "$target"; then
  echo "web/build.sh: installing the $target target" >&2
  rustup target add "$target"
fi

cargo build \
  --manifest-path "$root/Cargo.toml" \
  --package apothecarys-web-demo \
  --release \
  --target "$target"

mkdir -p "$out"
cp "$root/web/index.html" "$root/web/style.css" "$root/web/main.js" "$root/web/renderer.js" "$out/"
cp "$root/target/$target/release/apothecarys_web_demo.wasm" "$out/plant.wasm"

# The page links `plantgl`, which is CeCILL-C, so the Article 6.4 notice of
# rights travels with the deploy for the same reason `crates/game/build.rs`
# puts it next to the executable: a licence step that only happens at release
# time is a licence step that gets forgotten.
cp "$root/THIRD-PARTY-LICENSES" "$root/LICENSE" "$out/"

printf 'web/build.sh: wrote %s (%s)\n' "$out" "$(du -sh "$out" | cut -f1)"
