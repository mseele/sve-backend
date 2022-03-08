####################################################################################################
## Builder
####################################################################################################
FROM ekidd/rust-musl-builder:latest AS builder

ADD --chown=rust:rust . ./

RUN cargo build --release

####################################################################################################
## Final image
####################################################################################################
FROM scratch

# Run the web service on container startup
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/sve_backend /usr/local/bin/
USER 1000
ENV RUST_BACKTRACE=1 RUST_LOG=info
CMD /usr/local/bin/sve_backend
