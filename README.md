# inf-fingerprint

Browser fingerprinting for mainland-China mobile traffic. Two pieces:

- **`client/`** — a Rust→WASM SDK that collects ~120 device signals,
  parses CN in-app browser tokens (微信 / 支付宝 / 钉钉 / 飞书 / QQ / UC /
  Quark / 抖音 / 头条 / X5 / XWEB), and detects on-device noise injection
  (Brave farbling, iOS 26 ATFP).
- **`server/`** — a Rust service (axum + sqlx + Postgres) that takes the
  raw feature payload and runs a Naive-Bayes match against known visitor
  signatures. Returns a stable visitor id with confidence and per-feature
  drift flags. Server-side inference handles partial matches and feature
  drift (system update, font set change, …) that pure-client hashing
  can't tolerate.

The OSS build of `@fingerprintjs/fingerprintjs` is small but offers no signal
for in-app browsers (where iOS WebKit canvas/audio are largely normalized) and
collapses on any noise. This library trades bundle size for accuracy.

## Layout

```
inf-fingerprint/
├── Cargo.toml          # workspace root
├── client/             # WASM SDK
│   ├── Cargo.toml
│   └── src/
├── server/             # identification service
│   ├── Cargo.toml
│   ├── config.toml.example
│   ├── Dockerfile
│   ├── migrations/
│   └── src/
└── examples/demo.html
```

## Client

```bash
cargo install wasm-pack
wasm-pack build client --target web --release
```

```js
import init, { getFingerprint } from "inf-fingerprint";

await init();
const fp = await getFingerprint();
console.log(fp.visitorId);          // local fallback hash
console.log(fp.toJSON());            // full feature payload — POST to /v1/identify
```

## Server

```bash
# 1. Drop creds in place
cp server/config.toml.example server/config.toml
# edit server/config.toml

# 2. Run
cargo run -p inf-fingerprint-server
```

Environment overrides config path: `INF_FP_CONFIG=/etc/inf-fp/config.toml`.

### Schema isolation

The server runs in a single Postgres database but installs every table under
its own schema (default `inf_fp`). `search_path` is set on every pooled
connection, so application queries stay unqualified. Migrations are tracked
in `inf_fp._sqlx_migrations` and run automatically on startup.

### `POST /v1/identify`

Request: the JSON output of `getFingerprint().toJSON()`.

Response:

```json
{
  "visitor_id": "0d9f7c87-0c98-4d12-9c53-...",
  "match_kind": "exact" | "fuzzy" | "ambiguous" | "new",
  "score": 34.5,
  "second_score": 12.0,
  "candidates": [
    { "visitor_id": "...", "score": 34.5, "hits": [["canonical_ua", 6.0], ...] }
  ],
  "drift": ["system_version", "canvas"],
  "observation_count": 42
}
```

`score` is the natural-log sum of per-feature likelihood ratios. Default
threshold for an `exact` match is +14 (≈match_threshold +6); `fuzzy` between
`match_threshold` and `match_threshold+6`; `ambiguous` between
`ambiguous_threshold` and `match_threshold` (returned as a fresh visitor but
flagged); `new` below `ambiguous_threshold`. Tune in `[matcher]`.

### Deploy

```bash
docker build -t inf-fingerprint-server:0.1.0 -f server/Dockerfile .

docker run -d \
  --name inf-fp \
  -p 8080:8080 \
  -v /etc/inf-fp/config.toml:/app/config.toml:ro \
  --restart=unless-stopped \
  inf-fingerprint-server:0.1.0
```

## Develop

```bash
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude inf-fingerprint
cargo test -p inf-fingerprint --lib china::tests
wasm-pack build client --target web --release
```

## License

MIT.
