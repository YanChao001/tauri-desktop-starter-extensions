# Tauri Starter 扩展市场

由维护者审核、构建和发布的 Extism Wasm 插件目录，配套 [Tauri Desktop Starter](https://github.com/YanChao001/tauri-desktop-starter)。应用读取本仓库 Release 中已签名的 `catalog.json`，核验并下载扩展包；安装、启用与权限由用户确认。源码分支和本地构建不会自动更新线上市场。

## 已收录插件

| 插件                                         | 功能                                                         | 权限         |
| -------------------------------------------- | ------------------------------------------------------------ | ------------ |
| [文本工具箱](./plugins/text-tools/README.md) | 大小写命令、14 种文本处理、Unicode 统计、TXT / Markdown 导出 | 可选文本读取 |
| [数字合并](./plugins/arcade-2048/README.md)  | 方向键 / 滑动、合并计分、撤销与可复现棋局                    | 可选私有存储 |
| [数织](./plugins/nonogram/README.md)         | 三道 5×5 谜题、行列线索、标记、提示与进度恢复                | 可选私有存储 |
| [故事骰子](./plugins/story-dice/README.md)   | 五槽锁定、题材与种子、历史、草稿、收藏及故事导出             | 可选私有存储 |

文本工坊可以不授权手动输入；快捷大小写命令需要文本读取授权。其余插件不授权存储也可在当前会话游玩，授权后才可保存纪录、进度或收藏。当前四款均不请求网络、凭据或直接文件系统访问；生成文件仍由用户在宿主中选择保存位置。具体规则见各插件 README 与 `extension.json`。

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

当前市场的业务模块使用 Rust / Cargo，宿主另有 JS / TS 编译为 Wasm 的示例。四款市场插件都提供独立的 HTML / CSS / JavaScript 界面，Wasm 负责规则与受控效果；简单工具仍可直接使用文本或通用视图。界面不显示队列预算等宿主开发说明，管理功能统一位于“扩展工具”。语言、界面能力和限制见 [架构说明](./docs/architecture.md#插件界面与-html)。

## 文档与贡献

[文档索引](./docs/README.md) · [贡献流程](./CONTRIBUTING.md) · [智能体入口](./AGENTS.md) · [安全规范](./docs/security.md) · [MIT 许可](./LICENSE)

新增或修改插件先按贡献流程准备源码、清单、锁文件、手写测试与收录信息，再提交 PR。宿主协议、工具链和市场目录需要配套；发布、续签、下架及恢复统一见 [发布与管理](./docs/publishing.md)。签名表示维护者发布目录，不认证包内自述作者，也不自动授予权限。
