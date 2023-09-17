FROM mcr.microsoft.com/vscode/devcontainers/rust:1-bullseye

# Install cmake and protobuf-compiler
RUN apt-get update \
  && apt-get install -y cmake \
  && apt-get install -y protobuf-compiler \
  && rm -rf /var/lib/apt/lists/*

# Install Deno
ENV DENO_INSTALL=/usr/local
RUN curl -fsSL https://deno.land/x/install/install.sh | sh

