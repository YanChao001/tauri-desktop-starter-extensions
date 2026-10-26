# 插件开发指南

本指南面向向市场提交插件的作者。市场仓库负责插件源码、收录清单和构建；宿主负责权限、视图协议、Wasm 运行与安装。先了解 [架构与边界](./architecture.md)，再按本页准备环境。验证要求见 [测试规范](./testing.md)，上架与签名见 [发布指南](./publishing.md)。

## 环境与目录

市场脚本只使用 Node 内置模块，没有需要安装的 npm 运行依赖。Node 要求以 [`package.json`](../package.json) 为准，目前为 24 或以上；首次还要编译宿主工具，应选择同时满足宿主 `package.json` engines 的 Node 版本，并使用其中指定的 pnpm。不要直接把宿主 `.nvmrc` 的默认版本用于市场：两个仓库的支持范围并不完全相同。

Rust 使用本仓库 [`rust-toolchain.toml`](../rust-toolchain.toml) 钉住的工具链，插件依赖以各自 `Cargo.lock` 为准。宿主工具使用宿主自己的 `rust-toolchain.toml`；不要将宿主的 `RUSTUP_TOOLCHAIN` 导出到市场构建步骤，否则会覆盖市场工具链选择。宿主工具构建还需要对应平台的 Tauri 编译依赖；按 [宿主环境说明](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/development.md#环境要求) 安装。即使工具本身不启动窗口，其所属 Rust crate 仍依赖这些库。

从市场仓库根目录准备插件工具链：

```sh
rustup toolchain install
rustup target add wasm32-unknown-unknown
node --test scripts/*.test.mjs plugins/*/ui/*.test.mjs
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
| `target/`、`dist/`                             | 忽略的编译和打包输出，不手工修改或提交            |

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

独立 clone 不带宿主源码，也不提供预编译宿主工具。准备一个与市场协议匹配的宿主 checkout，按上一节前四步构建工具，再通过 `STARTER_EXTENSION_TOOL` 传递**绝对路径**。两个仓库不要求存在父子关系。

例如，宿主与市场目录位于同一工作目录下，并分别命名为 `tauri-desktop-starter` 和 `tauri-desktop-starter-extensions`。PowerShell 从**市场仓库根目录**执行：

```powershell
Push-Location ../tauri-desktop-starter
pnpm install --frozen-lockfile
pnpm build
cargo build --locked --manifest-path src-tauri/Cargo.toml --bin extension-tool
$env:STARTER_EXTENSION_TOOL = (Resolve-Path src-tauri/target/debug/extension-tool.exe).Path
Pop-Location
rustup toolchain install
rustup target add wasm32-unknown-unknown
```

macOS / Linux 也从**市场仓库根目录**执行：

```sh
market_root="$PWD"
cd ../tauri-desktop-starter
pnpm install --frozen-lockfile
pnpm build
cargo build --locked --manifest-path src-tauri/Cargo.toml --bin extension-tool
export STARTER_EXTENSION_TOOL="$PWD/src-tauri/target/debug/extension-tool"
cd "$market_root"
rustup toolchain install
rustup target add wasm32-unknown-unknown
```

变量只作用于当前 shell 及其子进程。另开终端需要重新设置；Windows 文件名必须为 `extension-tool.exe`，其他平台为 `extension-tool`。具体探测与检查见 [`scripts/host-tool.mjs`](../scripts/host-tool.mjs)。

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

`MARKET_REVISION` 必须为正 u32。Unix 秒适合日常生成不重复的本地目录；同一秒重复执行仍会冲突，请生成一个未使用的值。工具不覆盖已有 `dist/market-<revision>`，也不会把本地修订号自动登记为线上最新版本。生产目录必须满足比线上已发布修订更高的要求，见 [发布指南](./publishing.md)。

`check` 检查所有收录插件的 Rust 格式，按锁文件编译 Wasm，调用宿主 `validate` / `test`，再打包并用一次性内存密钥进行完整离线验签。成功时输出包目录及离线回归通过提示；输出包含 `catalog-unsigned.json` 和可本地导入的 `.starter-extension.json`，不生成可发布的签名 `catalog.json`，不读取生产私钥或修改 `public-key.txt`。

仅需构建和真实 Wasm 校验时也可在设置新修订号后，从**市场仓库根目录**运行 `node scripts/build.mjs`。`check` 已包含该步骤，不要先用相同修订号运行 `build` 再运行 `check`。

## 新增一个插件

当前四个市场插件都使用官方 Extism Rust PDK，分别独立维护 Cargo 清单和锁文件。按业务参考 [科学计算器](../plugins/scientific-calculator/) 的计算与工作区、[贪吃蛇](../plugins/snake/) 的本地高频循环和日志校验，或 [局域网信使](../plugins/lan-chat/) / [壁纸库](../plugins/wallpaper-browser/) 的固定服务调用。复制后先修改身份和说明，不要为不相关的功能顺带复制存储、监听或下载能力。

1. 建立 `plugins/<directory>/`。目录名使用小写字母开头的小写字母、数字和连字符，最长 64 字符；不要复制 `target/` 或 `dist/`。
2. 修改 `Cargo.toml` 的包名、版本与许可。保留 `cdylib` 产物类型，新增依赖时审阅 Wasm 支持、许可、体积和维护情况。只为新插件生成或更新自身的 `Cargo.lock`，不要顺带升级其他插件。
3. 修改 `extension.json`。使用独立稳定 ID，版本为数字 `major.minor.patch`；为每条命令选择输入、输出、导出函数和需要的权限，视图命令按需要设置侧栏 / 窗口入口。必需权限与可选权限按实际工作流拆分，不为无关能力申请授权。
4. 修改 `build.json` 的 `artifact`，使其等于 Cargo 输出的 Wasm 基名。例如包名 `starter-extism-my-plugin` 默认对应 `starter_extism_my_plugin.wasm`。这里只允许文件名，不能填写任意路径。
5. 实现逻辑和 PDK 导出，保证导出名与清单的 `function` 一致。参考现有插件将 PDK 导出限制在 `wasm32`，让普通 Rust 单测测试规则函数；每次调用使用传入的 state / fields / host 上下文恢复状态，不依赖实例全局变量长期保留。
6. 手写 `smoke.json`，覆盖每条清单命令，以及主要动作、拒绝授权、非法输入和效果回执。fixture 格式与分层边界见 [测试规范](./testing.md#真实-wasm-与手写-fixture)。补齐本插件 `README.md` 的玩法、输入额度、权限用途、存储版本、导出语义和已知限制。
7. 在 [`listings.json`](../listings.json) 增加 `{ "directory": "my-plugin", "category": "tools", "featured": false }`。分类只支持 `text`、`data`、`tools`、`games`；目录和扩展 ID 均不得重复。是否推荐由收录评审决定。

例如，新插件已经建立并修改为 `my-plugin` 后，从**市场仓库根目录**运行：

```sh
cargo generate-lockfile --manifest-path plugins/my-plugin/Cargo.toml
cargo fmt --manifest-path plugins/my-plugin/Cargo.toml
cargo test --locked --manifest-path plugins/my-plugin/Cargo.toml --target-dir target/my-plugin-native
```

`my-plugin` 是需要替换的示例名，不是仓库已有插件。依赖未变化的已有插件不要重复生成锁文件。随后按上一节生成新的 `MARKET_REVISION`，执行完整离线检查；检查通过后，在宿主中手动导入生成的包，验收授权、安装与真实交互。自动检查通过的范围见 [测试规范](./testing.md)。

## Rust、JavaScript、TypeScript 与界面

宿主并不限于 Rust：其开发示例同时提供 JavaScript 和 TypeScript 经 Extism JS PDK 编译为 Wasm 的路线，参见 [宿主 JS / TS 构建说明](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/extensions.md#构建-javascript-和-typescript-示例)。这些是宿主仓库的示例；市场当前的统一 `build.mjs` 固定调用 Cargo，仅直接构建 Rust 插件，不能只在收录清单填入一个 `.js` 文件。

JS / TS 插件运行于 Wasm 中的 JavaScript 环境，不是 Node.js 进程或浏览器页面。不要使用 Node 内置模块、DOM 或自行调用宿主 IPC 来获取能力。网络、持久化和用户文件等交给声明式请求及宿主授权处理。需要把 JS / TS 插件正式纳入市场时，需同时扩展受审查的构建流程并保留同样的宿主验证、fixture、打包和签名步骤。

## 自定义界面开发

简单命令不需要 HTML；复杂应用在视图命令 ui 中设置 document，例如 main，并建立 `ui/main.html`。自包含 HTML 内嵌 CSS、JavaScript、数据图片和字体；可用普通 DOM / Canvas，也可让框架构建为单文件。保留键盘、焦点、减少动效、主题和窄屏；不要引用 CDN、外部脚本或远程网页。

宿主在作者代码之前注入 SDK，HTML 不需要导入脚本。SDK 提供与宿主一致的主题 token、字体、语言、减少动效和基础控件样式；默认样式可以被作者 CSS 覆写，不必复制宿主组件或引入框架。独立窗口可选择主题和语言，包括跟随宿主语言；消费 snapshot 更新，避免只在初次加载时固定文案或画布颜色。

说明密度、Tooltip / placeholder、操作反馈和动态长文布局遵循 [宿主说明与布局规范](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/conventions/ui-guidelines.md#说明与布局稳定)；操作提示使用 SDK `notify`，其参数和限制见 [SDK](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/sdk/README.md)。

普通单选 select 由 SDK 渐进增强，仍读写原节点的值并使用原事件处理；需要原生交互时声明 `data-starter-select="native"`。支持范围、表单语义和覆写统一见 [SDK 默认外观](https://github.com/YanChao001/tauri-desktop-starter/blob/feature/workflows-and-extensions/sdk/README.md#默认外观与动效)，不要另写原型补丁或复制宿主控件实现。

首屏骨架由宿主真实加载状态控制，SDK 在首个视图、主题与 DOM 呈现就绪后通知展示；作者不手工发送内部握手或人为添加启动延迟。自己的异步业务仍需真实加载 / 失败状态，后续忙碌保持正文可见。宿主工具入口位于左侧当前扩展导航项或独立窗口标题栏，正文只组织业务内容。

`starter.onChange` 接收当前 view、fields、checkpoint、忙碌、语言和主题；`starter.run` 请求本命令动作，`starter.setFields` 只更新字段，`starter.checkpoint` 报告草稿及是否未保存，`starter.saveArtifact` 请求保存已生成产物。异步失败保留输入并显示可理解的反馈，保存取消返回 false，不能标成已保存。完整类型与示例见 [宿主界面 SDK](https://github.com/YanChao001/tauri-desktop-starter/tree/feature/workflows-and-extensions/sdk)，不要复制一套 DTO。

隔离界面的 sandbox 不含 `allow-forms`；浏览器可能在触发 `onsubmit` 前就阻止表单提交。操作按钮显式使用 `type="button"`，通过点击处理调用业务函数，不依赖 `onsubmit`、`requestSubmit()` 或按钮默认的 submit 行为。需要回车操作时，在对应输入框中按产品语义处理键盘事件、阻止默认提交，再调用同一业务函数；多行输入保留换行或使用明确的组合键。不要为修复按钮无响应而放宽宿主 sandbox。

高频游戏的帧和逻辑步放在界面内，必要时合并检查点，不逐帧调用 Wasm 或原生服务。普通临时游玩可以报告 `dirty: false`；真实未保存编辑用 `dirty: true`。离开暂停、返回手动恢复，并清理 RAF、音频、轮询与监听。检查点导入仍是未知数据，必须重验 schema、范围与可达性。

画布拖动和缩放同样在本地动画帧中绘制，按有界间隔合并最新 fields / checkpoint，并等待上一批完成，不能为每个 pointermove 连续调用 SDK。科学计算器按 250 ms 合并草稿，保留现有通道频率预算；开始业务动作前提交最新字段，避免旧回执覆盖新视口。幂等草稿写入遇到宿主忙碌冲突时，等待新的空闲状态再对同一草稿恢复一次；区分早到的拒绝与空闲后才到的回执，不用旧空闲快照反复抢跑，也不自动重放业务动作或重试一般失败。

本仓库 build 自动读取被引用的 `ui/<id>.html`，检查目录与文件不是链接、UTF-8 和额度，固定同一快照传给原生工具并打包。只修改 HTML 也必须提高插件版本；完整 JSON 包和 UI 文档摘要都会改变。界面重载恢复本窗口字段与检查点，不重新初始化 Wasm；未上报的局部状态无法保留。不同示例见各插件的 `ui/main.html`。

UI 与 Wasm 是不同上下文。界面不会获得宿主 DOM、可用 Tauri IPC、任意路径或凭据原文。通用 HTTP 和私有 KV 由 Wasm 返回受控 effects；LAN 长连接、短期图片资源、现有图库与外部链接使用下面的有限原生服务。供应商 URL、网页解析与结果来源解释在插件 Wasm 中实现，不向宿主追加具体网站业务。界面脚本不要使用 Node API 或另建浏览器存储。路线选择与边界见 [插件界面与 HTML](./architecture.md#插件界面与-html)。

## 固定原生服务

`starter.services.request(service, request)` 只接受宿主固定的 `lan`、`wallpaper`、`resources` 与 `links` 操作。宿主附加安装身份、哈希、代次和原生窗口，重新检查当前启用与授权；插件不能传入另一个 ID、任意命令或路径。完整字段以配套宿主 SDK 和 Rust 服务为准。

- **LAN** 使用 `messaging:lan`，信使将其作为必需权限。用户明确开启监听或输入数字私有 / 回环 IP 连接；不会自动发现公网设备、扫描网络或在导入会话时重连。连接属于原生窗口，导航后保持；界面仅在可见时读取状态，不能在 iframe 清理时误停整个服务。实际停止、最后服务窗口关闭、撤权、替换和退出由宿主回收。
- **图库** 使用 `wallpapers:manage`，仅支持 `list`、`import` 本机选择、`import-resource`、`apply`、`rename`、`remove` 和 `open-settings`，复用应用现有图库与设置。`import-resource` 接受本窗口有效的 `resourceId` 与显示名称；不接受 URL、`resultId` 或 `pageId`，没有供应商搜索接口。
- **图片资源** 使用可选 `network:images`，以 `fetch-image` 的公共 HTTPS URL 获取资源，返回 `resourceId`、MIME、字节数、尺寸和哈希。`preview` 生成安全 data URL，`save` 经系统对话框另存已验证原图，`release` 主动释放；收到取消不能标记已保存。句柄短期有效且绑定当前安装与窗口，关闭 / 撤权清理，不把它当长期存储或会话恢复资格。请求 URL 的查询参数同样可发送数据，权限用途需要明确说明。
- **外部链接** 使用可选 `links:open`，由 `links` 服务提交公共 HTTPS URL 并通过系统浏览器打开；不开放任意协议、路径或系统命令。

壁纸库的必应搜索 URL、HTML 解析、查询匹配及来源说明由插件 Wasm 负责；HTTP effect 使用可选 `network:https` 与清单中的 `https://cn.bing.com` GET 声明。只需要本机图库时，不授予这些网络或外链权限仍能使用。图片资源与普通 HTTP 是不同授权，图库权限不能替代它们。

HTTP 请求与 `resources` 的 `fetch-image` 可选 `networkMode: "direct" | "system"`，默认 `direct`。`system` 还需独立授予 `network:system-proxy`，不能用于本机来源；代理由系统 / 环境配置选择，插件不提交代理地址。公共 HTTPS 直连执行全部 DNS 公网校验并固定已核验地址；独立本机来源仍遵循 `network:local`。代理远端解析无法证明最终 IP，具体边界见 [安全规范](./security.md#原生服务与外部来源)。

服务是受控宿主能力，不是可插拔通用 IPC。新增方法或权限先完成宿主实现及边界测试，再更新插件；独立 CI 必须能获取对应宿主提交。具体用户操作见 [信使说明](../plugins/lan-chat/README.md) 和 [壁纸说明](../plugins/wallpaper-browser/README.md)，信任与许可见 [安全规范](./security.md#原生服务与外部来源)。

状态轮询与标记已读应使用独立在途状态，不把每轮查询当作手动业务忙碌；等待期间昵称、草稿和连接操作保持可用。手动动作后丢弃早先发出的旧查询回执，更新服务状态时保留用户新输入、焦点与选区。实现不应通过整页重建或周期禁用控件来保证互斥。

## 提交前

按 [测试规范](./testing.md) 选择与改动相称的检查，在 PR 中说明实际执行的平台、命令、结果、权限与数据影响。不要把本地离线验签写成线上发布，也不要提交生成包、编译目录、密钥或用户安装数据。

市场是独立仓库：先在市场分支提交、评审和推送，再由宿主更新子模块引用。普通开发提交不会更新应用读取的 Release，发布需单独按 [发布指南](./publishing.md) 执行。
