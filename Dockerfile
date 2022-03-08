####################################################################################################
## Builder
####################################################################################################
FROM rust:latest AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev build-essential gcc-x86-64-linux-gnu pkg-config libssl-dev
RUN update-ca-certificates

WORKDIR /usr/src/app
COPY . .

ENV RUSTFLAGS='-C linker=x86_64-linux-musl-gcc'
ENV TARGET_CC=x86_64-linux-musl-gcc

# Install production dependencies and build a release artifact
RUN cargo install --target x86_64-unknown-linux-musl

####################################################################################################
## Final image
####################################################################################################
FROM scratch

# Run the web service on container startup
COPY --from=builder /usr/local/cargo/bin/sve_backend .
USER 1000
ENV RUST_BACKTRACE=1 RUST_LOG=info
CMD ["./sve_backend"]
