####################################################################################################
## Builder
####################################################################################################
FROM rust:latest AS builder

RUN update-ca-certificates

WORKDIR /app

ADD . ./

RUN echo "$SVE_CREDENTIALS_ENCODED" | base64 -d > /tmp/base64
RUN SVE_CREDENTIALS_DECODED=$(cat /tmp/base64)
ENV SVE_CREDENTIALS=$SVE_CREDENTIALS_ENCODED

RUN echo "$SVE_EMAILS_ENCODED" | base64 -d > /tmp/base64
RUN SVE_EMAILS_DECODED=$(cat /tmp/base64)
ENV SVE_EMAILS=$SVE_EMAILS_DECODED

RUN cargo build --release

####################################################################################################
## Final image
####################################################################################################
FROM rust:latest

# Run the web service on container startup
COPY --from=builder /app/target/release/sve_backend .
USER 1000
ENV RUST_BACKTRACE=1 RUST_LOG=info
CMD ["./sve_backend"]
