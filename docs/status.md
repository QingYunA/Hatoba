# 实现状态

对照 [`hatoba-spec.md`](hatoba-spec.md) 的需求编号。✅ 已实现并有测试或实测；🟡 已实现、但需在 Windows 实机上验证或尚未对接真实服务；⬜ 未实现。

## 验证方式

| 层 | 验证 |
|---|---|
| `crates/hatoba-core` | 170 个单元 / 集成测试：加密固定向量（与 OpenSSL 交叉核对）、篡改检测、恢复码、保险库生命周期与节流、SQLite 明文扫描、两台设备经模拟服务端的同步与冲突场景、Worker / D1 后端的请求映射 |
| `crates/hatoba-ssh` | 112 个测试，其中 46 个连接真实 OpenSSH `sshd`（`HATOBA_SSH_IT=1`）：密码 / 各类密钥 / 带口令密钥 / PPK、指纹校验、Shell 往返与 50 MB 吞吐、背压、SFTP、端口转发、两级 ProxyJump、ssh-agent、错误分类 |
| `workers/sync` | 161 个 Vitest 测试，运行在本地 D1（miniflare）上：Setup Token、会话、限流、并发冲突、大小限制、改密吊销会话、恢复流程 |
| 客户端 ↔ Worker | `crates/hatoba-core/tests/worker_live.rs`：两台设备通过 `wrangler dev` 运行的真实 Worker 同步（初始化、恢复、双向编辑、冲突、删除、设备列表、吊销）；运行后扫描本地 D1，只有密文 |
| `apps/desktop` | TypeScript 严格模式；tauri-specta 生成绑定并与手写契约做编译期双向校验；前端单元测试；WebDriver 端到端冒烟测试（真实 Rust 后端 + 真实 sshd，见 `apps/desktop/e2e`）；各页面与设计稿逐屏对照（浅色 / 深色） |

## 安全（§4.3）

| 编号 | 状态 | 说明 |
|---|---|---|
| SEC-01 | ✅ | vault_key 与解密条目只在 Rust 内存；锁定时 zeroize |
| SEC-02 | ✅ | 闲置超时（Rust 侧计时，界面上报活动）、系统睡眠（Windows 挂起通知 + 通用时钟跳变检测）、Ctrl+Shift+L |
| SEC-03 | ✅ | 锁定时会话保持并遮罩；设置中可改为锁定即断开 |
| SEC-04 | ✅ | 日志不含秘密与终端内容（端到端测试后扫描日志确认） |
| SEC-05 | ✅ | CSP `default-src 'self'`、无远程内容、最小 capabilities、release 无 devtools、无 shell 插件 |
| SEC-06 | ✅ | 第 4 次起递增延迟（最长 5 分钟），计数持久化，节流期间不运行 Argon2 |
| SEC-07 | 🟡 | Windows Hello：Hello 凭据对固定挑战签名，HKDF 派生包装密钥加密 vault_key，存入凭据管理器；需实机验证 |
| SEC-08 | ✅ | 复制密码由 Rust 直接写剪贴板，30 秒后若未变化则清空 |
| SEC-09 | ✅ | zxcvbn（含通用与英文词典） |
| SEC-10 | ⬜ | Touch ID（P2，随 macOS 版） |

另外：客户端拒绝弱于默认值的 KDF 参数，防止恶意 Worker 降级 Argon2 参数（规格未要求，补充）。

## SSH / 终端 / SFTP / 转发（§7）

| 编号 | 状态 | 说明 |
|---|---|---|
| SSH-01…07 | ✅ | 密码、私钥、每次询问、TOFU 与指纹变化阻断、15 秒超时与分类错误、30 秒 keepalive、断线后手动重连 |
| SSH-08 | ✅ | keyboard-interactive / 2FA |
| SSH-09 | 🟡 | ssh-agent（Windows 命名管道 / `SSH_AUTH_SOCK`）；Windows 需实机验证；Pageant 为 P2 ⬜ |
| SSH-10 | ✅ | 多级 ProxyJump |
| SSH-11 | ✅ | 导入 `~/.ssh/config`（Host、HostName、User、Port、IdentityFile、ProxyJump） |
| SSH-12 | ⬜ | 导入 PuTTY 会话（P2） |
| TERM-01…06 | ✅ / 🟡 | 多标签、256 色 / truecolor、宽字符、PTY 尺寸同步、多行粘贴确认、回滚、字体主题；微软拼音 / 日文输入法的候选框需 Windows 实机验证 |
| TERM-07、08 | ✅ | 终端内搜索、Ctrl+单击打开链接 |
| TERM-09…11 | ⬜ | 分屏、会话日志、Snippets（P2） |
| SFTP-01…04 | ✅ | 文件面板、拖拽上传、下载、进度与取消、重命名 / 删除 / 新建目录、权限大小时间 |
| SFTP-05 | ⬜ | 直接编辑远程文件（P2） |
| FWD-01、02 | ✅ | 本地转发与随连接自动启动 |
| FWD-03、04 | ⬜ | 远程转发、SOCKS（P2） |

## 保险库、主机、密钥（§8）

| 编号 | 状态 | 说明 |
|---|---|---|
| VAULT-01…07 | ✅ | 主密码、恢复码（需回填最后一组）、解锁与节流、自动锁定、改密、恢复码重置、加密备份导出 |
| VAULT-08 | ⬜ | 明文导出（P2） |
| HOST-01…10 | ✅ | 增删改、分组、标签、收藏、模糊搜索（1000 台主机约 1 ms）、最近连接、双击 / 回车连接、密码只可替换、复制主机、TCP 在线探测 |
| KEY-01…06 | ✅ | 导入（OpenSSH / PEM / PPK v2、v3，错误原因明确）、生成 ed25519 / RSA 4096、列表、复制公钥、删除前提示、部署公钥 |
| KEY-07 | ⬜ | 查看私钥（P2；会把私钥交给 WebView，MVP 不提供） |

## 同步（§6）

| 项 | 状态 | 说明 |
|---|---|---|
| Worker 模式 | ✅ | 完整 API、Setup Token、限流、常量时间比较、会话 30 天滑动续期 |
| D1 直连模式 | 🟡 | 已实现并用 SQLite 模拟 REST 接口测试；尚未对接真实 Cloudflare D1 |
| 同步引擎 | ✅ | 增量拉取、批量推送、冲突重试、游标、2 秒防抖、60 秒轮询、聚焦触发、指数退避 |
| 冲突解决 | ✅ | 较新者胜出；密钥冲突保留副本；修改胜过删除；冲突日志可逐条查看与恢复 |
| 流程 A / B | ✅ | 首台设备开启同步、新设备从云端恢复 |
| 流程 C | ⬜ | 合并本地与已初始化的云端（P1）：MVP 提示“请在新设备上选择从云端恢复” |
| 设备管理 | ✅ | 列表（设备名加密存储）、吊销 |
| Deploy to Cloudflare 按钮 | 🟡 | 向导中提供链接；需仓库公开后验证 |

## Windows 适配（§9.1）

| 编号 | 状态 | 说明 |
|---|---|---|
| WIN-01 | 🟡 | 自绘标题栏、拖动、双击最大化、悬停最大化按钮弹出 Snap Layouts（发送 Win+Z）；需实机验证 |
| WIN-02 | ✅ | Segoe UI Variable / Cascadia Mono，中文 YaHei UI、日文 Yu Gothic UI 回退 |
| WIN-03 | 🟡 | Tauri 默认 Per-Monitor DPI；需在 100–200% 实机验证 |
| WIN-04、05 | ✅ | 应用快捷键统一加 Shift；Ctrl+C 有选中则复制、否则发 ^C；右键行为可设置 |
| WIN-06 | ✅ | NSIS 安装包内置 WebView2 引导程序，按用户安装 |
| WIN-07 | 🟡 | Windows 11 Mica，Windows 10 回退纯色 |
| WIN-08 | ✅ | 实时跟随系统深浅色 |
| WIN-09 | ✅ | PuTTY .ppk v2 / v3 |

## 发布（§11）

Authenticode 代码签名与 Tauri updater（P1）尚未配置；推送到 `main` 或手动触发 CI 时，会在 `windows-latest` 上构建未签名的 NSIS 安装包（PR 不构建）。
