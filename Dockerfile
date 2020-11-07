FROM ekidd/rust-musl-builder:stable

WORKDIR /home/rust/

ADD . .
RUN sudo chown -R rust:rust ./
RUN cargo build --release --target=x86_64-unknown-linux-musl

Run cp /home/rust/target/x86_64-unknown-linux-musl/release/server /home/rust/server