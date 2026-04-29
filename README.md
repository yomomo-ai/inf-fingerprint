# inf-fingerprint

Browser fingerprint library written in Rust, compiled to WASM. Tuned for
mainland-China mobile traffic where most sessions arrive through in-app
browsers (微信 / 支付宝 / 钉钉 / 飞书 / QQ / UC / Quark / 抖音 / 头条 / X5).

The OSS build of `@fingerprintjs/fingerprintjs` is small but offers no signal
for in-app browsers, where `MicroMessenger/`, `AlipayClient/`, `TBS/`,
`NetType/`, `__wxjs_environment` and the various JS bridges carry most of the
distinguishing entropy on iOS WebKit. This library covers those, alongside
the standard canvas / webgl / audio / screen / navigator / timezone / fonts /
touch / permissions probes.

Total bundle so far: ~88 KB gzip (wasm + js glue).

## Build

```bash
cargo install wasm-pack
wasm-pack build --target web --release
```

Other targets work too: `--target bundler`, `--target nodejs`,
`--target no-modules`. Output goes to `pkg/`.

## Use

```js
import init, { getFingerprint } from "inf-fingerprint";

await init();
const fp = await getFingerprint();
console.log(fp.visitorId);
console.log(fp.toJSON());
```

`fp.toJSON()` returns

```jsonc
{
  "visitor_id": "a1b2c3d4e5f6a7b8",
  "version": "0.1.0",
  "confidence": 0.9,
  "components": {
    "canvas":      { "hash": "...", "winding": false, "cjk_text_width": 64.0, ... },
    "webgl":       { "vendor": "...", "unmasked_renderer": "Apple GPU", ... },
    "audio":       { "hash": "...", "sample_checksum": 124.45, ... },
    "screen":      { "width": 390, "height": 844, "device_pixel_ratio": 3.0, ... },
    "navigator":   { "user_agent": "...", "ua_client_hints": {...}, ... },
    "timezone":    { "timezone": "Asia/Shanghai", "locale": "zh-CN", ... },
    "fonts":       { "available": ["PingFang SC","Microsoft YaHei",...] },
    "touch":       { "max_touch_points": 5, "coarse_pointer": true, ... },
    "permissions": { "geolocation": "prompt", ... }
  },
  "china": {
    "in_app": "wechat",
    "in_app_version": "8.0.42",
    "mini_program_env": null,
    "net_type": "WIFI",
    "language_tag": "zh_CN",
    "x5_kernel": { "version": "045901" },
    "bridges": { "weixin_jsbridge": true, ... },
    "device_vendor": "apple",
    "system_rom": "ios",
    "system_version": "17.5.1",
    "user_agent": "..."
  }
}
```

`visitorId` is an xxh3-64 of all collected components. Component-level
collection failures degrade to `null` rather than throwing.

## Develop

```bash
cargo check --target wasm32-unknown-unknown
cargo test --lib china::tests
cargo clippy --target wasm32-unknown-unknown -- -D warnings
```

The release profile is already size-tuned (`opt-level = "z"`, LTO, single
codegen unit, `panic = abort`). `wasm-pack` does not invoke `wasm-opt`
because the binaryen download from GitHub is unreliable on mainland networks.
Install binaryen separately and run `wasm-opt -Oz` on `pkg/*.wasm` to shave
another ~30%.

`examples/demo.html` is a manual test page; serve `pkg/` and `examples/` from
any static server.

## License

MIT
