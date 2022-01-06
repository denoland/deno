FROM mcr.microsoft.com/vscode/devcontainers/rust:0-1

# Update to Rust 1.56.1
RUN rustup update 1.56.1 && rustup default 1.56.1

# Install Deno
ENV DENO_INSTALL=/usr/local
RUN curl -fsSL https://deno.land/x/install/install.sh | sh
