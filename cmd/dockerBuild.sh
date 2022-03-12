#!/usr/bin/bash

ARCHS=('i686.musl' 'x86_64.musl' 'arm.musleabi' 'armv7.musleabi' 'aarch64.musl')

for arch in "${ARCHS[@]}"
do
    left="${arch%.*}"
    right="${arch#*.}"
    echo
    echo "${arch}=${left}-${right}"
    image="messense/rust-musl-cross:${left}-${right}"
    echo "docker pull ${image}"
    arch="${left}-unknown-linux-${right}"
    echo "docker run --rm -it -v \"$(pwd)\":/home/rust/src ${image} cargo build --release"
    echo "zip -q -j thomas-linux-${left}.zip target/${arch}/release/server target/${arch}/release/client"
done
