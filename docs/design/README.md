# 设计稿

视觉与交互的依据是 Claude Design 项目 `https://claude.ai/design/p/dbb8cd60-198e-4e17-9fe8-4e2dd4c7caf1`。

| 文件 | 说明 |
|---|---|
| `Hatoba.html` | 设计项目的单文件导出，可直接用浏览器打开（含字体与图标资源） |
| `source/Hatoba.dc.html` | 主设计文件：所有页面（浅色 / 深色）与中英文文案表 `T` |
| `source/Sidebar.dc.html`、`source/TabBar.dc.html` | 被主文件引用的侧边栏与标签栏组件 |
| `source/support.js` | 设计画布运行时（只用于渲染设计稿，应用不使用） |

## 移植约定

- 颜色、圆角、尺寸都整理成设计 token：`apps/desktop/src/styles/tokens.css`。浅色取自设计稿根节点变量，深色取自脚本中的 `DARK` 表；组件只引用变量。
- 图标与设计稿相同，使用 Phosphor（`@phosphor-icons/web`，本地打包，不加载远程资源）。
- 文案外置于 `apps/desktop/src/i18n/locales/`，中文与英文沿用设计稿 `T` 表，另补日文。
- Windows 首发版按规格 §9.1 做平台替换：不显示红绿灯按钮，标签栏与标题栏合一并在右侧放置最小化 / 最大化 / 关闭；字体换成 Segoe UI Variable 与 Cascadia Mono；"Touch ID" 对应 "Windows Hello"，"本 Mac" 对应 "本机"。

## 与规格的差异（以规格为准）

设计稿与 `docs/hatoba-spec.md` 冲突时，视觉、布局、文案以设计稿为准；行为、数据、安全以规格为准。实现中据此做了这些调整：

- 加密说明：设计稿写的是 XChaCha20-Poly1305，实际实现为规格 §4 的 AES-256-GCM + Argon2id，文案相应更正。
- Worker 接入：设计稿的"访问令牌 / SYNC_TOKEN"改为规格 §6.2 的 Setup Token（`SETUP_TOKEN`），只在首次初始化时使用。
- 同步冲突：设计稿是"暂停同步、逐项选择"；规格 §6.4 要求自动解决（较新者胜出、密钥不静默丢弃、修改胜过删除）并记录日志，因此冲突页改为逐条查看已自动解决的冲突，可保留当前结果或恢复另一版本。
- 恢复码：规格要求 128 位随机数；为便于抄写和校验，显示为 8 组 × 4 位 Crockford Base32（含 32 位校验）。
- 解锁页页脚不显示主机数量：条目类型不得以明文存储（规格 §4.2），锁定状态下无法统计。
- 查看私钥（KEY-07）为 P2，且会把私钥交给 WebView，MVP 不提供该按钮。
