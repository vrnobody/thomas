#!/usr/bin/bash

ARCHS=('i686.musl' 'x86_64.musl' 'armv5te.musleabi' 'arm.musleabi' 'armv7.musleabihf' 'mips.musl' 'mipsel.musl')

for arch in "${ARCHS[@]}"
do
    left="${arch%.*}"
    right="${arch#*.}"
    echo "${arch}=${left}.${right}"
    image="getsentry/rust-musl-cross:${left}-${right}"
    echo "docker pull ${image}"
    arch="${left}-unknown-linux-${right}"
    echo "docker run --rm -it -v \"$(pwd)\":/home/rust/src getsentry/rust-musl-cross:${image} cargo build --release"
    echo "zip -q -j thomas-linux-${left}.zip target/${arch}/release/server target/${arch}/release/client"
done