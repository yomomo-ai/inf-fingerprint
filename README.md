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
its own schema (default `fingerprint`). `search_path` is set on every pooled
connection, so application queries stay unqualified. Migrations are tracked
in `fingerprint._sqlx_migrations` and run automatically on startup.

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

`score` is the natural-log sum of per-feature likelihood ratios (Naive Bayes
with hand-calibrated weights — see `server/src/bayes.rs` for the full table
and reasoning). A clean iOS-WeChat full-feature match scores ≈40; a different
user sharing the same canonical UA (worst-case false-positive bucket) scores
≈3-8.

Defaults:
- `≥ match_threshold + 10` (≥22) → `exact`
- `≥ match_threshold` (≥12) → `fuzzy` — drift expected, signature updated
- `≥ ambiguous_threshold` (≥6) → `ambiguous` — new visitor created but flagged
- otherwise → `new`

Tune in `[matcher]`.

### Deploy

The Dockerfile is a 3-stage build: cargo-chef planner → cargo-chef builder
(deps cached as long as `Cargo.toml`/`Cargo.lock` are unchanged) → distroless
runtime (~35MB final, glibc only, no shell). To build for the deploy host
from a Mac dev machine, target linux/amd64 explicitly:

```bash
docker buildx build --platform linux/amd64 \
  -t inf-fingerprint-server:0.1.0 \
  -f server/Dockerfile .

docker run -d \
  --name inf-fp \
  -p 28091:28091 \
  --add-host=host.docker.internal:host-gateway \
  -v /etc/inf-fp/config.toml:/app/config.toml:ro \
  --restart=unless-stopped \
  inf-fingerprint-server:0.1.0
```

`--add-host=host.docker.internal:host-gateway` lets the container reach the
Postgres at the host's IP when PG runs outside Docker. On macOS / Windows
the alias resolves natively.

Distroless has no shell, so `docker exec -it inf-fp sh` doesn't work — for
debugging, swap the runtime base to `gcr.io/distroless/cc-debian13:debug`
(includes busybox) or run the binary in a regular `debian:bookworm-slim`.

## Develop

```bash
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude inf-fingerprint
cargo test -p inf-fingerprint --lib china::tests
wasm-pack build client --target web --release
```

## Releasing

`.github/workflows/build.yml` is path-aware: each push to `main` only
publishes the parts that actually changed.

- Touching `client/**` triggers a wasm-pack build and a conditional
  `npm publish` of the package version in `client/Cargo.toml`. If that
  version is already on npm the publish step no-ops with a notice; bump
  `[package].version` to ship a new release.
- Touching `server/**` (or anything the Dockerfile reads) triggers a docker
  build and pushes `ghcr.io/yomomo-ai/inf-fingerprint-server` tagged with
  `latest`, `main`, and the short SHA.

Cutting a client release:

```bash
$EDITOR client/Cargo.toml          # set [package].version = "0.2.0"
cargo update -p inf-fingerprint --precise 0.2.0
git commit -am "bump client to 0.2.0"
git push
```

Server changes ship as soon as you push to `main` — the image always tracks
HEAD via the `latest` and `<sha>` tags.

## License

MIT.
