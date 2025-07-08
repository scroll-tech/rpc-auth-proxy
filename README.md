# Steps to run the example

1. Run the proxy server:

```sh
cargo run
```

2. Run the example frontend:

```sh
cd examples/siwe-frontend && yarn && yarn start
```

3. Go to http://localhost:8080


## Configuration

You can configure the proxy server via an optional `config.toml` file or command-line arguments. If neither is set, built-in defaults are used.

`config.toml` example:

```toml
bind_address = "0.0.0.0:8080"
upstream_url = "http://validium-sequencer:8545"
```

**Override with CLI**:

```sh
cargo run -- --bind-address 127.0.0.1:9000 --upstream-url http://localhost:8545
```

**Config file path**:

By default, the server uses `config.toml` in the current directory. You can use `--config config.toml` to specify a different path.

**Defaults**:

If not specified, `bind_address` defaults to `0.0.0.0:8080`, `upstream_url` defaults to `http://validium-sequencer:8545`.

**Precedence**: CLI arguments > config.toml > defaults.
