# Tauri Starter 扩展市场

由维护者审核、构建和发布的 Extism Wasm 插件目录，配套 [Tauri Desktop Starter](https://github.com/YanChao001/tauri-desktop-starter)。应用读取本仓库 Release 中已签名的 `catalog.json`，核验并下载扩展包；安装、启用与权限由用户确认。源码分支和本地构建不会自动更新线上市场。

**新版四款插件需要本轮 0.1.5 宿主源码的原生构建。** 此轮只发布市场，不同时发布宿主安装包；旧 0.1.4 的权限 / 图标枚举无法识别新目录值时会返回 `MARKET_INVALID`。包内 `minHostVersion` 不能修补已经运行的旧程序，必须重新构建并启动包含新能力的 Rust 宿主，单纯刷新前端无效。配套提交与处理说明见 [固定配套宿主](./docs/publishing.md#固定配套宿主)。

## 已收录插件

| 插件                                                    | 功能                                                             | 权限                     |
| ------------------------------------------------------- | ---------------------------------------------------------------- | ------------------------ |
| [科学计算器](./plugins/scientific-calculator/README.md) | 科学表达式、RAD / DEG、记忆与历史、多函数绘图、求根和数值积分    | 可选私有存储             |
| [贪吃蛇](./plugins/snake/README.md)                     | 方向键 / WASD / 滑动、音效、三档速度、障碍与穿墙、可重放棋局     | 可选私有存储             |
| [局域网信使](./plugins/lan-chat/README.md)              | 手动 IP 连接、联系人、明文文本消息与送达回执、本机历史和会话导出 | 必需 `messaging:lan`     |
| [壁纸库](./plugins/wallpaper-browser/README.md)         | Commons 公共领域 / CC0 浏览、原图另存、本机图库与应用背景管理    | 必需 `wallpapers:manage` |

计算器和贪吃蛇无需授权即可使用，主动保存工作区或纪录才需要私有存储。局域网信使使用独立授权的原生服务，在可信局域网内以明文 TCP 通信；不提供端到端加密、身份认证、云中继或文件传输。壁纸库的必需授权覆盖固定 Commons 来源和现有应用图库，安装审阅时一次确认权限并启用后即可使用；原图下载与压缩图加入图库是不同操作，设置的是应用背景。具体操作、限制和许可见各插件 README 与 `extension.json`。

壁纸网络默认跟随用户已配置的系统代理，也可选择严格直连；界面不能指定代理 URL 或关闭 TLS。代理和直连的 DNS 信任边界不同，详见 [壁纸连接说明](./plugins/wallpaper-browser/README.md#网络连接)；固定来源、许可核验和图像额度始终保留。

## 快速开始

市场可以独立检出，也可作为宿主的 `marketplace/` 子模块工作。Node 要求见 [package.json](./package.json)，Rust 版本见 [rust-toolchain.toml](./rust-toolchain.toml)。市场脚本没有 npm 依赖，以下命令从本仓库根目录运行：

```sh
node --test scripts/*.test.mjs
```

首次完整构建需要先准备宿主的无界面 `extension-tool` 和 Wasm 编译目标，按 [开发指南](./docs/development.md#准备宿主校验工具) 操作。工具就绪后，在 PowerShell 中运行：

```powershell
$env:MARKET_REVISION = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds().ToString()
node scripts/check.mjs
```

`check` 编译全部收录插件、真实运行手写 fixture，并用一次性测试密钥核对目录及包；不使用生产签名密钥，也不发布线上目录。产物位于 `dist/market-<revision>/`，同一修订目录不能覆盖。其他 shell、单插件迭代与验证范围见 [开发指南](./docs/development.md) 和 [测试规范](./docs/testing.md)。

当前四款业务模块均以独立 Cargo 项目构建为真实 Wasm，界面为自包含 HTML / CSS / JavaScript；没有 React / Vite 构建依赖。宿主另有 JS / TS 编译为 Wasm 的示例。科学计算和得分校验由 Wasm 执行，高频游戏绘制留在界面；LAN 长连接与壁纸操作由明确授权的原生服务持有。管理功能位于“扩展工具”，独立窗口另有语言与主题入口。语言、默认样式、HTML 和服务边界见 [架构说明](./docs/architecture.md#插件界面与-html)。

## 文档与贡献

[文档索引](./docs/README.md) · [贡献流程](./CONTRIBUTING.md) · [智能体入口](./AGENTS.md) · [安全规范](./docs/security.md) · [MIT 许可](./LICENSE)

新增或修改插件先按贡献流程准备源码、清单、锁文件、手写测试与收录信息，再提交 PR。宿主协议、工具链和市场目录需要配套；发布、续签、下架及恢复统一见 [发布与管理](./docs/publishing.md)。签名表示维护者发布目录，不认证包内自述作者，也不自动授予权限。
