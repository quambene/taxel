# Taxel

Taxel provides a command line interface (CLI) and Rust bindings for the ELSTER Rich Client (ERiC).

![Taxel mockup](/mockup.png)

Generate an XML file in the XBRL standard from a CSV file with tax and accounting
data; validate and send the XBRL document to tax authorities.

Supported features:

- [x] eBilanz

---

- [What is ELSTER?](#what-is-elster)
- [What is ERiC?](#what-is-eric)
- [What is eBilanz?](#what-is-ebilanz)
- [Requirements](#requirements)
- [Install Taxel](#install-taxel)
- [Usage](#usage)
- [Testing](#testing)
- [Rust bindings for the ELSTER Rich Client (ERiC)](#rust-bindings-for-the-elster-rich-client-eric)
  - [Generate bindings](#generate-bindings)
  - [Test bindings](#test-bindings)

## What is ELSTER?

Elster (short for _Elektronische Steuererklärung_) is a project by the German tax administrations to process tax returns and declarations.

## What is ERiC?

ERiC is a C library that is integrated into a tax application. ERiC checks the data supplied by the tax application for plausibility, and transmits the data encrypted to the computing center of the respective tax administration.

## What is eBilanz?

eBilanz (short for _Elektronische Bilanz_) is the electronic transmission of the company balance sheet and P&L in a standardized format (XBRL) to the tax authorities in the context of tax declaration.

## Requirements

You need to have the shared library `libericapi.so` and the header file `ericapi.h` available on your system which can be downloaded from [ELSTER for developers](https://www.elster.de/elsterweb/entwickler/login) after access has been requested [here](https://www.elster.de/elsterweb/registrierung-entwickler/form).

For generating the bindings on your platform and architecture, you need `libclang` as well. For example, on Debian/Ubuntu install:

``` bash
apt install llvm-dev libclang-dev clang
```

## Install Taxel

``` bash
git clone git@github.com:quambene/taxel.git
cd taxel

# Build and install taxel binary to ~/.cargo/bin
cargo install --path ./taxel-cli
```

_Note:_ Run `cargo install --path ./taxel-cli` again to update to the latest version. Uninstall the binary with `cargo uninstall taxel`.

## Usage

1. Create an environment file:

    ``` bash
    touch .env
    ```

1. Set environment variables `LIBRARY_PATH`, `LIBRARY_NAME`, `HEADER_FILE`, `PLUGIN_PATH`, and `LD_LIBRARY_PATH` in your `.env`. For example:

    ``` bash
    LIBRARY_PATH=ERiC-36.1.8.0-Linux-x86_64/ERiC-36.1.8.0/Linux-x86_64/lib
    LIBRARY_NAME=ericapi
    HEADER_FILE=ERiC-36.1.8.0-Linux-x86_64/ERiC-36.1.8.0/Linux-x86_64/include/ericapi.h
    PLUGIN_PATH=ERiC-36.1.8.0-Linux-x86_64/ERiC-36.1.8.0/Linux-x86_64/lib/plugins2
    LD_LIBRARY_PATH=ERiC-36.1.8.0-Linux-x86_64/ERiC-36.1.8.0/Linux-x86_64/lib
    ```

1. Source your environment:

    ``` bash
    set -a && source .env && set +a
    ```

1. Run taxel:

    ``` bash
    # Extract values from xml file
    taxel extract \
        --xml-file "my_ebilanz.xml" \
        --output-file "my_ebilanz.csv"

    # Generate xml file from csv file
    taxel generate \
         --csv-file "my_ebilanz.csv" \
         --template-file "templates/elster_v11_ebilanz_v6.5_test.xml" \
         --output-file "my_bilanz.xml"

    # Validate xml file
    taxel validate \
        --tax-type "Bilanz" \
        --tax-version 6.4 \
        --xml-file "my_tax_data.xml"

    # Validate xml file and print confirmation as pdf file
    taxel validate \
        --tax-type "Bilanz" \
        --tax-version 6.4 \
        --xml-file "my_tax_data.xml" \
        --print "my_eBilanz.pdf"

    # Send xml file to tax authorities
    taxel send \
        --tax-type "Bilanz" \
        --tax-version 6.4 \
        --xml-file "my_tax_data.xml" \
        --certificate-file "my_elster_certificate.pfx" \
        --password "my_password"

    # Send xml file to tax authorities and print confirmation as pdf file
    taxel send \
        --tax-type "Bilanz" \
        --tax-version 6.4 \
        --xml-file "my_tax_data.xml" \
        --certificate-file "my_elster_certificate.pfx" \
        --password "my_password" \
        --print "my_eBilanz.pdf"
    ```

_Remark_: In step 2, note the difference between file name (e.g. `libericapi.so` on Linux) and `LIBRARY_NAME` (which is `ericapi`).

## Testing

``` bash
# Run unit tests for taxel
cargo test -p taxel -- --test-threads=1

# Run integration tests for taxel
cargo test -p taxel --test '*' --features integration-test -- --test-threads=1

# Run integration tests for taxel in release mode
cargo test -p taxel --release --test '*' --features integration-test -- --test-threads=1

# Run unit tests for taxel-cli
cargo test --lib -p taxel-cli

# Run integration tests for taxel-cli
cargo test -p taxel-cli --test '*' --features integration-test -- --test-threads=1

# Run integration tests for taxel-cli in release mode
cargo test -p taxel-cli --release --test '*' --features integration-test -- --test-threads=1

# Run unit tests for taxel-xml
cargo test --lib -p taxel-xml

# Run unit tests for taxel-py
cd taxel-py
pytest -v -m unit
```

## Rust bindings for the ELSTER Rich Client (ERiC)

### Generate bindings

The bindings have to be generated on-the-fly for your specific platform and architecture:

``` bash
cargo build -p taxel-bindings
```

The bindings are generated in `target/debug/build/taxel-<random-id>/out/bindings.rs`.

### Test bindings

The bindings are included in `src/lib.rs` via `include!` macro and tested by:

``` bash
cargo test -p taxel-bindings --lib
```

Logs are written to `eric.log` in the current directory.
