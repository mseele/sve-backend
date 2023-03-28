####################################################################################################
## Builder
####################################################################################################
FROM rust:1.68.2 AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev pkg-config libssl-dev ca-certificates llvm-dev libclang-dev clang
RUN update-ca-certificates

WORKDIR /app

ADD . ./

RUN cargo build --target x86_64-unknown-linux-musl --release

####################################################################################################
## Final image
####################################################################################################
FROM scratch

# Run the web service on container startup
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/sve_backend .
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
USER 1000
ENV RUST_BACKTRACE=1 RUST_LOG=info
CMD ["./sve_backend"]
