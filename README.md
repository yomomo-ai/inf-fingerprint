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

Build locally:

```bash
cargo install wasm-pack
wasm-pack build client --target web --release
```

The published npm package is `inf-fingerprint`. The recommended entry point is
`identify()`, which collects features, calls the server, caches the response in
localStorage (with stale-while-revalidate), and falls back to a local hash when
the server is unreachable:

```js
import init, { identify } from "inf-fingerprint";

await init();
const id = await identify({
  endpoint: "https://fp.example.com/v1/identify",
});
console.log(id.visitor_id, id.match_kind, id.from_server);
```

Full integration guide: [`docs/integration.zh.md`](docs/integration.zh.md).

A lower-level `getFingerprint()` is also exported for callers who want the raw
feature struct (e.g. to POST themselves, or to use the local hash without ever
talking to a server).

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

Wire format is msgpack on both directions (`Content-Type: application/msgpack`).
The request body is the msgpack-encoded feature struct produced by
`getFingerprint()`; the SDK's `identify()` does this for you.

Response (decoded):

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
  "observation_count": 42,
  "via_persistence": false
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
runtime (~35MB final, glibc only, no shell).

CI publishes pre-built images to GHCR on every push to `main` — pull rather
than rebuild unless you're modifying the Dockerfile:

```bash
docker pull ghcr.io/yomomo-ai/inf-fingerprint-server:latest
docker run -d \
  --name inf-fp \
  --init \
  --restart=unless-stopped \
  -p 28091:28091 \
  --add-host=host.docker.internal:host-gateway \
  -v /etc/inf-fp/config.toml:/app/config.toml:ro \
  ghcr.io/yomomo-ai/inf-fingerprint-server:latest
```

To build locally for the deploy host from a Mac dev machine, target
linux/amd64 explicitly:

```bash
docker buildx build --platform linux/amd64 \
  -t inf-fingerprint-server:dev \
  -f server/Dockerfile .
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

- Touching `client/**` (or any shared file in the filter — `Cargo.toml`,
  `Cargo.lock`, `rust-toolchain.toml`, `build.yml`) triggers a wasm-pack
  build, then `npm publish` to the public registry. The patch number is
  stamped automatically from `git rev-list --count HEAD` so every push
  ships a fresh, monotonic version (`0.1.27`, `0.1.28`, …) — no manual
  `Cargo.toml` bump required.
- Touching `server/**` triggers a docker build and pushes
  `ghcr.io/yomomo-ai/inf-fingerprint-server` tagged with `latest`, `main`,
  and the short SHA.

To bump the major or minor (i.e. ship breaking changes), edit
`client/Cargo.toml`'s `[package].version` to the new `MAJOR.MINOR.0` —
the CI keeps your major/minor and rewrites only the patch. Document the
break in this README before pushing.

## License

MIT.
