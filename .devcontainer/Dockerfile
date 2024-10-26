FROM mcr.microsoft.com/vscode/devcontainers/rust:1-bullseye

# Install cmake
RUN apt-get update \
  && apt-get install -y cmake \
  && rm -rf /var/lib/apt/lists/*

# Install Deno
ENV DENO_INSTALL=/usr/local
RUN curl -fsSL https://deno.land/x/install/install.sh | sh

