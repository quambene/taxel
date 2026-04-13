# Taxel GUI

- [Development](#development)
  - [Requirements](#requirements)
  - [Usage](#usage)
- [Architecture](#architecture)

## Development

### Requirements

``` bash
cargo install dioxus-cli # Install dioxus CLI
```

### Usage

``` bash
dx serve --hotpatch # Run app with hot reload
```

## Architecture

The app is structured as follows:

- `domain`: contains pure business data and rules
- `app`: contains application state and orchestration logic that coordinates
  domain objects and feature behavior
- `infrastructure`: handles external concerns like files, XML parsing, JSON
  storage, and network I/O
- `ui`: contains egui-based rendering code that displays application state and
  forwards user interactions to the app layer

``` txt
src/
  main.rs
  lib.rs
  app/
    mod.rs
    ...
  domain/
    mod.rs
    ...
  infrastructure/
    mod.rs
    ...
  ui/
    mod.rs
    report_view.rs
    ...
```

The typical operation flow looks like this:

``` txt
ui (user clicks button)
   ↓
app (load_and_validate_report)
   ↓
infrastructure (load + parse XML)
   ↓
domain (validate report)
   ↓
app returns result
   ↓
ui updates state
```
