## Steps to run the example

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
# The address for the server to bind to
bind_address = "0.0.0.0:8090"

# The upstream HTTP endpoint (e.g. for forwarding requests)
upstream_url = "http://validium-sequencer:8545"

# The L2 RPC endpoint for onchain verification (smart contract signature verification)
l2_rpc_url = "http://validium-l2:8545"

# List of admin API tokens. Only users with these tokens can access admin functions.
admin_keys = [
  "admin-token-1-abcdefg",
  "admin-token-2-hijklmn"
]

# JWT token expiry in seconds
# Timeout is not exact, there is a 60s leeway by default
jwt_expiry_secs = 3600

# The key ID used to sign new JWT tokens.
# This must match the 'kid' of one of the entries in 'jwt_signer_keys'.
default_kid = "key-2025-07"

# JWT signer keys; to invalidate a key, simply remove its entry.
# Each key must have a unique 'kid' (key ID).
jwt_signer_keys = [
    { kid = "key-2025-07", secret = "supersecret1" },
    { kid = "key-2025-06", secret = "supersecret2" }
]
```

### Override with CLI

```sh
cargo run -- --bind-address 127.0.0.1:9000 --upstream-url http://localhost:8545
```

### Config file path

By default, the server uses `config.toml` in the current directory. You can use `--config config.toml` to specify a different path.

### Defaults

If not specified, `bind_address` defaults to `0.0.0.0:8080`, `upstream_url` defaults to `http://validium-sequencer:8545`, and `l2_rpc_url` defaults to `http://localhost:8545`.

### Precedence

Precedence order for configuration is: CLI arguments > `config.toml` > defaults.

## SIWE Authentication & Signature Verification

This proxy server supports Sign-In with Ethereum (SIWE) authentication with comprehensive signature verification for different account types:

1. EOA (Externally Owned Accounts): Traditional ECDSA signature verification

2. Smart Contract Accounts: ERC-1271 signature verification via onchain calls

3. EIP-7702 Accounts: Hybrid verification supporting both contract and EOA signatures

### L2 RPC Configuration
The `l2_rpc_url` field in the configuration specifies the L2 RPC endpoint used for:

- Checking account code to determine account type

- Performing ERC-1271 signature verification for smart contract accounts

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
