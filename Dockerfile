####################################################################################################
## Builder
####################################################################################################
FROM ekidd/rust-musl-builder:latest AS builder

ADD --chown=rust:rust . ./

RUN echo "$SVE_CREDENTIALS_ENCODED" | base64 -d > /tmp/base64
RUN SVE_CREDENTIALS_DECODED=$(cat /tmp/base64); echo "$SVE_CREDENTIALS_DECODED Output: $SVE_CREDENTIALS_DECODED"
ENV SVE_CREDENTIALS=$SVE_CREDENTIALS_ENCODED
RUN echo $SVE_CREDENTIALS

RUN echo "$SVE_EMAILS_ENCODED" | base64 -d > /tmp/base64
RUN SVE_EMAILS_DECODED=$(cat /tmp/base64); echo "$SVE_EMAILS_ENCODED Output: $SVE_EMAILS_DECODED"
ENV SVE_EMAILS=$SVE_EMAILS_DECODED
RUN echo $SVE_EMAILS

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
