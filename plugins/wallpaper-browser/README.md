# 壁纸库

可操作的本机图库与开放许可壁纸浏览器。自定义 HTML 提供搜索、来源说明、预览和图库管理，Wasm 仅初始化受验证的视图，实际系统操作通过 `starter.services.request("wallpaper", request)` 完成。

## 功能与授权

浏览与管理共用必需权限 `wallpapers:manage`。安装审阅时确认用途、授予该权限并启用，即可列出和导入本机图片、搜索与下载 Commons 图片、重命名、移除、设置宿主应用背景，并打开宿主设置。界面中的缺少授权提示用于旧会话或授权变化时的防御处理。图片只通过原生对话框导入或另存，插件不能提供系统路径。这里的背景是应用背景，不是操作系统桌面壁纸。

远程源固定为 Wikimedia Commons 官方 Action API，搜索不自动发起；只有用户搜索或选择预览、下载时才访问网络。每次预览、导入和下载均重新查询文件许可，仅接收明确标为 **Public domain / CC0** 的 JPEG、PNG、WebP，展示来源页、作者、许可和原始尺寸。未识别或需额外署名条款的其他许可证不在本插件下载范围。

- **下载原图**：验证 MIME、真实解码和尺寸后，通过原生另存对话框保存原始文件，最多 16 MiB、8192 像素每边、1600 万像素。取消保存会明确返回取消。
- **加入图库**：下载服务端预览图，原生重绘并压缩为现有图库支持的 JPEG；受当前图库 24 张及每张数据额度限制。这不是原图下载。
- **应用背景**：选择图库图片与覆盖 / 完整显示方式，使用设置 revision CAS 保存并同步多窗口；冲突时刷新后重新选择。
- **本机管理**：本地图库在断网时仍可使用。移除当前应用背景前要求用户确认，同时恢复纯色背景。

远程图片不会作为 iframe 的远程 `img` 加载；宿主只返回小型重绘 data URL。下载源限制到 Commons 官方域名，不跟随重定向，不接受任意 URL。来源许可由 Commons 文件页提供，用户在其他用途发布图片前仍应核对来源页。

## 网络连接

默认“跟随系统代理”，沿用锁定 reqwest 版本支持的系统配置及 `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY`、`NO_PROXY` 环境规则；没有配置匹配代理或被排除时直接连接。插件只能选择跟随系统或直接连接，不能自行指定代理地址、请求头、目标域名或关闭证书检查。用户代理可能为本机服务，这由系统网络设置决定，不代表插件获得任意本机网络权限。

所有请求仍要求 HTTPS、精确的 Commons API / 原图 / 缩略图来源与路径、正常 TLS 证书验证，以及既有响应体积、MIME、解码和尺寸限制。实际走直连时，由受限 DNS resolver 验证全部目标地址是公网地址，并将同一组已验证地址交给连接器；代理连接由用户代理解析目标，应用通过固定域名和 TLS 证书约束来源，不声称能校验代理远端最终解析到的 IP。这个取舍仅适用于本服务的固定 provider，普通插件 HTTP broker 不受影响。

“直接连接”明确禁用代理，保留所有公网 DNS 检查。若 VPN 将目标解析为 `198.18.0.0/15` 等 Fake-IP / 保留网段，直接连接会拒绝；选择已正确配置的系统代理，或为 Wikimedia 配置真实公网 DNS。系统代理与排除规则由当前平台及 reqwest 实际支持情况决定，不承诺执行任意 PAC 脚本。

## 开发与验证

```sh
cargo test --manifest-path plugins/wallpaper-browser/Cargo.toml --locked
```

在市场仓库根目录按[开发指南](../../docs/development.md)设置配套宿主工具，运行完整市场构建。`smoke.json` 验收真实 Wasm 初始化且无隐式副作用；来源、入参、尺寸、重绘、设置和图库连接由宿主隔离单元测试验证。测试不修改用户壁纸、真实图库或设备设置。原生对话框、Commons 连通性和真实 WebView 交互需单独验收。

接口和许可的一手资料：[MediaWiki Imageinfo](https://www.mediawiki.org/wiki/API:Imageinfo)、[MediaWiki Search](https://www.mediawiki.org/wiki/API:Search)、[Commons 再利用规范](https://commons.wikimedia.org/wiki/Commons:Reusing_content_outside_Wikimedia)。
