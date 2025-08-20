#!/bin/bash
set -e

cargo near build reproducible-wasm
cp target/wasm32-unknown-unknown/release/linkdrop.wasm ./res/

