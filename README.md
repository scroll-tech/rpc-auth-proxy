## Steps to run the example

1. Run the proxy server:

```sh
cargo run -- --config config.toml
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
validium_url = "http://validium-sequencer:8545"
```

### Override with CLI

```sh
cargo run -- --bind-address 127.0.0.1:9000 --validium-url http://localhost:8545
```

### Config file path

By default, the server uses `config.toml` in the current directory. You can use `--config config.toml` to specify a different path.

### Defaults

If not specified, `bind_address` defaults to `0.0.0.0:8080`, `validium_url` defaults to `http://validium-sequencer:8545`.

### Precedence

Precedence order for configuration is: CLI arguments > `config.toml` > defaults.

## Authorization & Admin Key Management

### Admin keys

You can specify one or more admin API tokens via the `admin_keys` field in `config.toml`:

```toml
admin_keys = ["admin-token-1-abcdefg", "admin-token-2-hijklmn"]
```

Any HTTP request with header

```http
Authorization: Bearer <admin_key>
```

matching one of these values will receive full admin permissions.

### Access levels

**Full**: Requests using a valid admin key.

**Restricted**: Requests with a regular JWT.

**None**: Requests without any authorization or with invalid authorization.

### Security and key rotation

1. Treat admin keys as sensitive credentials.

2. Rotate admin keys by updating the config and restarting the server.

## JWT Signer Key Management (Key Rotation)

### JWT signer keys

Used for signing and verifying user JWT tokens.

Each entry under `jwt_signer_keys` must have a unique `kid` and `secret`.

The `default_kid` field specifies which key is used to sign new JWTs.

**Example:**

```toml
default_kid = "key-2025-07"
jwt_signer_keys = [
  { kid = "key-2025-07", secret = "supersecret1" },
  { kid = "key-2025-06", secret = "supersecret2" }
]
```

### Token expiry

The `jwt_expiry_secs` field sets the lifetime (in seconds) for newly issued JWT tokens.

**Example:**

```toml
jwt_expiry_secs = 3600  # JWTs are valid for 1 hour, timeout is not exact, there is a 60s leeway by default
```

### Key rotation

To rotate JWT signer keys, you can add a new key entry to `jwt_signer_keys`, set `default_kid` to the new key, and optionally remove old keys.

For a graceful key rotation (phased removal):

1. After switching `default_kid`, keep old keys in `jwt_signer_keys` (for verification only), so old tokens remain valid until expiry.

2. Once all old tokens are expired (i.e., after `jwt_expiry_secs`), you can safely remove the old key entry.

3. Old JWTs signed with removed keys will be rejected.

**Note**: After changing keys, restart the server to reload configuration.

### Security and key rotation

1. Treat JWT signer keys as sensitive credentials.

2. Rotate keys regularly for better security.
