
set dotenv-load

# Run desktop app
run:
    cargo run -p taxel-gui

run-debug:
    RUST_LOG="taxel_gui=debug" cargo run -p taxel-gui

# Run desktop app with hot reloading
run-dev:
    cd taxel-gui && dx serve --hotpatch

run-dev-debug:
    cd taxel-gui && RUST_LOG="taxel_gui=debug" dx serve --hotpatch

# Run tests
test:
    cargo test

# Run unit tests
test-unit:
    cargo test --lib
