# Tauri Starter 扩展市场

由维护者审核、构建和发布的 Extism Wasm 插件目录，配套 [Tauri Desktop Starter](https://github.com/YanChao001/tauri-desktop-starter)。应用读取本仓库 Release 中已签名的 `catalog.json`，核验并下载扩展包；安装、启用与权限由用户确认。发布相关改动经 PR 合入 `main` 后自动构建、签名并发布为 latest；纯文档不触发，功能分支和本地构建不会更新线上目录。触发范围与手动续签见 [发布与管理](./docs/publishing.md#actions-生产发布)。

插件最低宿主要求以各自 `extension.json` 为准。开发阶段按选定配套宿主验证，不承诺历史版本兼容；版本选择与发布顺序见 [固定配套宿主](./docs/publishing.md#固定配套宿主)。

## 已收录插件

| 插件                                                | 功能                                                                      | 权限                                   |
| --------------------------------------------------- | ------------------------------------------------------------------------- | -------------------------------------- |
| [计算器](./plugins/scientific-calculator/README.md) | 五种计算模式、十三类转换器、记忆与历史、独立竖屏窗口                      | 可选私有存储、置顶、汇率网络与来源链接 |
| [贪吃蛇](./plugins/snake/README.md)                 | Slither 移植：360°转向、冲刺与成长、离线 AI、皮肤、小地图、原版音效与重放 | 可选私有存储                           |
| [内网通信](./plugins/lan-chat/README.md)            | 聊天式界面、文字 / 表情 / 图片 / 文件、送达回执、会话导出与本机回环测试   | 必需 `messaging:lan`                   |
| [壁纸库](./plugins/wallpaper-browser/README.md)     | 四图源搜索与分类、最多 24 张平铺、原图另存与本机图库管理                  | 必需 `wallpapers:manage`               |

计算器的五种模式与十二类单位转换、贪吃蛇均可离线使用，保存工作区或纪录需要可选私有存储。内网通信为明文 TCP，无远端身份认证，仅适合可信局域网。壁纸库拒绝联网仍可管理本机图库，背景仅作用于本应用，图片许可由原站决定。具体操作、数据与权限用途见各插件 README；网络及来源审核见 [安全规范](./docs/security.md#原生服务与外部来源)。

## 快速开始

首次编写插件按 [独立作者入口](./docs/plugin-authoring.md) 完成最小文本示例，再按 [开发指南](./docs/development.md) 迭代，按 [测试规范](./docs/testing.md#按改动选择检查) 验证改动，通过 [贡献流程](./CONTRIBUTING.md) 提交收录审查。签名发布由维护者从已审核源码执行。

市场可以独立检出，也可作为宿主的 `marketplace/` 子模块工作。Node 要求见 [package.json](./package.json)，Rust 版本见 [rust-toolchain.toml](./rust-toolchain.toml)。市场脚本没有 npm 依赖，以下命令从本仓库根目录运行：

```sh
node --test scripts/*.test.mjs plugins/*/ui/*.test.mjs
```

首次完整构建需要先准备宿主的无界面 `extension-tool` 和 Wasm 编译目标，按 [开发指南](./docs/development.md#准备宿主校验工具) 操作。工具就绪后，在 PowerShell 中运行：

```powershell
$env:MARKET_REVISION = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds().ToString()
node scripts/check.mjs
```

`check` 编译全部收录插件、真实运行手写 fixture，并用一次性测试密钥核对目录及包；不使用生产签名密钥，也不发布线上目录。产物位于 `artifacts/market-<revision>/`，同一修订目录不能覆盖。其他 shell、单插件迭代与验证范围见 [开发指南](./docs/development.md) 和 [测试规范](./docs/testing.md)。

四款插件均使用独立 Rust / Cargo 项目和自包含 HTML / CSS / JavaScript，没有 React / Vite 构建依赖。Wasm、界面与原生服务的分工，以及宿主 JS / TS 示例的适用范围见 [架构说明](./docs/architecture.md)。

## 文档与贡献

[文档索引](./docs/README.md) · [贡献流程](./CONTRIBUTING.md) · [智能体入口](./AGENTS.md) · [安全规范](./docs/security.md) · [MIT 许可](./LICENSE)

新增或修改插件先按贡献流程准备源码、清单、锁文件、手写测试与收录信息，再提交 PR。宿主协议、工具链和市场目录需要配套；发布、续签、下架及恢复统一见 [发布与管理](./docs/publishing.md)。签名表示维护者发布目录，不认证包内自述作者，也不自动授予权限。
