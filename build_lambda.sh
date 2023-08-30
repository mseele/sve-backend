#!/bin/bash

archive_name=sve_backend_lambda
file_name=bootstrap

rm -f $archive_name.zip

cargo build --target=aarch64-unknown-linux-musl --release

cp target/aarch64-unknown-linux-musl/release/sve_backend $file_name

zip -r $archive_name.zip $file_name

rm $file_name
