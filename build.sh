#!/bin/bash
set -e

docker run -it --rm -v "$(pwd):/mnt" --workdir /mnt sourcescan/cargo-near:0.10.0-rust-1.81.0 bash -c '
  RUSTFLAGS="-C link-arg=-s" cargo build --target wasm32-unknown-unknown --release
  cp target/wasm32-unknown-unknown/release/linkdrop.wasm ./res/
'
