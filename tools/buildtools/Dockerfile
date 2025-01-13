# Use Ubuntu 24.04 LTS as the base image
FROM ubuntu:24.04

WORKDIR /mnt
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    libssl-dev \
    pkg-config \
    wget \
    cmake \
    libglib2.0-dev \
    lsb-release \
    software-properties-common \
    gnupg \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Add the Cargo and Rust binaries to the PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Verify the installation
RUN rustc --version && cargo --version

RUN wget https://apt.llvm.org/llvm.sh && \
    chmod +x llvm.sh && \
    ./llvm.sh 17 && \
    apt-get install --install-recommends -y cmake libglib2.0-dev