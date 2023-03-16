FROM nearprotocol/bridge as bridge
FROM ubuntu:20.04 as wasm

ENV DEBIAN_FRONTEND noninteractive

RUN apt-get update -qq && apt-get install -y \
    git \
    cmake \
    g++ \
    pkg-config \
    libssl-dev \
    curl \
    llvm \
    clang \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl https://sh.rustup.rs -sSf | \
    sh -s -- -y --no-modify-path

RUN rustup target add wasm32-unknown-unknown
WORKDIR /root
COPY . .
RUN ./build.sh

FROM node:12
ENV NEAR_ENV local
RUN npm install -g near-cli
RUN mkdir ~/.near
COPY --from=bridge /root/.near/localnet/node0/validator_key.json ~/.near
COPY --from=wasm /root/res/linkdrop.wasm .
CMD ["near deploy --accountId node0 --wasmFile linkdrop.wasm --keyPath validator_key.json --nodeUrl $NODE_URL"]
