set dotenv-load

# Run desktop app
run:
    RUST_LOG="taxel_gui=info,taxel=info" cargo run -p taxel-gui

# Run desktop app in debug mode
run-debug:
    RUST_LOG="taxel_gui=debug,taxel=debug" cargo run -p taxel-gui

# Run desktop app in release mode
run-release:
    RUST_LOG="taxel_gui=info,taxel=info" cargo run -p taxel-gui --release

# Run desktop app with hot reloading
run-dev:
    cd taxel-gui && RUST_LOG="taxel_gui=debug,taxel=debug" dx serve --hotpatch

# Run desktop app with hot reloading and trace logging
run-dev-trace:
    cd taxel-gui && RUST_LOG="taxel_gui=trace,taxel=trace" dx serve --hotpatch

# Run tests
test:
    cargo test

# Run unit tests
test-unit:
    cargo test --lib

# Run cargo check with env vars
check:
    cargo check

# Run cargo clippy with env vars
clippy:
    cargo clippy --all-targets --all-features

# Run cargo build with env vars
build:
    cargo build

# Run cargo build with env vars in release mode
build-release:
    cargo build --release
