# CodeIndex

CodeIndex is a local-first high performance codebase index engine designed for AI. It helps your LLM understanding the structure and semantics of a codebase and grabs code context based on the inputs.

## Development

### Install Dependencies

```bash
rustup component add rustfmt
brew install protobuf
cargo install cargo-watch
```

### Run

```bash
cargo watch -x run
```

### Lint

```bash
cargo fmt
cargo clippy
```