# Taxel

Taxel provides Rust bindings for the ELSTER Richt Client (ERiC).

- [What is ELSTER?](#what-is-elster)
- [What is ERiC?](#what-is-eric)
- [Requirements](#requirements)
- [Install Taxel from Github.com](#install-taxel)
- [Set library name](#set-library-name)
- [Generate bindings](#generate-bindings)
- [Test bindings](#test-bindings)

## What is ELSTER?

Elster (short for _Elektronische Steuererklärung_) is a project by the German tax administrations to process tax returns and declarations.

## What is ERiC?

ERiC is a C library that is integrated into a tax application. ERiC checks the data supplied by the tax application for plausibility and transmits the data electronically to the computing center of the respective tax administration.

## Requirements

You need to have the shared library `libericapi.so` and the header file `ericapi.h` available on your system which can be downloaded from [ELSTER for developers](https://www.elster.de/elsterweb/entwickler/login) after access has been requested [here](https://www.elster.de/elsterweb/registrierung-entwickler/form).

For generating the bindings for your platform and architecture, you need `libclang` as well. For example, on Debian/Ubuntu install:

``` bash
apt install llvm-dev libclang-dev clang
```

## Install Taxel from [Github.com](https://github.com/quambene/taxel)

``` bash
git clone git@github.com:quambene/taxel.git
cd taxel
```

## Set library name

1. Create an environment file:

    ``` bash
    touch .env
    ```

2. Set environment variables `LIBRARY_PATH`, `LIBRARY_NAME`, `HEADER_FILE`, and `PLUGIN_PATH` in your `.env`. For example:

    ``` bash
    LIBRARY_PATH=ERiC-36.1.8.0-Linux-x86_64/ERiC-36.1.8.0/Linux-x86_64/lib
    LIBRARY_NAME=ericapi
    HEADER_FILE=ERiC-36.1.8.0-Linux-x86_64/ERiC-36.1.8.0/Linux-x86_64/include/ericapi.h
    PLUGIN_PATH=ERiC-36.1.8.0-Linux-x86_64/ERiC-36.1.8.0/Linux-x86_64/lib/plugins2
    ```

3. Source your environment:

    ``` bash
    set -a && source .env && set +a
    ```

_Remark_: In step 2, note the difference between file name (e.g. `libericapi.so` on Linux) and `LIBRARY_NAME` (which is `ericapi`).

## Generate bindings

The bindings have to be generated on-the-fly for your specific platform and architecture:

``` bash
cargo build
```

The bindings are generated in `target/debug/build/taxel-<random-id>/out/bindings.rs`.

## Test bindings

The bindings are included in `src/lib.rs` via `include!` macro and tested by:

``` bash
cargo test
```

Logs are written to `eric.log`.
