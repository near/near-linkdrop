#!/bin/bash
set -e

cargo near build reproducible-wasm
cp target/near/linkdrop.wasm ./res/

