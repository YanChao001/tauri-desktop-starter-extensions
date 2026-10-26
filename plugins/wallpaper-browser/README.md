# 壁纸库

必应图片搜索、本机图库与应用背景管理。插件的 Wasm 负责搜索 URL、网页解析、结果筛选和会话状态，自定义 HTML 负责界面；宿主提供通用 HTTP、图片资源、外链和已有图库能力，不包含必应业务。版本 1.0.3 需要重新构建并重启配套 **0.1.6 原生宿主**。

## 功能与授权

`wallpapers:manage` 是本机图库的必需权限，允许列出、通过用户对话框导入、重命名、移除图片、设置应用背景及打开图库设置。这里设置的是应用背景，不是操作系统桌面壁纸。拒绝所有可选网络权限时，本机功能仍可使用。

其他能力分别可选授权：

| 权限                   | 用途                                                                    |
| ---------------------- | ----------------------------------------------------------------------- |
| `network:https`        | Wasm 通过 HTTP effect 访问清单声明的 `https://cn.bing.com` GET 搜索端点 |
| `network:images`       | 通过通用资源服务下载公网 HTTPS 缩略图或原图，预览、另存及加入图库       |
| `links:open`           | 用户点击后，在外部浏览器打开图片来源页，核对来源与使用许可              |
| `network:system-proxy` | 对上述已获授权的网络请求选择系统代理；不额外授予搜索或图片网络权限      |

界面按具体授权启用入口并说明限制。仅授予系统代理不能搜索或下载图片；首次打开时，已获代理授权则选择系统代理，否则直接连接，合法且仍有授权的检查点偏好优先恢复。关闭权限不会以空结果或保存成功冒充操作已完成。

## 搜索、预览与资源

用户点击搜索后，关键词原文发送至 `https://cn.bing.com/images/search`。Wasm 核对返回搜索框和每个结果链接的关键词，按原始排序接收至多 12 个符合 HTTPS 来源规则的结果。无结果元素、验证码、网页改版和错误页面分别处理，不回退成无关结果。**不按 Public domain / CC0 或其他许可筛选，也不保证图片使用许可**；来源页供用户自行核对。

搜索通过现有 HTTP effect 和对应 `host-result` 回执完成，不使用原生壁纸服务搜索。`action: null` 只按当前语言重绘；失败或切换语言保留此前结果和未提交输入，不自动重发网络请求。结果元数据保存在有界插件 state，插件生成的结果标识只用于列表身份，不能代替原生授权或网络校验。插件会验证恢复状态，不保留图片二进制、路径或原生资源 ID。

预览使用必应缩略图，下载使用结果中的第三方 HTTPS 原图。界面调用 `starter.services.request("resources", { action: "fetch-image", url, networkMode })`，取得绑定当前安装身份和原生窗口的短期不透明 `resourceId`，再用该句柄预览、另存或导入图库。资源 10 分钟过期，关闭窗口、撤权或替换安装会清理；每条预览、下载与加入图库流程均在 `finally` 中释放资源，失败、取消和晚到响应也不把资源带入会话或检查点。资源句柄不证明图片来自必应，来源解释仍由插件负责。

- **下载原图**：资源服务验证 MIME、真实解码与图片额度，通过原生另存对话框保存原始文件。取消保存明确返回取消，不返回系统路径。
- **加入图库**：把通用图片资源交给图库 `import-resource`；宿主重绘压缩为现有图库格式，保留最多 24 张及单图额度。压缩副本不作为原始文件另存。
- **应用背景**：选择图库图片及覆盖 / 完整显示方式，使用现有设置 revision CAS 保存并同步多窗口；冲突时刷新后重新选择。
- **本机管理**：本地图片通过用户选择导入；移除当前使用图片前要求确认，并恢复纯色背景，不删除用户原始文件。

远程图片不会直接作为 iframe 的远程 `img` 加载。资源服务返回受控的小型重绘 data URL；HTTPS、DNS / SSRF、TLS、禁止重定向、MIME、真实解码及原图 16 MiB / 每边 8192 像素 / 1600 万像素额度仍由宿主核验。插件的 URL 筛选只是业务规则，不取代原生边界。

## 网络连接

宿主 HTTP effect 与图片获取在省略 `networkMode` 时默认 `direct`；界面会按上述授权和检查点规则显式选择模式。直接连接禁止代理，宿主核验全部解析地址并固定到本次连接，拒绝私网、环回及保留网段。VPN Fake-IP 等合成保留地址可能因此被拒绝。

系统代理需要独立授权；沿用宿主支持的用户系统配置和代理排除规则，插件不能指定代理 URL、任意请求头、关闭 TLS 或绕过来源声明。命中系统代理时，宿主不能核验代理远端最终解析的目标 IP；没有匹配代理或被排除而直连时仍执行 DNS 检查。授权系统代理表示接受该信任边界，不承诺任意 PAC 支持。

## 开发与验证

从市场仓库根目录运行：

```sh
cargo test --locked --manifest-path plugins/wallpaper-browser/Cargo.toml --target-dir target/wallpaper-browser-native
cargo clippy --locked --manifest-path plugins/wallpaper-browser/Cargo.toml --target-dir target/wallpaper-browser-native --all-targets -- -D warnings
cargo build --locked --release --target wasm32-unknown-unknown --manifest-path plugins/wallpaper-browser/Cargo.toml --target-dir target/wallpaper-browser
node --test plugins/wallpaper-browser/ui/main.test.mjs
```

依赖 `serde`、`regex`、`url` 固定复用配套宿主锁定版本，分别用于有界状态编码、网页属性解析和 URL 规范化；均为 MIT / Apache-2.0 许可的纯 Rust 依赖，支持本插件的原生测试和 Wasm 目标，不新增系统库或网络运行时。包体以实际构建产物为准，不预估未经测量的数字。

纯解析回归从宿主迁入插件，包含查询一致、真实空结果、无效重复项不屏蔽有效结果、HTML 注释 / 属性 / 原始文本健壮性与 HTTPS 来源规则。`smoke.json` 验证真实 Wasm 启动、搜索 effect、成功及失败回执、权限拒绝和不匹配回执；完整校验按[开发指南](../../docs/development.md)准备配套宿主工具后执行。资源授权、URL / DNS、解码额度、句柄生命周期、另存取消和图库 CAS 由宿主边界测试负责；界面测试使用 DOM / SDK 替身。离线 fixture 和替身均不代表真实必应、图片原站、WebView 或系统对话框已验收。

本轮移除旧原生必应结果缓存和 Commons `pageId` 操作，不保留开发期的业务兼容服务；旧包需升级。现有图库图片、应用偏好和设置不迁移、不删除。会话文件继续绑定插件及 UI 文档版本，旧会话不能冒充新版业务状态。
