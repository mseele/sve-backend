#!/bin/zsh

# cross compile for docker image
export CC_x86_64_unknown_linux_musl=x86_64-unknown-linux-musl-gcc
export CXX_x86_64_unknown_linux_musl=x86_64-unknown-linux-musl-g++
export AR_x86_64_unknown_linux_musl=x86_64-unknown-linux-musl-ar
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-unknown-linux-musl-gcc
cargo build --release --target=x86_64-unknown-linux-musl

# create docker image
docker build --pull --rm -f "Dockerfile" -t sve_backend:latest "."

# upload into google app engine repository
docker tag sve_backend:latest eu.gcr.io/$gae-project-name$/sve_backend
docker push eu.gcr.io/$gae-project-name$/sve_backend
