FROM rust:1-slim AS builder

WORKDIR /app
COPY . .
RUN cargo build --release -p zn-cli && \
    cp target/release/zero-nine /usr/local/bin/zero-nine

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates git && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/zero-nine /usr/local/bin/zero-nine

ENTRYPOINT ["zero-nine"]
CMD ["--help"]
