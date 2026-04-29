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

`init()` 必须在第一次调用前执行一次，加载 WASM 模块（约 140KB gz，浏览器自动缓存）。后续整个 tab 内复用，无需重复 `init`。

## 三、返回字段

| 字段 | 类型 | 说明 |
|---|---|---|
| `visitor_id` | string (UUID) | 稳定的访客 ID，**这是你要存的东西** |
| `match_kind` | `"exact"` / `"fuzzy"` / `"ambiguous"` / `"new"` / `"offline"` | 匹配置信度，见下表 |
| `score` | number | 贝叶斯打分（log-likelihood），调试用 |
| `second_score` | number | 第二高候选的分数，越接近 `score` 越可疑 |
| `observation_count` | number | 该访客累计被观测过多少次 |
| `via_persistence` | boolean | 是否走的 localStorage/cookie 快速路径（不是靠特征匹配） |
| `from_server` | boolean | `false` 表示 server 不可达，走了本地兜底 |
| `cached` | boolean | 本次结果是否来自浏览器本地缓存 |
| `stale` | boolean | 缓存命中但已过 SWR 阈值，后台正在异步刷新 |

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
  apiKey?: string,            // 可选，目前未启用，留空即可
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

所以**正常情况下首屏只有第一次访问会有约 100ms 网络延迟**，之后几乎零开销。`forceRefresh: true` 可强制重新识别，慎用。

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
{ match_kind: "offline", from_server: false, visitor_id: "<本地哈希>" }
```

本地兜底 ID 是用浏览器特征算的快速哈希，**与 server 端 visitor_id 不在同一空间**，仅用于"页面能继续跑"的兜底，不要持久化或当作业务主键。建议业务侧检查 `from_server: false` 时不入库或单独打标。

## 八、CORS 与鉴权

- 服务对所有 origin 开放，无 CORS 限制
- 当前**不要求 apiKey**（浏览器藏不住 key），靠 nginx 30r/s per-IP 限流兜底
- 不发 cookie，不依赖跨站 cookie

## 九、版本与升级

- npm 包名：`inf-fingerprint`
- 版本号：CI 自动 stamp，每次 `client/**` 变更都会发布一个新 patch（如 `0.1.27`）
- 升级直接 `npm update inf-fingerprint`，不会有破坏性变更直到 `0.2.x`

## 十、问题反馈

- GitHub: https://github.com/yomomo-ai/inf-fingerprint/issues
- 钉钉：找 @Gt
