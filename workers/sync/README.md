# Hatoba Sync Worker

Hatoba 的云同步服务端：一个运行在**你自己的 Cloudflare 账号**里的 Worker（Hono）加一个 D1 数据库。
它只保存密文，不依赖任何 Hatoba 官方服务器。

> English summary at the bottom of this file.

- 一个部署只服务一个用户（单保险库）。
- 服务端永远拿不到主密码、`vault_key` 或任何明文，见[安全属性](#安全属性)。
- 行为以仓库根目录的 [`docs/hatoba-spec.md`](../../docs/hatoba-spec.md)（§4 安全模型、§5.3 D1 表结构、§6 同步）为准。

---

## 部署

预计 10 分钟。需要：

- 一个 Cloudflare 账号（免费版即可，个人使用通常在免费额度内）
- Node.js **22 或更新**（wrangler 4 的要求）和 npm

以下命令均在本目录（`workers/sync`）执行，Windows PowerShell 与 macOS / Linux 通用。

### 1. 安装依赖并登录

```sh
npm install
npx wrangler login
```

### 2. 创建 D1 数据库

```sh
npx wrangler d1 create hatoba
```

可以用 `--location apac|weur|eeur|oc|wnam|enam` 指定离你近的区域，例如 `npx wrangler d1 create hatoba --location apac`。

命令会打印一个 `database_id`。把它填进 [`wrangler.toml`](./wrangler.toml) 的 `[[d1_databases]]`，替换占位值 `00000000-0000-0000-0000-000000000000`：

```toml
[[d1_databases]]
binding = "DB"                  # 不要改，代码依赖这个名字
database_name = "hatoba"
database_id = "<这里填 wrangler 打印的 id>"
migrations_dir = "migrations"
```

如果 wrangler 询问是否自动把绑定写入配置，选择"否"并手动填写；若选了"是"，请确认配置里没有重复的 `[[d1_databases]]` 段。

### 3. 建表（迁移）

```sh
npx wrangler d1 migrations apply hatoba --remote
```

没有 `--remote` 时只会作用于本地开发数据库。

### 4. 设置 Setup Token

Setup Token 用来防止别人抢在你之前初始化一个刚部署、还没配置的 Worker。它是一个只有你知道的随机字符串，**请用强随机值**：

```sh
# macOS / Linux / Git Bash
openssl rand -base64 32

# 任意系统（只要装了 Node）
node -e "console.log(require('crypto').randomBytes(32).toString('base64url'))"

# Windows PowerShell（不需要 openssl）
$b = New-Object byte[] 32; [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($b); [Convert]::ToBase64String($b)
```

把生成的值存进密码管理器（稍后要在 Hatoba 里粘贴），然后写入 Worker 的 secret：

```sh
npx wrangler secret put SETUP_TOKEN
```

按提示粘贴该值。如果 wrangler 提示"还没有名为 hatoba-sync 的 Worker，是否创建"，回答是。Secret 不会出现在 `wrangler.toml` 或 git 里。

### 5. 部署

```sh
npx wrangler deploy
```

输出里会有 Worker 的地址，形如 `https://hatoba-sync.<你的子域>.workers.dev`。验证部署是否成功：

```sh
curl https://hatoba-sync.<你的子域>.workers.dev/v1/health
# {"service":"hatoba-sync","version":"0.1.0","api":1,"initialized":false}
```

如果返回 `503 database_unavailable`，说明第 2 步的 `database_id` 填错了或第 3 步没有执行。

### 6. 在 Hatoba 里启用同步

1. 打开 Hatoba：**设置 → 云同步**，选择 **Worker 模式**。
2. 填写 **Worker URL**（第 5 步的地址）和 **Setup Token**（第 4 步生成的值），点击"测试连接"。
3. 按向导设置或输入主密码。第一台设备会调用 `/v1/setup` 完成初始化，随后自动登录并推送全部条目。

其他设备加入：在新设备首次启动时选择"从云端恢复"，只需要 Worker URL 和主密码，**不需要** Setup Token。

### 初始化之后（可选加固）

初始化完成后 Setup Token 就不再需要了。你可以删除它，这样 `/v1/setup` 会一直返回 `503 setup_token_not_configured`：

```sh
npx wrangler secret delete SETUP_TOKEN
```

### 升级

```sh
git pull
npm install
npx wrangler d1 migrations apply hatoba --remote   # 只会执行新增的迁移
npx wrangler deploy
```

### 重置（丢弃云端保险库）

只在你确定要清空云端数据（例如初始化时填错了东西）时使用，本地设备上的数据不受影响：

```sh
npx wrangler d1 execute hatoba --remote --command "DELETE FROM sessions; DELETE FROM items; DELETE FROM meta;"
```

### 备份与检查

```sh
npx wrangler d1 export hatoba --remote --output hatoba-backup.sql
```

导出文件里只有密文、KDF 参数和哈希，搜不到任何主机名、密码或私钥。这也是检查"服务端只有密文"的办法。

---

## 安全属性

**服务端保存什么**

| 数据 | 内容 |
|---|---|
| `meta` | `kdf_salt`、`kdf_params`、`SHA-256(auth_key)`、`SHA-256(recovery_auth)`、两个被包装的 vault key（`protected_vault_key`、`recovery_vault_key`，都是 AES-256-GCM 密文）、全局 `seq` |
| `items` | 条目 ID、AES-256-GCM 信封（密文）、revision、seq、删除标记、更新时间 |
| `sessions` | `SHA-256(session_token)`、`device_id`、被 vault_key 加密的设备名、时间戳、scope |

**服务端永远看不到**：主密码、`master_key`、`enc_key`、`vault_key`、恢复码、任何条目明文，以及条目的类型（类型在加密后的明文里）。

**能看到的元数据**：条目数量、ID、密文大小、修改时间、设备数量、登录/同步的时间，以及 Cloudflare 本来就能看到的 IP 地址。

**设计上的保证**

- **Setup Token 防抢注**：没有它，`/v1/setup` 一律拒绝（`401`）；没有配置它时直接 `503`。比较使用常量时间。
- **口令校验值不可逆推主密码**：服务端存的是 `SHA-256(auth_key)`，而 `auth_key` 是 Argon2id 输出经 HKDF 派生的 256 位值。即使 D1 整库泄露，要还原主密码仍然必须逐个尝试 Argon2id。
- **会话 token**：32 字节随机值（base64url），D1 里只存其 SHA-256，30 天有效，使用时滑动续期（写入频率限制为每分钟一次）。每台设备（`device_id`）同时只有一个有效会话。
- **恢复会话受限**：`/v1/recover` 签发的会话只有 15 分钟，且只能调用 `PUT /v1/vault/password`，调用其他任何接口返回 `403`。
- **修改主密码**：在一个 D1 batch（事务）里更新 `meta` 并吊销其他所有会话。用恢复会话改密时，所有会话包括它自己都会被吊销，客户端需要用新密码重新登录。
- **限流**：`/v1/setup`、`/v1/login`、`/v1/recover` 按来源 IP 和接口分别限制为每分钟 10 次（Workers Rate Limiting binding），超出返回 `429`。计数按 Cloudflare 数据中心独立统计。
- **无 CORS**：客户端是原生 Rust 程序，Worker 不返回任何 CORS 头，所以浏览器里的第三方网页无法跨域调用它。
- **不记录秘密**：代码里不会把 token、`auth_key`、信封或请求体写入日志，所有响应带 `Cache-Control: no-store`。

**Worker 或 Cloudflare 账号被攻破时**（对应规格 §4.4）

- 攻击者**无法解密**任何数据，只能拿到密文和 KDF 参数，必须对主密码做 Argon2id 暴力破解。
- 恶意 Worker 可以**删除数据、回滚到旧版本、拒绝服务**。本地副本不受影响；回滚检测列为 P2。
- 恶意 Worker 能在登录时看到 `auth_key`（它本来就要校验这个值），但 `enc_key` 和 `vault_key` 与它是单向派生关系，推不出来。
- 恶意 Worker 还可以在 `/v1/prelogin` 里返回被削弱的 KDF 参数。**所以客户端必须对 KDF 参数设置下限**（算法、内存、迭代次数低于下限时拒绝登录），不能无条件信任服务端返回的值。

**需要知道的限制**

- 持有有效的完整会话 token 就能修改主密码（规格如此设计，不需要旧密码）。设备丢失后请从另一台设备在设备列表里吊销它，并考虑修改主密码。
- 持有恢复码等于持有整个保险库：它既能解出 `vault_key`，也能通过 `/v1/recover` 重设主密码。

---

## API

所有接口都在 `/v1` 下，请求和响应都是 JSON（`Content-Type: application/json`，其他类型返回 `415`）。
需要会话的接口使用 `Authorization: Bearer <session_token>`。

**约定**

- 时间戳一律是 Unix 毫秒（与条目明文里的 `updated_at` 一致），包括 `created_at`、`last_seen`、`expires_at`。
- `auth_key`、`recovery_auth`：32 字节，**标准 base64（带 `=` 填充）**，由客户端发送原始值，服务端存 `SHA-256`（十六进制）。
- `kdf_params`：**JSON 字符串**（客户端把参数对象序列化成字符串发送），服务端原样存储、原样返回，不解释其内容。必须是 JSON 对象，最大 1 KB。
- `kdf_salt`：16–256 个字符的不透明字符串（`A-Za-z0-9+/_=-`），服务端不解码。
- `protected_vault_key`、`recovery_vault_key`、`device_name`、条目 `envelope`：不透明字符串，原样存储。分别最大 4 KB、4 KB、4 KB、64 KB（按 UTF-8 字节计）。
- `device_id`、条目 `id`：`[A-Za-z0-9_-]`，1–64 个字符（UUID 和字面量 `settings` 都符合）。
- 错误响应：`{ "error": "<code>", "message": "..." }`，`message` 可能省略，且永远不会回显你提交的值。

| 方法 | 路径 | 认证 | 说明 |
|---|---|---|---|
| GET | `/v1/health` | 无 | `{ service: "hatoba-sync", version, api: 1, initialized }`；D1 不可用或未迁移时 `503 database_unavailable` |
| GET | `/v1/prelogin` | 无 | `{ kdf_salt, kdf_params }`；未初始化 `404 not_initialized` |
| POST | `/v1/setup` | Setup Token | 初始化；成功 `201 { initialized: true }`，已初始化 `409 already_initialized`，令牌错误 `401 invalid_setup_token`，未配置 `503 setup_token_not_configured` |
| POST | `/v1/login` | 无 | `{ auth_key, device_id, device_name }` → `{ session_token, expires_at }` |
| POST | `/v1/recover` | 无 | `{ recovery_auth, device_id, device_name }` → `{ recovery_vault_key, kdf_salt, kdf_params, session_token, expires_at }`（受限会话，15 分钟） |
| GET | `/v1/vault` | 完整会话 | `{ schema_version, kdf_salt, kdf_params, protected_vault_key, recovery_vault_key, seq }` |
| GET | `/v1/items?since=&limit=` | 完整会话 | `{ items, next_since, has_more }` |
| POST | `/v1/items` | 完整会话 | `{ changes: [...] }` → `{ results: [...] }` |
| PUT | `/v1/vault/password` | 完整或恢复会话 | 修改主密码并吊销会话，成功 `200 { ok, relogin_required }` |
| GET | `/v1/devices` | 完整会话 | `{ devices: [{ device_id, device_name, created_at, last_seen, expires_at, current }] }` |
| DELETE | `/v1/devices/:device_id` | 完整会话 | 吊销该设备的会话，`204`；可以吊销自己；设备不存在时同样返回 `204` |

### POST /v1/setup

```http
POST /v1/setup
Authorization: Bearer <SETUP_TOKEN>
Content-Type: application/json

{
  "schema_version": 1,
  "kdf_salt": "<opaque salt>",
  "kdf_params": "{\"alg\":\"argon2id\",\"v\":1,...}",
  "auth_key": "<base64, 32 bytes>",
  "protected_vault_key": "<envelope>",
  "recovery_vault_key": "<envelope>",
  "recovery_auth": "<base64, 32 bytes>"
}
```

### GET /v1/items

按 `seq` 增量拉取：`SELECT ... WHERE seq > :since ORDER BY seq LIMIT :limit`。`since` 默认 0；`limit` 默认 500，最大 1000（更大的值会被截为 1000）；非法值返回 `400`。

```json
{
  "items": [
    { "id": "0192...", "envelope": "{\"v\":1,...}", "revision": 4, "seq": 1207, "deleted": false, "updated_at": 1790000000000 }
  ],
  "next_since": 1207,
  "has_more": false
}
```

`next_since` 是本页最后一条的 `seq`（空页时等于传入的 `since`）。已删除条目是墓碑：`deleted: true, envelope: null`。

### POST /v1/items

```json
{ "changes": [
  { "id": "0192...", "base_revision": 3, "deleted": false, "envelope": "{\"v\":1,...}", "updated_at": 1790000000000 }
]}
```

- `base_revision = 0` 表示新建；否则必须等于服务端当前 revision（乐观并发）。
- 删除：`deleted: true` 且 `envelope` 为 `null`（或省略）。非删除项的 `envelope` 必须是非空字符串。
- `updated_at` 可选；省略时使用服务器时间。
- 每次请求最多 100 条变更，超出返回 `413 too_many_changes`。同一请求里不能重复出现同一个 `id`。
- 每条变更独立执行一个 D1 batch（事务）。`results` 与请求顺序一致：

```json
{ "results": [
  { "id": "0192...", "status": "ok", "revision": 4, "seq": 1207 },
  { "id": "0193...", "status": "conflict",
    "server": { "revision": 6, "seq": 1190, "deleted": false, "envelope": "...", "updated_at": 1790000000000 } },
  { "id": "0194...", "status": "error", "error": "too_large" },
  { "id": "0195...", "status": "error", "error": "not_found" }
]}
```

| status | 含义 |
|---|---|
| `ok` | 已写入，返回新的 `revision` 和 `seq` |
| `conflict` | `base_revision` 与服务端不一致（含"新建但 ID 已存在"）。`server` 是当前服务端状态（墓碑的 `envelope` 为 `null`），由客户端按规格 §6.4 解决后重试 |
| `error` / `too_large` | 单个信封超过 64 KB。**只影响这一条**，其余变更照常处理，所以一个超大条目不会卡住整个推送队列 |
| `error` / `not_found` | `base_revision > 0` 但服务端没有这个条目（例如数据库被重置）。客户端可把该条目当作新条目（`base_revision = 0`）重新上传 |

`seq` 全局单调递增但允许出现空洞（冲突的尝试也会消耗一个 seq）。

结构性错误（缺字段、类型错误、非法 ID、`deleted` 与 `envelope` 不一致、重复 ID）会让**整个请求**返回 `400`，且不写入任何数据。

### PUT /v1/vault/password

```json
{
  "kdf_salt": "...", "kdf_params": "{...}",
  "auth_key": "<base64, 32 bytes>",
  "protected_vault_key": "<envelope>",
  "recovery_vault_key": "<envelope>", "recovery_auth": "<base64, 32 bytes>"
}
```

`recovery_vault_key` 和 `recovery_auth` 要么同时提供（轮换恢复码），要么都不提供。更新 `meta` 与吊销会话在同一个事务里完成：

- 完整会话：吊销**其他所有**会话，调用者保持登录（`relogin_required: false`）。
- 恢复会话：吊销**所有**会话包括调用者自己（`relogin_required: true`），客户端需要用新密码重新登录。

### 错误码

| HTTP | `error` | 场景 |
|---|---|---|
| 400 | `invalid_request` / `invalid_json` | 字段缺失、类型或格式错误；JSON 无法解析 |
| 401 | `unauthorized` / `invalid_session` | 缺少 token / token 未知或已过期 |
| 401 | `invalid_credentials` | `auth_key` 或 `recovery_auth` 错误 |
| 401 | `invalid_setup_token` | Setup Token 缺失或错误 |
| 403 | `insufficient_scope` | 恢复会话调用了 `PUT /v1/vault/password` 以外的接口 |
| 404 | `not_initialized` / `not_found` | 尚未 setup / 路径不存在 |
| 409 | `already_initialized` | 重复 setup |
| 413 | `too_many_changes` / `payload_too_large` | 超过 100 条变更 / 请求体过大 |
| 415 | `unsupported_media_type` | 请求体不是 JSON |
| 429 | `rate_limited` | 触发限流（带 `Retry-After: 60`） |
| 500 | `internal_error` | 未预期的错误（细节不会返回给客户端） |
| 503 | `setup_token_not_configured` / `database_unavailable` | 未设置 secret / D1 不可用或未迁移 |

---

## 本地开发与测试

```sh
npm run typecheck      # tsc --noEmit
npm test               # Vitest，在本地 workerd + 本地 D1 上运行全部接口测试
```

测试使用 `@cloudflare/vitest-plugin`（`@cloudflare/vitest-pool-workers` 的继任包），每个测试前清空本地 D1，迁移由 `migrations/` 在测试启动时自动应用。覆盖 Setup Token、会话与过期、冲突与并发推送、分页、大小限制、改密吊销、恢复流程、设备管理和限流。

手动调试：

```sh
cp .dev.vars.example .dev.vars        # 里面的 SETUP_TOKEN 只用于本地
npm run db:migrate:local
npm run dev                           # http://localhost:8787
```

> 本地 `wrangler dev` 没有 `CF-Connecting-IP` 请求头，所有请求共用同一个限流计数。

> 如果在添加依赖时 npm 10 报 `Cannot read properties of null (reading 'edgesOut')`，这是 npm 10.x 解析 vitest 可选 peer 依赖时的已知问题，请升级到 npm 11（或使用 pnpm）。仓库里已有 `package-lock.json`，直接 `npm install` / `npm ci` 不受影响。

目录结构：

```
workers/sync/
├── migrations/0001_init.sql   # 规格 §5.3 的表结构（外加 sessions.scope 与索引）
├── src/
│   ├── index.ts               # Hono 应用、错误处理、通用响应头
│   ├── config.ts              # 限额与有效期
│   ├── env.ts                 # 绑定类型
│   ├── errors.ts              # ApiError
│   ├── middleware.ts          # 限流、会话认证、请求体大小限制
│   ├── sessions.ts            # 创建会话
│   ├── util.ts                # base64 / SHA-256 / 常量时间比较
│   ├── validate.ts            # 请求校验
│   └── routes/                # health, setup, auth, vault, items, devices
├── test/                      # Vitest 测试
└── wrangler.toml
```

---

## English summary

**Hatoba Sync Worker** is the optional cloud-sync backend for Hatoba, a desktop SSH client with end-to-end encryption. You deploy it to your own Cloudflare account (Worker + D1). It stores ciphertext only; the master password, vault key and plaintext never reach it.

Deploy (Node.js 22+):

```sh
npm install
npx wrangler login
npx wrangler d1 create hatoba                       # paste database_id into wrangler.toml
npx wrangler d1 migrations apply hatoba --remote
openssl rand -base64 32                             # generate a strong setup token
npx wrangler secret put SETUP_TOKEN
npx wrangler deploy
```

Then in Hatoba open **Settings → Cloud Sync → Worker mode** and paste the Worker URL and the Setup Token. Other devices only need the URL and the master password.

Key properties:

- The setup token stops anyone else from initialising a freshly deployed Worker; it can be deleted once setup is done.
- Only `SHA-256(auth_key)` / `SHA-256(recovery_auth)`, wrapped keys and AES-256-GCM envelopes are stored. A compromised Worker or D1 cannot decrypt anything, but can delete data, roll it back or deny service (spec §4.4). Clients must enforce a floor on the KDF parameters returned by `/v1/prelogin`.
- Sessions are 32-byte random tokens (only their hash is stored), valid for 30 days with sliding renewal. The recovery flow issues a 15-minute session that can only call `PUT /v1/vault/password`.
- `/v1/setup`, `/v1/login` and `/v1/recover` are rate limited to 10 requests per minute per IP and endpoint. No CORS headers are sent.
- Pushes are optimistic-concurrency writes (`base_revision`); conflicts return the current server row. Limits: 100 changes per request, 64 KB per envelope (an oversized envelope fails that single change with `too_large`, not the whole request). All timestamps are Unix milliseconds.

The API reference above applies as-is; error bodies are always `{ "error": "<code>", "message"?: "..." }`.
