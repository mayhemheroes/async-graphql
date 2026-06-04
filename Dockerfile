FROM --platform=linux/amd64 rustlang/rust:nightly-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    clang \
    llvm \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-fuzz

WORKDIR /build
COPY . .

# Build the fuzz target with cargo-fuzz (libfuzzer instrumentation, requires nightly)
WORKDIR /build/parser
RUN cargo fuzz build parse_query

FROM --platform=linux/amd64 debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libgcc-s1 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/parser/fuzz/target/x86_64-unknown-linux-gnu/release/parse_query /parse_query
