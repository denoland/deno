# Based on https://github.com/LukeChannings/deno-arm64
FROM ubuntu:18.04

SHELL ["/bin/bash", "-c"]

RUN apt-get update -y
RUN DEBIAN_FRONTEND="noninteractive" TZ="Europe/London" apt-get install -y python curl build-essential unzip git libtool autoconf cmake
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

WORKDIR /
RUN curl -OL https://github.com/protocolbuffers/protobuf/releases/download/v25.0/protoc-25.0-linux-aarch_64.zip
RUN unzip protoc-25.0-linux-aarch_64.zip

RUN git config --global core.symlinks true
ADD . /deno

WORKDIR /deno

ENV PATH="/root/.cargo/bin:${PATH}"

RUN rustup target add wasm32-unknown-unknown
RUN rustup target add wasm32-wasi

RUN RUST_BACKTRACE=full cargo build --locked --bin --release deno
