# taxel-py

Convert XSD to XML.

## Setup

```bash
uv sync
```

## Usage

```bash
uv run python src/xsd_to_xml.py
```

## Testing

```bash
uv run pytest
```

## Dependencies

Update the lock file after changing dependencies in `pyproject.toml`:

```bash
uv lock
```

Add or update a specific package:

```bash
uv add lxml
```
