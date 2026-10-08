# Hatoba

**Hatoba（波止場）** 是一款开源桌面 SSH 客户端：集中管理主机、密钥和终端会话，并通过**你自己的 Cloudflare 账号**（Worker + D1）做端到端加密同步。

- **没有官方服务器**：同步后端部署在你的 Cloudflare 账号里，Hatoba 接触不到你的数据。
- **端到端加密**：所有条目在本机用 AES-256-GCM 加密后才离开设备；Worker、D1 乃至整个 Cloudflare 账号泄露也只会暴露密文。
- **本地优先**：没有网络、没有开启同步时功能完整可用。
- **Windows 优先**：自绘标题栏、Snap Layouts、Mica、Segoe / Cascadia 字体、Windows 输入法；macOS 与 Linux 随后跟进。

> 状态：开发中（0.1）。需求与架构见 [`docs/hatoba-spec.md`](docs/hatoba-spec.md)，逐条实现情况见 [`docs/status.md`](docs/status.md)，视觉设计稿见 [`docs/design/`](docs/design/)。

## 功能

| 模块 | 内容 |
|---|---|
| 主机 | 分组、标签、收藏、模糊搜索、最近连接、在线状态探测、导入 `~/.ssh/config`、ProxyJump 跳板机 |
| 终端 | 多标签、xterm-256color / truecolor、中日文宽字符与输入法、复制粘贴（多行确认）、终端内搜索、可点击链接 |
| 认证 | 密码、私钥（ed25519 / ecdsa / rsa，含口令）、每次询问、ssh-agent、keyboard-interactive（2FA） |
| 安全 | 首次连接指纹确认（TOFU），指纹变化阻断连接；主密码 + Argon2id；恢复码；自动锁定（闲置 / 睡眠 / 手动） |
| SFTP | 终端右侧文件面板，拖拽上传、下载、进度与取消、重命名、删除、新建目录 |
| 密钥库 | 导入（OpenSSH / PEM / PuTTY .ppk）、生成 ed25519 / RSA 4096、复制公钥、部署公钥到主机 |
| 云同步 | Worker 模式（推荐）或 D1 直连；增量同步、冲突自动解决并可逐条查看、设备管理与吊销 |

## 仓库结构

```
apps/desktop/          Tauri 2 桌面应用
  src/                 前端：React + TypeScript + Vite + Zustand + xterm.js
  src-tauri/           Tauri 壳：commands、events、channels、capabilities
crates/hatoba-core/    加密、保险库、数据模型、本地 SQLite、同步引擎（不依赖 Tauri）
crates/hatoba-ssh/     SSH 会话、PTY、SFTP、端口转发、密钥解析（russh）
workers/sync/          Cloudflare Worker（Hono + D1）及部署说明
docs/                  需求文档与设计稿
```

## 开发

需要 Rust stable、Node.js 22+、pnpm 10。Windows 上需要 WebView2（Windows 11 自带）；Linux 需要 `libwebkit2gtk-4.1-dev` 等 Tauri 依赖。

```sh
pnpm install
pnpm tauri dev            # 启动桌面应用
pnpm dev                  # 只启动前端（浏览器中使用内置的模拟后端，带设计稿示例数据）

cargo test --workspace    # Rust 单元测试
pnpm typecheck && pnpm test
```

前端在浏览器中运行时会自动使用 `src/ipc/mock`，可用 URL 参数切换演示状态，例如
`?state=onboarding`、`?state=locked`、`?state=empty`、`?sync=conflict`。

SSH 集成测试使用真实的 OpenSSH 服务器：`HATOBA_SSH_IT=1 cargo test -p hatoba-ssh`（需要已安装 `sshd`）。

TypeScript 绑定由 tauri-specta 从 Rust 生成：`cargo test -p hatoba-desktop export_bindings`；`src/ipc/contract.check.ts` 会在类型检查时比对生成的绑定与前端使用的契约。

端到端冒烟测试（Linux，真实后端 + 临时 sshd）：见 [`apps/desktop/e2e`](apps/desktop/e2e/README.md)。

## 部署同步 Worker

见 [`workers/sync/README.md`](workers/sync/README.md)：`wrangler d1 create` → `wrangler d1 migrations apply` → `wrangler secret put SETUP_TOKEN` → `wrangler deploy`，然后在 Hatoba 的「云同步」中填写 Worker 地址和 Setup Token。

## 安全模型（摘要）

```
主密码 ─Argon2id(64 MiB, t=3, p=4)→ master_key ─HKDF→ enc_key（仅本机） / auth_key（登录 Worker）
vault_key（随机 32 字节）─AES-256-GCM(enc_key)→ protected_vault_key
每个条目 ─AES-256-GCM(vault_key, AAD = hatoba/item/v1/{id})→ 信封 {v, n, c}
```

服务端只保存 `SHA-256(auth_key)` 和密文；条目类型等元数据也不以明文出现。忘记主密码且丢失恢复码时数据无法恢复——这是设计使然。完整说明见规格文档 §4。

## 许可证

[MIT](LICENSE)
