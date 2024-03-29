#!/usr/bin/env bash

cargo build --bin client
cargo build --bin server

# release
# cargo build --release --bin client
# cargo build --release --bin server