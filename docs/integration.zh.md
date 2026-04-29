# 前端接入指南

`inf-fingerprint` 是一个 Rust→WASM 编译的浏览器指纹库，专门针对中国大陆移动端 in-app 浏览器（微信 / 支付宝 / 钉钉 / 飞书 / QQ / UC / Quark / 抖音 / 头条 / X5 / XWEB）做了识别和归一化处理。SDK 把"采集设备特征 → 调服务端做贝叶斯匹配 → localStorage 缓存"整条链路打包成一行 `await identify({ endpoint })`。

服务端是配套的 axum + Postgres 服务（`server/` 目录），自部署。本文只讲前端接入；服务端部署见 [`README.md`](../README.md)。

## 一、安装

```bash
npm install inf-fingerprint
# 或 pnpm add inf-fingerprint / yarn add inf-fingerprint
```

ESM-only 包，由 wasm-pack 生成，**仅浏览器使用**。Node / SSR 环境下请确保只在 `useEffect` / `onMounted` / `if (typeof window !== "undefined")` 等浏览器分支调用。

## 二、最简调用

```js
import init, { identify } from "inf-fingerprint";

await init();
const id = await identify({
  endpoint: "https://your-fp-server.example.com/v1/identify",
});

console.log(id.visitor_id); // "0d9f7c87-0c98-4d12-9c53-..." 形式的 UUID
```

`init()` 必须在第一次调用前执行一次，加载 WASM 模块（约 100KB gz：~92KB wasm + ~9KB JS glue，浏览器自动缓存）。后续整个 tab 内复用，无需重复 `init`。

## 三、API

### 3.1 函数签名

```ts
function identify(options: {
  endpoint: string;           // 必填：服务端地址，全 URL
  apiKey?: string;            // 可选：见 §八 鉴权说明
  cacheTtlSeconds?: number;   // 缓存硬过期，默认 86400 (24h)
  staleSeconds?: number;      // SWR 软过期，默认 cacheTtlSeconds / 2
  forceRefresh?: boolean;     // 跳过缓存强制走网络，默认 false
  timeoutMs?: number;         // fetch 超时（毫秒），默认 5000
}): Promise<IdentityResult>;
```

### 3.2 返回字段（`IdentityResult`）

| 字段 | 类型 | 说明 |
|---|---|---|
| `visitor_id` | string | 稳定的访客 ID，**这是要存的主键**。在线时是 UUID（如 `0d9f7c87-...`）；`match_kind === "offline"` 时是 16 位 hex 哈希 |
| `match_kind` | string | `"exact"` / `"fuzzy"` / `"ambiguous"` / `"new"` / `"offline"`，见 §3.3 |
| `score` | number | 贝叶斯打分（log-likelihood），调试用；越高置信越强 |
| `second_score` | number | 第二高候选分数；越接近 `score` 越说明这次匹配可能模棱两可 |
| `observation_count` | number | 该 visitor 在服务端累计被观测过多少次（首次访问后 ≥1） |
| `via_persistence` | boolean | **服务端**侧——`true` 表示客户端在请求体里带了之前存下的 `client_visitor_id` 且与服务端记录吻合，走了快速路径；`false` 表示靠特征 bucket 扫描匹配的（或新访客）。**和 `cached` 字段无关** |
| `from_server` | boolean | 本次结果是否最终源自服务端响应。**注意**：从浏览器缓存读出来的也是 `true`（缓存的就是上次成功的服务端响应）；只有服务端完全不可达且无可用缓存时才是 `false` |
| `cached` | boolean | 本次结果是否直接命中浏览器 localStorage 缓存（没发请求） |
| `stale` | boolean | 缓存命中但已过 SWR 阈值，后台正在异步刷新；当前调用拿到的还是旧值 |
| `cached_at_ms` | number | 此结果**首次**从服务端拿到的时间戳（`Date.now()` 毫秒），无论本次是否走缓存 |

### 3.3 `match_kind` 取值

| 值 | 含义 | 业务侧建议 |
|---|---|---|
| `"exact"` | 高置信匹配，已识别为已知访客 | 直接信任 |
| `"fuzzy"` | 系统更新 / 字体变化等正常漂移，仍判为已知访客，签名已自动更新 | 信任 |
| `"ambiguous"` | 有近似候选但分数不够过 match_threshold，**已新建** visitor_id 但服务端打了 review 标记 | 可走风控人工 / 二次校验 |
| `"new"` | 全新访客，没有近似候选 | 信任，作为新用户处理 |
| `"offline"` | 服务端不可达，用浏览器本地哈希降级 | **不要持久化或作为业务主键** |

## 四、缓存行为（SWR）

`identify()` 内置 stale-while-revalidate 缓存（写在 `localStorage` 的 `__inf_fp_identity_cache`）：

```
首次调用：             走网络 → 写 localStorage → 返回 (cached: false)
~ staleSeconds 内：     读缓存 → 立即返回 (cached: true, stale: false)
staleSeconds ~ ttl：    读缓存立即返回 (cached: true, stale: true) + 后台异步刷新
> cacheTtlSeconds：     缓存失效 → 走网络
```

默认 `cacheTtlSeconds = 86400`（24h），`staleSeconds = 43200`（12h）。

**性能特征**：
- 首次调用通常**几百 ms 到 1 秒**——大头是端侧特征采集（WebRTC 探测就有 ~800ms 上限），网络往返只占小头
- 24h 内复用：**同步从 localStorage 读，零网络开销**
- `forceRefresh: true` 可强制重新识别，会跳过缓存且不写后台刷新；慎用

## 五、典型用法

### 5.1 用户匿名识别 + 埋点

```js
import init, { identify } from "inf-fingerprint";

let visitorId = null;

(async () => {
  await init();
  const id = await identify({
    endpoint: "https://your-fp-server.example.com/v1/identify",
  });
  visitorId = id.visitor_id;
  yourTracker.setUser({ visitorId, fingerprint: id });
})();
```

### 5.2 风控 / 反作弊（更激进的缓存策略）

```js
const id = await identify({
  endpoint: "https://your-fp-server.example.com/v1/identify",
  cacheTtlSeconds: 300, // 5 分钟，比埋点场景紧
});

if (id.match_kind === "ambiguous" || id.match_kind === "offline") {
  // 走风控人工 / 二次校验
}
if (id.via_persistence === false && id.observation_count > 50) {
  // 老访客但客户端持久化（cookie / localStorage / sessionStorage）被清
  // 可能是隐身模式 / 设备清理 / 刷子换号
}
```

### 5.3 React Hook

```jsx
import { useEffect, useState } from "react";
import init, { identify } from "inf-fingerprint";

let initPromise = null;

function useFingerprint(endpoint) {
  const [id, setId] = useState(null);
  useEffect(() => {
    if (!initPromise) initPromise = init();
    initPromise
      .then(() => identify({ endpoint }))
      .then(setId);
  }, [endpoint]);
  return id;
}
```

### 5.4 Vue 3 Composable

```js
import { ref, onMounted } from "vue";
import init, { identify } from "inf-fingerprint";

let initPromise = null;

export function useFingerprint(endpoint) {
  const id = ref(null);
  onMounted(async () => {
    if (!initPromise) initPromise = init();
    await initPromise;
    id.value = await identify({ endpoint });
  });
  return id;
}
```

## 六、错误与离线兜底

`identify()` **永远不抛异常**（除非 `endpoint` 字段缺失）；服务端不可达或网络故障会优雅降级，返回：

```js
{
  match_kind: "offline",
  from_server: false,
  visitor_id: "a3b5c7d9e1f24680", // 16 位 hex，xxh3 哈希
  score: 0,
  observation_count: 0,
  // ... 其他字段也都有合理默认值
}
```

兜底 ID 是用本地稳定特征算的 xxh3 哈希（同设备同浏览器多次调用应该一致），**与服务端的 UUID 空间完全不同**。仅用于"页面能继续跑"的降级，**不要持久化或作为业务主键**。

**消费侧统一兜底模板**：

```js
const id = await identify({ endpoint });

if (!id.from_server || !id.visitor_id) {
  // 降级路径：不入库 / 单独打 offline 标记 / fallback 到自有 anonymousId
} else {
  // 正常路径
}
```

极端情况：如果连特征采集本身都失败了（极少见，比如 WASM 启动异常），`visitor_id` 会是空字符串，`match_kind` 仍是 `"offline"`——上面的 `if` 条件能覆盖。

## 七、CORS

服务端默认 `Access-Control-Allow-Origin: *`，对所有 origin 开放，无需配置 origin 白名单。SDK 不带 cookie（fetch `credentials: "omit"`），不依赖跨站 cookie，浏览器侧任何域调用都能通过 preflight。

## 八、鉴权与限流

`apiKey` 字段在浏览器场景下**一律不传**。理由：

- 浏览器代码 minify 后也是明文，F12 能看到，`apiKey` 在浏览器侧根本无法保密
- 服务端**不校验** `X-API-Key`——这个字段的存在是给运维层（nginx / 网关 / WAF）做"限流豁免"判断用的，与业务鉴权无关
- 浏览器请求一律走运维层的常规限流通道；SDK 内置 SWR 缓存，正常用户一天最多触发一次网络请求，几乎不可能撞到合理设置的限流阈值

如果你的服务端运维层启用了"匹配某个 key 的请求绕过限流"的策略，那把那个 key 配给**服务端可信调用方**（不会经过浏览器的）；浏览器集成不需要、也不应该使用。

## 九、版本

- npm 包名：`inf-fingerprint`
- 当前主版本：`0.1.x`，patch 自动递增（CI 用 main 分支 commit 数当 patch 号）
- patch 之间保持向后兼容；major/minor bump 才有破坏性变更（会在 GitHub Release notes 列出）

升级：`npm update inf-fingerprint` 或锁版本 `^0.1.0`。

## 十、反馈

GitHub Issues：https://github.com/yomomo-ai/inf-fingerprint/issues

源码：[`client/src/identify.rs`](../client/src/identify.rs) 是这套 API 的实现。
