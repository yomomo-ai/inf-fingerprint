# 接入 inf-fingerprint

给 yomomo-ai 旗下产品快速集成访客识别能力。线上服务已部署在 `https://fp.influo-ai.com`。

## 一、安装

```bash
npm install inf-fingerprint
# 或
pnpm add inf-fingerprint
```

ESM-only 包，由 wasm-pack 生成，纯前端使用。Node 环境（SSR）请仅在 `useEffect` / 浏览器分支调用。

## 二、最简调用

```js
import init, { identify } from "inf-fingerprint";

await init();
const id = await identify({
  endpoint: "https://fp.influo-ai.com/v1/identify",
});

console.log(id.visitor_id); // "0d9f7c87-0c98-4d12-9c53-..."
```

`init()` 必须在第一次调用前执行一次，加载 WASM 模块（约 100KB gz：~92KB wasm + ~9KB JS glue，浏览器自动缓存）。后续整个 tab 内复用，无需重复 `init`。

## 三、返回字段

| 字段 | 类型 | 说明 |
|---|---|---|
| `visitor_id` | string | 稳定的访客 ID，**这是你要存的东西**。在线时是 UUID（如 `0d9f7c87-...`）；`match_kind === "offline"` 时是 16 位 hex 哈希 |
| `match_kind` | string | `"exact"` / `"fuzzy"` / `"ambiguous"` / `"new"` / `"offline"`，见下方说明 |
| `score` | number | 贝叶斯打分（log-likelihood），调试用 |
| `second_score` | number | 第二高候选的分数，越接近 `score` 越可疑 |
| `observation_count` | number | 该访客在服务端累计被观测过多少次 |
| `via_persistence` | boolean | **服务端**侧——`true` 表示客户端在请求体里带了之前存下的 `client_visitor_id` 且与服务端记录吻合，走了快速路径；`false` 表示靠特征 bucket 扫描匹配的（或新访客）。**和下面 `cached` 字段无关** |
| `from_server` | boolean | 本次结果是否来自服务端响应。**注意**：从浏览器缓存读出来的也是 `true`（因为缓存的就是上次成功的服务端响应）；只有 server 完全不可达且无可用缓存时才是 `false` |
| `cached` | boolean | 本次结果是否直接命中浏览器 localStorage 缓存（没发请求） |
| `stale` | boolean | 缓存命中但已过 SWR 阈值，后台正在异步刷新；当前调用拿到的还是旧值 |
| `cached_at_ms` | number | 此结果首次从服务端拿到的时间戳（`Date.now()` 毫秒），无论是否走缓存 |

`match_kind` 含义：
- `exact` — 高置信，新老访客都信任
- `fuzzy` — 系统更新 / 字体变化等正常漂移，已自动更新画像
- `ambiguous` — 创建了新 visitor_id 但服务端打了 review 标记
- `new` — 全新访客
- `offline` — server 没连上，返回的是浏览器本地降级 ID（**不要用作长期主键**）

## 四、配置项

```ts
identify({
  endpoint: string,           // 必填
  apiKey?: string,            // 可选；浏览器场景一律不传（见 §八）
  cacheTtlSeconds?: number,   // 缓存硬过期，默认 86400 (24h)
  staleSeconds?: number,      // SWR 软过期，默认 cacheTtlSeconds / 2
  forceRefresh?: boolean,     // 跳过缓存强制走网络，默认 false
  timeoutMs?: number,         // fetch 超时，默认 5000
});
```

## 五、缓存策略

`identify()` 内置 SWR（stale-while-revalidate）机制：

```
首次调用：    走网络 → 写 localStorage → 返回
24h 内复用：  读缓存 → 立即返回（cached: true）
12h ~ 24h：   读缓存立即返回 + 后台异步刷新（stale: true）
> 24h：       缓存失效，走网络
```

首次调用通常**几百 ms 到 1 秒**——大头是端侧特征采集（WebRTC 探测就有 ~800ms 上限），网络往返只占小头。**24h 内复用都是同步从 localStorage 读出来，零网络开销**。`forceRefresh: true` 可强制重新识别，慎用。

## 六、典型用法

### 6.1 用户登录前埋点

```js
import init, { identify } from "inf-fingerprint";

let visitorId = null;

(async () => {
  await init();
  const id = await identify({
    endpoint: "https://fp.influo-ai.com/v1/identify",
  });
  visitorId = id.visitor_id;
  // 上报到你们自己的埋点系统
  yourTracker.setUser({ visitorId, fingerprint: id });
})();
```

### 6.2 风控 / 反作弊场景

```js
const id = await identify({
  endpoint: "https://fp.influo-ai.com/v1/identify",
  cacheTtlSeconds: 300, // 5 分钟，比埋点场景激进
});

if (id.match_kind === "ambiguous" || id.match_kind === "offline") {
  // 拉风控走人工 / 二次校验
}
if (id.via_persistence === false && id.observation_count > 50) {
  // 老访客但 cookie/localStorage 被清，可能是隐身模式或刷子
}
```

### 6.3 React 集成

```jsx
import { useEffect, useState } from "react";
import init, { identify } from "inf-fingerprint";

let initPromise = null;

function useFingerprint() {
  const [id, setId] = useState(null);
  useEffect(() => {
    if (!initPromise) initPromise = init();
    initPromise
      .then(() => identify({ endpoint: "https://fp.influo-ai.com/v1/identify" }))
      .then(setId);
  }, []);
  return id;
}
```

## 七、离线兜底

server 挂掉或网络不通时，`identify()` **不会抛异常**，而是返回：
```js
{
  match_kind: "offline",
  from_server: false,
  visitor_id: "a3b5c7d9e1f24680", // 16 位 hex，xxh3 哈希
  score: 0,
  observation_count: 0,
  ...
}
```

兜底 ID 是用本地采集到的稳定特征算的 xxh3 哈希（同设备同浏览器多次调用应该一致），**和服务端的 UUID 空间完全不同**。仅用于"页面能继续跑"的降级逻辑，**不要持久化或当作业务主键**——业务侧检查 `from_server === false` 时建议不入库或单独打标。

极端情况：如果连特征采集本身都失败了（极少见，比如 WASM 启动异常），`visitor_id` 会是空字符串，`match_kind` 仍是 `"offline"`。建议消费侧统一用 `if (!id.from_server || !id.visitor_id) { /* 降级 */ }` 来兜。

## 八、CORS 与鉴权

- 服务对所有 origin 开放，无 CORS 限制；不发 cookie、不依赖跨站 cookie
- 服务端**不校验 `X-API-Key`**——`apiKey` 这个字段唯一的作用是**让 nginx 决定是否绕过限流**，server 接到请求都一视同仁
- **浏览器调用一律不传 `apiKey`**——浏览器藏不住 key（minify 后也是明文），传了等于公开，反而让限流绕过对所有人生效
- 普通浏览器调用受 nginx **30 r/s per-IP** 限流保护，超了返回 429
- `apiKey` 字段仅留给**服务端可信调用方**：他们传对值就绕过 nginx 限流

## 九、版本与升级

- npm 包名：`inf-fingerprint`
- 版本号：CI 自动 stamp，每次 `client/**` 变更都会发布一个新 patch（patch 号用 main 分支累计 commit 数，如 `0.1.27`）
- patch 之间保持向后兼容；major/minor bump（如 `0.2.0`）才会有破坏性变更，会在 README 里单列 changelog

## 十、问题反馈

- GitHub: https://github.com/yomomo-ai/inf-fingerprint/issues
- 钉钉：找 @Gt
