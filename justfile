
set dotenv-load

# Run desktop app
run:
    cd taxel-gui && cargo run

# Run desktop app with hot reloading
run-dev:
    cd taxel-gui && dx serve --hotpatch

# Run tests
test:
    cargo test

# Run unit tests
test-unit:
    cargo test --lib
