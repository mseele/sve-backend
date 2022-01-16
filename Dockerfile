####################################################################################################
## Builder
####################################################################################################
FROM rust:latest AS builder

# # Download the targets for static linking
# RUN rustup target add x86_64-unknown-linux-musl
# RUN apt update && apt install -y musl-tools musl-dev
# RUN update-ca-certificates

# WORKDIR /usr/src/app
# COPY . .

# # Install production dependencies and build a release artifact
# RUN cargo install --target x86_64-unknown-linux-musl --path .

COPY target/x86_64-unknown-linux-musl/release/sve_backend /usr/local/cargo/bin/sve_backend

####################################################################################################
## Final image
####################################################################################################
FROM scratch

# Run the web service on container startup
COPY --from=builder /usr/local/cargo/bin/sve_backend .
USER 1000
CMD ["./sve_backend"]
