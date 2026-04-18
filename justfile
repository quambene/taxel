
set dotenv-load

# Run desktop app
run:
    RUST_LOG="taxel_gui=info,taxel=info" cargo run -p taxel-gui

run-debug:
    RUST_LOG="taxel_gui=debug,taxel=debug" cargo run -p taxel-gui

# Run desktop app with hot reloading
run-dev:
    cd taxel-gui && RUST_LOG="taxel_gui=debug,taxel=debug" dx serve --hotpatch

run-dev-trace:
    cd taxel-gui && RUST_LOG="taxel_gui=trace,taxel=trace" dx serve --hotpatch

# Run tests
test:
    cargo test

# Run unit tests
test-unit:
    cargo test --lib
