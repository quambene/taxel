# Taxel

[![build status](https://github.com/quambene/taxel/actions/workflows/ci.yml/badge.svg)](https://github.com/quambene/taxel/actions/workflows/ci.yml)

Taxel provides a command line interface (CLI) to generate the electronic balance
sheet (eBilanz) in the XBRL format.

![Taxel mockup](/mockup.png)

Generate an XML file in the XBRL standard from a CSV file with tax and accounting
data; validate and send the XBRL document to tax authorities.

Supported features:

- [x] eBilanz

---

- [What is eBilanz?](#what-is-ebilanz)
- [Install Taxel](#install-taxel)
- [Usage](#usage)
- [Testing](#testing)
- [Rust bindings and SDK for the ELSTER Rich Client (ERiC)](#rust-bindings-and-sdk-for-the-elster-rich-client-eric)

## What is eBilanz?

eBilanz (short for _Elektronische Bilanz_) is the electronic transmission of the company balance sheet and P&L in a standardized format (XBRL) to the tax authorities in the context of tax declaration.

## Install Taxel

``` bash
git clone git@github.com:quambene/taxel.git
cd taxel

# Build and install taxel binary to ~/.cargo/bin
cargo install --path ./taxel-cli
```

_Note:_ Run `cargo install --path ./taxel-cli` again to update to the latest version. Uninstall the binary with `cargo uninstall taxel`.

## Usage

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

# Send xml file to tax authorities and print confirmation as pdf file
taxel send \
    --tax-type "Bilanz" \
    --tax-version 6.4 \
    --xml-file "my_tax_data.xml" \
    --print "my_eBilanz.pdf"
```

## Testing

``` bash
# Run unit tests for taxel
cargo test -p taxel -- --test-threads=1

# Run integration tests for taxel
cargo test -p taxel --test '*' --features integration-test -- --test-threads=1

# Run integration tests for taxel in release mode
cargo test -p taxel --release --test '*' --features integration-test -- --test-threads=1

# Run unit tests for taxel-cli
cargo test -p taxel-cli

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

## Rust bindings and SDK for the ELSTER Rich Client (ERiC)

Rust bindings and SDK for ERiC were moved to <https://github.com/quambene/eric-rs>.
