####################################################################################################
## Builder
####################################################################################################
FROM ekidd/rust-musl-builder:latest AS builder

ADD --chown=rust:rust . ./

RUN echo $SVE_CREDENTIALS | base64 -d > /tmp/base64
RUN SVE_CREDENTIALS=$(cat /tmp/base64); echo "Output: $SVE_CREDENTIALS"

RUN echo $SVE_EMAILS | base64 -d > /tmp/base64
RUN SVE_EMAILS=$(cat /tmp/base64); echo "Output: $SVE_EMAILS"

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
