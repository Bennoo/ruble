FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive
# enable 'universe' because musl-tools & clang live there
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    software-properties-common && \
    add-apt-repository --yes universe

# now install build deps
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    build-essential curl git ca-certificates \
    pkg-config clang musl-tools libssl-dev just && \
    rm -rf /var/lib/apt/lists/*

# Install nano editor
RUN apt-get update && apt-get install -y nano

# install Node.js and npm
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash -
RUN apt-get install -y nodejs

# install Rust + musl
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && \
    ~/.cargo/bin/rustup target add aarch64-unknown-linux-musl && \
    ~/.cargo/bin/rustup component add clippy rustfmt

ENV PATH="/root/.cargo/bin:${PATH}"

# Install the CODEX CLI
RUN npm install -g @openai/codex
RUN mkdir -p /root/.codex
COPY .devcontainer/codex_config.toml /root/.codex/config.toml
COPY .devcontainer/auth.json /root/.codex/auth.json

WORKDIR /workspace
