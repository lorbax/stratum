#!/bin/sh

WORKSPACES="benches/Cargo.toml common/Cargo.toml protocols/Cargo.toml roles/Cargo.toml
utils/Cargo.toml"

for workspace in $WORKSPACES; do
    echo "Executing clippy on: $workspace"
    cargo clippy --manifest-path="$workspace" -- -D warnings -A dead-code
    if [ $? -ne 0 ]; then
        echo "Clippy found some errors in: $workspace"
        exit 1
    fi

    echo "Running tests on: $workspace"
    cargo test --manifest-path="$workspace"
    if [ $? -ne 0 ]; then
        echo "Tests failed in: $workspace"
        exit 1
    fi

    echo "Running fmt on: $workspace"
    cargo +nightly fmt --manifest-path="$workspace"
    if [ $? -ne 0 ]; then
        echo "Fmt failed in: $workspace"
        exit 1
    fi
done

echo "Clippy success, all tests passed!"

