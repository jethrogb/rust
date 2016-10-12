#!/bin/sh
RUST_TARGET_PATH=.. cargo rustc --target x86_64-unknown-none-gnu --release -- -C link-args="-nostdlib -Wl,-e,start"
