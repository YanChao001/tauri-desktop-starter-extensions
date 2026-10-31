# 插件开发指南

本指南面向向市场提交插件的作者。市场仓库负责插件源码、收录清单和构建；宿主负责权限、视图协议、Wasm 运行与安装。先了解 [架构与边界](./architecture.md)，再按本页准备环境。验证要求见 [测试规范](./testing.md)，上架与签名见 [发布指南](./publishing.md)。

## 首次编写插件

先按 [独立作者入口](./plugin-authoring.md) 取得配套宿主的公开契约、SDK 与 `extension-tool`，完成最小文本插件的编译、真实 Wasm 校验和本地打包。工具的取得边界由该页维护。

最短路径是确定配套宿主提交并准备工具 → 完成最小示例 → 按业务选择文本、通用视图或自定义 HTML → 编写手写 fixture 并验收真实宿主交互 → 按本页新增插件和贡献流程提交。先验证单个插件，再运行完整市场检查；教学示例不加入发布收录清单。

## 环境与目录

市场脚本只使用 Node 内置模块，无需安装 npm 依赖。Node 以 [`package.json`](../package.json) 为准；编译宿主工具时，还须满足宿主 `engines` 并使用其指定 pnpm。两个仓库的 Node 支持范围不同，不直接照搬宿主 `.nvmrc`。

Rust 使用本仓库 [`rust-toolchain.toml`](../rust-toolchain.toml) 钉住的工具链，插件依赖以各自 `Cargo.lock` 为准。宿主工具使用宿主自己的 `rust-toolchain.toml`；不要将宿主的 `RUSTUP_TOOLCHAIN` 导出到市场构建步骤，否则会覆盖市场工具链选择。宿主工具构建还需要对应平台的 Tauri 编译依赖；按 [宿主环境说明](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/development.md#环境要求) 安装。即使工具本身不启动窗口，其所属 Rust crate 仍依赖这些库。

从市场仓库根目录准备插件工具链：

```sh
rustup toolchain install
rustup target add wasm32-unknown-unknown
```

主要目录的职责如下：

| 路径                                           | 用途                                              |
| ---------------------------------------------- | ------------------------------------------------- |
| `plugins/<directory>/src/`                     | 插件逻辑、PDK 导出与规则单测                      |
| `plugins/<directory>/Cargo.toml`、`Cargo.lock` | 插件包元数据和固定依赖                            |
| `plugins/<directory>/extension.json`           | 宿主清单：ID、版本、命令、权限和最低宿主版本      |
| `plugins/<directory>/ui/<id>.html`             | 自定义界面的单文件源码，按命令的 ui.document 引用 |
| `plugins/<directory>/build.json`               | Cargo 编译后的 Wasm 文件名                        |
| `plugins/<directory>/smoke.json`               | 由真实宿主 worker 执行的手写验收                  |
| `listings.json`                                | 收录目录、分类和推荐标记                          |
| `scripts/`                                     | 构建、校验、签名与发布工具                        |
| `target/`                                      | Cargo 编译缓存与 Wasm 模块，不手工修改或提交      |
| `artifacts/market-<revision>/`                 | 本次修订的完整插件包与目录，不手工修改或提交      |

构建、离线检查和生产发布共用市场仓库内的 `artifacts/market-<revision>/`，不再写入 `dist/`，也不向宿主的 `artifacts/` 复制发布包。独立检出与作为子模块检出保持同一布局；宿主运行测试时创建的临时 fixture、报告和截图由宿主测试流程管理。编译缓存保留在 `target/`，清理打包结果无需删除它。最小作者示例的本地包使用 `artifacts/author-text/`，见[独立作者入口](./plugin-authoring.md)。

## 准备宿主校验工具

市场构建需要与当前协议一致的 `extension-tool`。它是宿主提供的专用无界面二进制，支持 `validate` 和 `test`，不初始化桌面应用。不要使用主应用可执行文件代替它，也不要为绕过失败而跳过权威校验。

### 作为宿主子模块开发

市场源码位于宿主的 `marketplace/`。以下 PowerShell 命令从**宿主仓库根目录**运行；执行完后当前目录是市场根目录：

```powershell
pnpm install --frozen-lockfile
pnpm build
cargo build --locked --manifest-path src-tauri/Cargo.toml --bin extension-tool
$env:STARTER_EXTENSION_TOOL = (Resolve-Path src-tauri/target/debug/extension-tool.exe).Path
Set-Location marketplace
rustup toolchain install
rustup target add wasm32-unknown-unknown
```

macOS / Linux 的对应命令同样从**宿主仓库根目录**运行：

```sh
pnpm install --frozen-lockfile
pnpm build
cargo build --locked --manifest-path src-tauri/Cargo.toml --bin extension-tool
export STARTER_EXTENSION_TOOL="$PWD/src-tauri/target/debug/extension-tool"
cd marketplace
rustup toolchain install
rustup target add wasm32-unknown-unknown
```

标准子模块布局会自动探测 `../src-tauri/target/debug/extension-tool(.exe)`；显式设置变量便于确认正在使用哪个宿主。如果设置过 `CARGO_TARGET_DIR`，请改用实际输出位置。宿主协议代码变化后重新构建工具，变量指向旧二进制不会自动更新其行为。

### 独立检出市场仓库

独立 clone 不包含宿主源码或预编译工具。另行检出配套宿主，在其根目录执行上一节的 `pnpm install`、`pnpm build` 与 `cargo build`，再回到市场目录准备本仓库 Rust 工具链和 Wasm 目标。两个检出目录不要求存在父子关系。

通过 `STARTER_EXTENSION_TOOL` 指定工具的**绝对路径**，例如 PowerShell：

```powershell
$env:STARTER_EXTENSION_TOOL = "E:\tools\tauri-desktop-starter\src-tauri\target\debug\extension-tool.exe"
```

macOS / Linux：

```sh
export STARTER_EXTENSION_TOOL="/absolute/path/to/tauri-desktop-starter/src-tauri/target/debug/extension-tool"
```

替换为实际位置，另开终端后重新设置。工具选择规则见 [`scripts/host-tool.mjs`](../scripts/host-tool.mjs)。

## 首次完整离线检查

从**市场仓库根目录**运行以下 PowerShell 命令：

```powershell
$env:MARKET_REVISION = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds().ToString()
node scripts/check.mjs
```

macOS / Linux 从**市场仓库根目录**运行：

```sh
export MARKET_REVISION="$(date -u +%s)"
node scripts/check.mjs
```

`MARKET_REVISION` 必须为正 u32。Unix 秒适合日常生成不重复的本地目录；同一秒重复执行仍会冲突，请生成一个未使用的值。工具不覆盖已有 `artifacts/market-<revision>`，本地修订号不会自动登记为线上最新版本。生产修订递增要求见 [发布指南](./publishing.md#有效期与续签)。

`check` 检查 Rust 格式、按锁文件编译全部 Wasm，通过宿主 `test` 后打包并用一次性内存密钥离线验签。输出为 `catalog-unsigned.json` 和可本地导入的 `.starter-extension.json`，不生成生产 `catalog.json` 或使用生产私钥。具体检查与未覆盖范围见 [测试规范](./testing.md#真实-wasm-与手写-fixture)。

仅需构建和真实 Wasm 校验时也可在设置新修订号后，从**市场仓库根目录**运行 `node scripts/build.mjs`。`check` 已包含该步骤，不要先用相同修订号运行 `build` 再运行 `check`。

## 新增一个插件

当前四个市场插件都使用官方 Extism Rust PDK，分别独立维护 Cargo 清单和锁文件。按业务参考 [计算器](../plugins/scientific-calculator/) 的计算与工作区、[贪吃蛇](../plugins/snake/) 的本地高频循环和日志校验，或 [内网通信](../plugins/lan-chat/) / [壁纸库](../plugins/wallpaper-browser/) 的固定服务调用。复制后先修改身份和说明，不要为不相关的功能顺带复制存储、监听或下载能力。

1. 建立 `plugins/<directory>/`。目录名使用小写字母开头的小写字母、数字和连字符，最长 64 字符；不要复制 `target/` 或 `artifacts/`。
2. 修改 `Cargo.toml` 的包名、版本与许可。保留 `cdylib` 产物类型，新增依赖时审阅 Wasm 支持、许可、体积和维护情况。只为新插件生成或更新自身的 `Cargo.lock`，不要顺带升级其他插件。
3. 修改 `extension.json`。使用独立稳定 ID，版本为数字 `major.minor.patch`；为每条命令选择输入、输出、导出函数和需要的权限，视图命令按需要设置侧栏 / 窗口入口。必需权限与可选权限按实际工作流拆分，不为无关能力申请授权。
4. 修改 `build.json` 的 `artifact`，使其等于 Cargo 输出的 Wasm 基名。例如包名 `starter-extism-my-plugin` 默认对应 `starter_extism_my_plugin.wasm`。这里只允许文件名，不能填写任意路径。
5. 实现逻辑和 PDK 导出，保证导出名与清单的 `function` 一致。参考现有插件将 PDK 导出限制在 `wasm32`，让普通 Rust 单测测试规则函数；每次调用使用传入的 state / fields / host 上下文恢复状态，不依赖实例全局变量长期保留。
6. 手写 `smoke.json`，覆盖每条清单命令，再按实际能力覆盖主要动作、非法输入、拒绝授权与效果回执。fixture 格式与分层边界见 [测试规范](./testing.md#真实-wasm-与手写-fixture)。README 说明用户操作、权限、数据及已知限制；使用存储或导出时说明其语义。
7. 在 [`listings.json`](../listings.json) 增加 `{ "directory": "my-plugin", "category": "tools", "featured": false }`。分类只支持 `text`、`data`、`tools`、`games`；目录和扩展 ID 均不得重复。是否推荐由收录评审决定。

例如，新插件已经建立并修改为 `my-plugin` 后，从**市场仓库根目录**运行：

```sh
cargo generate-lockfile --manifest-path plugins/my-plugin/Cargo.toml
cargo fmt --manifest-path plugins/my-plugin/Cargo.toml
cargo test --locked --manifest-path plugins/my-plugin/Cargo.toml --target-dir target/my-plugin-native
```

`my-plugin` 是需要替换的示例名，不是仓库已有插件。依赖未变化的已有插件不要重复生成锁文件。随后按上一节生成新的 `MARKET_REVISION`，执行完整离线检查；检查通过后，在宿主中手动导入生成的包，验收授权、安装与真实交互。自动检查通过的范围见 [测试规范](./testing.md)。

## Rust、JavaScript、TypeScript 与界面

宿主提供 [JS / TS 编译为 Wasm 的示例](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/extensions.md#构建-javascript-和-typescript-示例)，其执行环境没有 Node 内置模块或浏览器 DOM。市场 `build.mjs` 当前固定调用 Cargo；收录其他语言须先实现受审核的构建流程，并保留相同的宿主验证、fixture、打包与签名步骤。语言与界面的区别见 [架构说明](./architecture.md#语言与-wasm)。

## 自定义界面开发

简单命令不需要 HTML；复杂应用在视图命令中设置 `ui.document: "main"`，并建立 `ui/main.html`。自包含 HTML 内嵌 CSS、JavaScript、数据图片和字体，可使用 DOM / Canvas 或框架生成的单文件。保留键盘、焦点、减少动效、主题和窄屏，不引用 CDN、外部脚本或远程网页。

宿主在作者代码之前注入 SDK，HTML 不需要导入脚本。SDK 提供与宿主一致的主题 token、字体、语言、减少动效和基础控件样式；默认样式可以被作者 CSS 覆写，不必复制宿主组件或引入框架。独立窗口可选择主题和语言，包括跟随宿主语言；消费 snapshot 更新，避免只在初次加载时固定文案或画布颜色。

说明密度、Tooltip / placeholder、操作反馈和动态长文布局遵循 [宿主说明与布局规范](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/conventions/ui-guidelines.md#说明与布局稳定)；操作提示使用 SDK `notify`，其参数和限制见 [SDK](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/sdk/README.md)。

控件增强、原生 select 例外与样式覆写统一见 [SDK 默认外观](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/sdk/README.md#默认外观与动效)。

首屏骨架由宿主真实加载状态控制，SDK 在首个视图、主题与 DOM 呈现就绪后通知展示；作者不手工发送内部握手或人为添加启动延迟。自己的异步业务仍需真实加载 / 失败状态，后续忙碌保持正文可见。宿主工具入口位于左侧当前扩展导航项或独立窗口标题栏，正文只组织业务内容。

通过 SDK 消费状态更新、执行业务动作、提交字段与草稿检查点，并保存产物；失败保留输入，按回执区分完成与取消。方法、类型和示例由 [宿主界面 SDK](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/sdk/README.md) 维护，不另写 DTO。

隔离界面的 sandbox 不含 `allow-forms`；浏览器可能在触发 `onsubmit` 前就阻止表单提交。操作按钮显式使用 `type="button"`，通过点击处理调用业务函数，不依赖 `onsubmit`、`requestSubmit()` 或按钮默认的 submit 行为。需要回车操作时，在对应输入框中按产品语义处理键盘事件、阻止默认提交，再调用同一业务函数；多行输入保留换行或使用明确的组合键。不要为修复按钮无响应而放宽宿主 sandbox。

高频游戏的帧和逻辑步放在界面内，必要时合并检查点，不逐帧调用 Wasm 或原生服务。普通临时游玩可以报告 `dirty: false`；真实未保存编辑用 `dirty: true`。离开暂停、返回手动恢复，并清理 RAF、音频、轮询与监听。检查点导入仍是未知数据，必须重验 schema、范围与可达性。

画布拖动和缩放在本地动画帧中绘制，串行合并最新 fields / checkpoint，业务动作前先提交末次草稿。忙碌时根据新状态协调草稿，不循环重放业务动作；实现可参考 [计算器](../plugins/scientific-calculator/README.md#绘图与科学表达式) 及其界面回归。

本仓库 build 自动读取被引用的 `ui/<id>.html`，检查目录与文件不是链接、UTF-8 和额度，固定同一快照传给原生工具并打包。只修改 HTML 也必须提高插件版本；完整 JSON 包和 UI 文档摘要都会改变。界面重载恢复本窗口字段与检查点，不重新初始化 Wasm；未上报的局部状态无法保留。不同示例见各插件的 `ui/main.html`。

通用 HTTP 和私有 KV 由 Wasm 返回受控 effects，长连接等使用下面的固定服务；供应商 URL、网页解析与来源解释留在插件 Wasm。UI 不使用原生 IPC、Node API 或另建浏览器存储，隔离边界见 [安全规范](./security.md#执行隔离与数据能力)。

## 固定原生服务

`starter.services.request(service, request)` 只接受宿主固定的 `lan`、`wallpaper`、`resources`、`links` 与 `window` 操作。宿主附加安装身份、哈希、代次和原生窗口，重新检查当前启用与授权；插件不能传入另一个 ID、任意命令、窗口 label 或路径。完整字段以配套宿主 SDK 和 Rust 服务为准。

| 服务        | 权限                | 调用要点                                                                                     |
| ----------- | ------------------- | -------------------------------------------------------------------------------------------- |
| `lan`       | `messaging:lan`     | 明确监听、连接与消息 / 附件发送，本机回环使用真实 TCP；导航后连接保留，iframe 卸载只清理轮询 |
| `wallpaper` | `wallpapers:manage` | 复用现有图库与设置；远程导入只接受有效 `resourceId` 和名称                                   |
| `resources` | `network:images`    | `fetch-image` 获取，`preview` / `save` 使用，`release` 释放；不持久化句柄                    |
| `links`     | `links:open`        | 用户请求打开公共 HTTPS 来源页                                                                |
| `window`    | `window:control`    | 读取、切换本插件独立原生窗口的置顶状态，嵌入主界面与浏览器不支持                             |

网络请求按配套宿主的 [来源与授权](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/extensions.md#来源与授权) 继承连接设置，代理授权独立于基础网络授权。壁纸库不在局部界面或检查点维护连接偏好；具体操作见 [网络连接](../plugins/wallpaper-browser/README.md#网络连接)，安全审核见 [安全规范](./security.md#原生服务与外部来源)。

服务是受控宿主能力，不是可插拔通用 IPC。新增方法或权限先完成宿主实现及边界测试，再更新插件；独立 CI 必须能获取对应宿主提交。具体用户操作见 [内网通信说明](../plugins/lan-chat/README.md) 和 [壁纸说明](../plugins/wallpaper-browser/README.md)，信任与许可见 [安全规范](./security.md#原生服务与外部来源)。

状态轮询与标记已读应使用独立在途状态，不把每轮查询当作手动业务忙碌；等待期间昵称、草稿和连接操作保持可用。手动动作后丢弃早先发出的旧查询回执，更新服务状态时保留用户新输入、焦点与选区。实现不应通过整页重建或周期禁用控件来保证互斥。

## 提交前

按 [测试规范](./testing.md) 选择与改动相称的检查，在 PR 中说明实际执行的平台、命令、结果、权限与数据影响。不要把本地离线验签写成线上发布，也不要提交生成包、编译目录、密钥或用户安装数据。

市场是独立仓库：先在市场分支提交、评审和推送，再由宿主更新子模块引用。发布范围内的改动经 PR 合入 `main` 后自动执行生产发布；功能分支和纯文档不触发，手动发布与续签见 [发布指南](./publishing.md)。
