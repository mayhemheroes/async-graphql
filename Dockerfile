FROM rust as builder
RUN rustup toolchain add nightly
RUN rustup default nightly
RUN cargo +nightly install -f cargo-fuzz

ADD . /async-graphql
WORKDIR /async-graphql/parser/fuzz

RUN cargo fuzz build parse_query

# Package Stage
FROM ubuntu:20.04

COPY --from=builder /async-graphql/parser/fuzz/target/x86_64-unknown-linux-gnu/release/parse_query /
