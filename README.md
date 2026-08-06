<!-- markdownlint-disable MD024 -->

# Taxel

[![build status](https://github.com/quambene/taxel/actions/workflows/ci.yml/badge.svg)](https://github.com/quambene/taxel/actions/workflows/ci.yml)

Taxel provides a GUI and command line interface (CLI) to generate the electronic balance
sheet (eBilanz) in the XBRL format.

![Taxel mockup](/mockup.png)

Generate a report in the XBRL standard with tax and accounting data; validate
and send the XBRL document to the tax authorities.

Supported features:

- [x] eBilanz

---

- [What is eBilanz?](#what-is-ebilanz)
- [Taxel GUI](#taxel-gui)
  - [Install Taxel GUI](#install-taxel-gui)
  - [Usage](#usage)
- [Taxel CLI](#taxel-cli)
  - [Install Taxel CLI](#install-taxel-cli)
  - [Usage](#usage-1)
  - [Testing](#testing)
- [Rust bindings and SDK for the ELSTER Rich Client (ERiC)](#rust-bindings-and-sdk-for-the-elster-rich-client-eric)
- [Changelog](#changelog)

## What is eBilanz?

eBilanz (short for _Elektronische Bilanz_) is the electronic transmission of the company balance sheet and P&L in a standardized format (XBRL) to the tax authorities in the context of tax declaration.

## Taxel GUI

### Install Taxel GUI

Prebuilt binaries and releases are not provided in this repository. Releases
will be published in the
[taxel-releases](https://github.com/IO-Propagator/taxel-releases) repo.

### Usage

1. Create new report
1. Search for ID `genInfo.report.id.reportElement` in the GCD report section,
   and select the relevant report elements (e.g. balance sheet and income
   statement)
1. Fill out GCD and selected report sections
1. Validate report
1. Send report (the report is sent to the respective tax authority, based on the
   13-digit tax number in the GCD section).

Instead of creating a new report, you can also load an existing report via
button `Import report`. However, the taxonomy of the imported report is
preserved which might be outdated.

To import values from an existing report but using an up-to-date taxonomy,
create a new report first. Then, import values of the existing report via button
`Import values`. values`.

## Taxel CLI

### Install Taxel CLI

``` bash
git clone git@github.com:quambene/taxel.git
cd taxel

# Build and install taxel binary to ~/.cargo/bin
cargo install --path ./taxel-cli
```

_Note:_ Run `cargo install --path ./taxel-cli` again to update to the latest version. Uninstall the binary with `cargo uninstall taxel`.

### Usage

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

### Testing

``` bash
# Run unit tests for taxel-cli
cargo test -p taxel-cli

# Run integration tests for taxel-cli (requires ERiC library)
cargo test -p taxel-cli --test '*' -- --test-threads=1

# Run external tests for taxel-cli (requires ERiC library and Elster certificate)
cargo test -p taxel-cli --release --test '*' --features external-test -- --test-threads=1

# Run unit tests for taxel
cargo test --lib -p taxel

# Run unit tests for taxel-py
cd taxel-py
pytest -v -m unit
```

## Rust bindings and SDK for the ELSTER Rich Client (ERiC)

Rust bindings and SDK for ERiC were moved to <https://github.com/quambene/eric-rs>.

## Changelog

The `taxel` repository contains multiple crates with separate changelogs:

- workspace: [view changelog](https://github.com/quambene/taxel/blob/main/CHANGELOG.md)
- `taxel`: [view changelog](https://github.com/quambene/taxel/blob/main/taxel/CHANGELOG.md)
- `taxel-cli`: [view changelog](https://github.com/quambene/taxel/blob/main/taxel-cli/CHANGELOG.md)
- `taxel-gui`: [view changelog](https://github.com/quambene/taxel/blob/main/taxel-gui/CHANGELOG.md)
