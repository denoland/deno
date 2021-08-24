# syntax=docker/dockerfile:1.3

ARG RUST_VERSION=1.54.0


FROM rust:${RUST_VERSION} AS deps

WORKDIR /src

RUN --mount=type=bind,target=.,rw \
  --mount=type=cache,target=/usr/local/cargo/registry \
  cargo fetch


FROM deps AS build

RUN --mount=type=bind,target=.,rw \
  --mount=type=cache,target=./target \
  --mount=type=cache,target=/usr/local/cargo/registry \
  cargo build --release \
  && mkdir -p /out && cp -Rf target /out/

WORKDIR /out


FROM scratch AS bin

COPY --from=build /out/target/release/deno /deno


FROM debian:latest AS debian

COPY --from=bin /deno /usr/local/bin/deno

CMD [ "deno", "run", "https://deno.land/std/examples/welcome.ts" ]


# debian is the default
FROM debian


FROM ubuntu:latest AS ubuntu

COPY --from=bin /deno /usr/local/bin/deno

CMD [ "deno", "run", "https://deno.land/std/examples/welcome.ts" ]


FROM alpine:latest AS alpine

COPY --from=bin /deno /usr/local/bin/deno

CMD [ "deno", "run", "https://deno.land/std/examples/welcome.ts" ]


FROM centos:latest AS centos

COPY --from=bin /deno /usr/local/bin/deno

CMD [ "deno", "run", "https://deno.land/std/examples/welcome.ts" ]
