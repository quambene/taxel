
set dotenv-load

# Run desktop app
run:
    cd taxel-gui && cargo run

# Run desktop app with hot reloading
run-dev:
    cd taxel-gui && dx serve --hotpatch

run-dev-debug:
    RUST_LOG="taxel_gui=debug" cargo run -p taxel-gui

# Run tests
test:
    cargo test

# Run unit tests
test-unit:
    cargo test --lib
