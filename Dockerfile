FROM rust:1-slim-trixie

RUN rustup component add rustfmt clippy \
    && rustup target add x86_64-unknown-linux-musl

WORKDIR /app
