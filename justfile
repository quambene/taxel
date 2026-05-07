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

# Run unit tests
test-unit:
    cargo test --lib

# Run integration tests (requires ERiC library)
test-integration:
    cargo test --test '*' -- --test-threads=1

# Run integration tests in release mode (requires ERiC library)
test-integration-release:
    cargo test --release --test '*' -- --test-threads=1

# Run external tests (requires ERiC library and Elster certificate)
test-external:
    cargo test --test '*' --features external-test -- --test-threads=1

# Run external tests in release mode (requires ERiC library and Elster certificate)
test-external-release:
    cargo test --release --test '*' --features external-test -- --test-threads=1

# Run Python unit tests
test-py:
    cd taxel-py && pytest -v -m unit

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
