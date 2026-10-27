# 独立插件作者入口

本页面向只取得市场仓库、尚不了解宿主实现的作者。插件应依赖公开协议、SDK 和校验工具；编写插件不要求阅读宿主 React、Tauri 或 Rust 服务的内部实现。市场收录目前支持 Rust + Extism PDK 编译的 Wasm，以及可选的自包含 HTML / CSS / JavaScript 界面。

## 当前能独立完成什么

**当前市场仓库还不是完整的独立开发套件。** 这里已有业务示例、构建打包和测试说明，但完整公开协议与界面 SDK 仍位于宿主仓库；完整离线检查需要宿主提供的 `extension-tool`。宿主源码依赖主要来自工具的构建方式，不表示作者必须理解其实现。

| 材料或操作                                         | 当前提供方式                                                | 作者需要做什么                                                                                                                                         |
| -------------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Rust 插件、HTML 界面、手写 fixture、许可和权限示例 | 本仓库 `plugins/` 与[开发指南](./development.md)            | 按实际业务参考，避免复制无关权限与功能                                                                                                                 |
| 编译插件 Wasm、原生业务单测                        | 本仓库 Rust 工具链与各插件锁文件                            | 安装 Rust 和 Wasm 目标，不需要编译宿主界面                                                                                                             |
| 清单、输入输出、视图、effects 与服务的完整公共约定 | 下表链接的宿主文档与 SDK                                    | 对照与工具一致的宿主提交；不要根据内部实现猜协议                                                                                                       |
| UI 类型声明                                        | 宿主 `sdk/extension-ui.d.ts` 及其 `generated/protocol.d.ts` | 按[宿主 SDK 说明](https://github.com/YanChao001/tauri-desktop-starter/blob/v0.1.7/sdk/README.md)一并取得；市场尚未提供版本化快照                       |
| 清单 / ABI / 视图校验、真实 Wasm fixture           | 配套 `extension-tool` 二进制                                | 当前按[准备宿主校验工具](./development.md#准备宿主校验工具)检出并构建宿主，或由维护者提供同一提交构建的可信工具；尚无本页可指向的预编译 CLI 下载发行物 |
| 本地包安装、授权、真实网络 / 存储 / 原生窗口       | 配套已构建或已安装的原生宿主                                | 从生成包进行交互验收；CLI 和浏览器替身不能证明这些操作成功                                                                                             |

单独 clone 市场后可以编写和编译 Rust 业务、运行原生单测以及脚本 / 界面回归；取得工具前不能完成权威 Wasm 验收与完整 `check`。JS / TS 编译为 Wasm 的宿主示例不属于本市场现有自动构建入口，区别见[语言与界面](./development.md#rustjavascripttypescript-与界面)。

## 作者需要的公开契约

本页公开协议、SDK 和类型链接固定到宿主 `v0.1.7` 标签，以该标签的源码契约为基线，不要求阅读实现文件。源码标签不表示安装包已经公开，下载与安装状态以实际 [Release](https://github.com/YanChao001/tauri-desktop-starter/releases/tag/v0.1.7) 为准。测试更新的宿主源码时，先确认完整 40 位提交 SHA，再同步取得该提交的文档、SDK 和 `extension-tool`，保存材料来源；本地未提交源码不能冒充该 SHA 的已发布内容。最低宿主版本与源码 SHA 的不同职责见[固定配套宿主](./publishing.md#固定配套宿主)。

| 开发问题                       | 公开契约入口                                                                                                                                                                                                                               | 需要核对的内容                                                                                                   |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------- |
| 如何声明与调用文本 / JSON 命令 | [包与输入契约](https://github.com/YanChao001/tauri-desktop-starter/blob/v0.1.7/docs/extensions.md#包与输入契约)                                                                                                                            | 清单字段、导出名、输入 / 结果格式、必需与可选权限、用户主动提交的数据                                            |
| 如何返回标准界面并处理动作     | [应用插件协议](https://github.com/YanChao001/tauri-desktop-starter/blob/v0.1.7/docs/extensions.md#开发带界面的应用插件)                                                                                                                    | `version`、`action`、`state`、`fields`、`locale`、视图字段 / 动作 / 块 / 产物、额度和未知状态拒写                |
| 如何请求网络与存储             | [网络、凭据与私有数据](https://github.com/YanChao001/tauri-desktop-starter/blob/v0.1.7/docs/extensions.md#网络凭据与私有数据)                                                                                                              | HTTP 来源 / 方法 / 代理授权、凭据引用、storage revision、effect ID、`host-result` 回执与失败保留草稿             |
| 如何编写自定义 HTML            | [界面 SDK](https://github.com/YanChao001/tauri-desktop-starter/blob/v0.1.7/sdk/README.md)、[公共类型](https://github.com/YanChao001/tauri-desktop-starter/tree/v0.1.7/sdk)                                                                 | `starter` 方法、snapshot、主题 / 语言 / active、检查点、失败与取消；运行时由宿主注入，无需复制 JS 或访问原生 IPC |
| 如何使用桌面能力               | [受控桌面服务](https://github.com/YanChao001/tauri-desktop-starter/blob/v0.1.7/docs/extensions.md#受控桌面服务)                                                                                                                            | `lan`、`wallpaper`、`resources`、`links`、`window` 的操作、请求 / 响应、权限、窗口归属与资源释放                 |
| 执行有什么限制                 | [执行、存储与生命周期](https://github.com/YanChao001/tauri-desktop-starter/blob/v0.1.7/docs/extensions.md#执行存储与生命周期)、[自定义界面](https://github.com/YanChao001/tauri-desktop-starter/blob/v0.1.7/docs/extensions.md#自定义界面) | 每次新 Wasm 实例、预算 / 队列 / 取消、界面隐藏与卸载、在途副作用和安装代次                                       |
| 如何证明输出符合约定           | 本仓库[真实 Wasm 与手写 fixture](./testing.md#真实-wasm-与手写-fixture)                                                                                                                                                                    | 断言格式、逐条显式输入、真实 worker 校验；合成 host 上下文不会执行真实服务                                       |

公开资料目前仍有缺口：市场没有本地完整协议快照；宿主 UI 类型主要覆盖视图等依赖，`lan` / `wallpaper` 服务仍以未知响应暴露，不能当作完整字段参考。新业务需要的字段、错误码或取消语义在公开资料中不明确时，由维护者补齐权威契约后再开发，不能凭 `unknown`、现有插件偶然使用的字段或内部命令字符串推断支持范围。增加服务操作或权限需要配套宿主实现，不能仅修改插件清单。

## 第一个文本插件

以下示例把用户主动提交的文本转换为大写，保留中文字符，只申请 `text:read`，不联网或持久化。该权限不可省略：真实宿主会拒绝未获文本权限的非空输入；离线 fixture 直接提供输入，不能替代实际授权验收。

从市场仓库根目录手工建立 `examples/author-text/src/`，按下面路径保存五个 UTF-8 文件。该目录由你新建，供本地练习；不要把生成的包或构建输出提交到源码目录。不修改 `listings.json`，也不会进入生产市场构建。实际提交时再按[新增插件](./development.md#新增一个插件)迁入自己的 `plugins/<directory>/`。

`examples/author-text/Cargo.toml`：

```toml
[package]
name = "starter-extism-author-text"
version = "1.0.0"
edition = "2024"
license = "MIT"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
extism-pdk = "=1.4.1"

[profile.release]
opt-level = "s"
lto = true
strip = true
```

`examples/author-text/src/lib.rs`：

```rust
use extism_pdk::*;

#[plugin_fn]
pub fn uppercase(input: String) -> FnResult<String> {
    Ok(input.to_uppercase())
}
```

`examples/author-text/extension.json`：

```json
{
  "formatVersion": 3,
  "apiVersion": 2,
  "minHostVersion": "0.1.7",
  "id": "example.author-text",
  "version": "1.0.0",
  "name": "文本大写",
  "author": "Example author",
  "description": "将用户主动提交的文本转换为大写，保留中文字符。",
  "commands": [
    {
      "id": "uppercase",
      "title": "转换为大写",
      "function": "uppercase",
      "inputFormat": "text",
      "resultFormat": "text",
      "permissions": ["text:read"]
    }
  ],
  "permissions": ["text:read"],
  "optionalPermissions": [],
  "network": []
}
```

`examples/author-text/build.json`：

```json
{ "artifact": "starter_extism_author_text.wasm" }
```

市场自动构建按这个文件找到 Cargo 产物；下方练习使用相同的显式文件名，不修改市场收录清单。

`examples/author-text/smoke.json`：

```json
[
  {
    "command": "uppercase",
    "input": "Hello, plugin!",
    "expectOutput": "HELLO, PLUGIN!"
  },
  {
    "command": "uppercase",
    "input": "你好，Rust!",
    "expectOutput": "你好，RUST!"
  }
]
```

按[环境准备](./development.md#环境与目录)安装工具链与 Wasm 目标，再从**市场仓库根目录**依次执行，前一步成功后再执行下一步：

```sh
cargo generate-lockfile --manifest-path examples/author-text/Cargo.toml
cargo fmt --manifest-path examples/author-text/Cargo.toml -- --check
cargo build --locked --release --target wasm32-unknown-unknown --manifest-path examples/author-text/Cargo.toml --target-dir target/author-text-build
```

保留生成的 `Cargo.lock`；已有插件依赖未变化时不重复生成锁文件。以上只证明插件可编译。按[准备宿主校验工具](./development.md#准备宿主校验工具)设置 `STARTER_EXTENSION_TOOL` 后，将下面内容保存为 `examples/author-text/check.mjs`，仍从市场根目录执行 `node examples/author-text/check.mjs`：

```javascript
import { writeFile } from "node:fs/promises";
import { packagePlugin } from "../../scripts/build.mjs";
import { resolveHostTool, validatePlugin } from "../../scripts/host-tool.mjs";

const checked = await validatePlugin({
  manifestPath: "examples/author-text/extension.json",
  wasmPath:
    "target/author-text-build/wasm32-unknown-unknown/release/starter_extism_author_text.wasm",
  fixturePath: "examples/author-text/smoke.json",
  executable: await resolveHostTool(),
});
const packaged = packagePlugin(
  checked.manifest,
  checked.wasm,
  checked.uiDocuments,
);
await writeFile(
  "target/author-text-build/example.author-text-1.0.0.starter-extension.json",
  packaged.content,
  { flag: "wx" },
);
console.log("真实 Wasm 用例通过，已生成本地安装包。");
```

该入口使用权威工具验证同一份清单 / Wasm 字节快照，再将已检查字节交给市场现有打包函数。输出文件已存在时拒绝覆盖；重新练习时选择新输出文件名或先检查并移走自己的旧练习包。脚本不签名市场目录、不发布 Release。

在配套原生宿主中导入生成的 `.starter-extension.json`，审阅插件与文本权限后启用，输入中英文并执行“转换为大写”。安装、授权、真实输入与结果展示属于单独的交互验收，本页离线命令不代替它。实际业务插件还需补齐原生规则单测、错误处理和适用的[验收范围](./testing.md#按改动选择检查)。

## 完整独立开发套件应如何交付

要让作者仅依赖市场公开资料和下载工具完成开发，后续交付需要补齐以下具体内容；这不是当前已发布能力：

1. 宿主从权威 Rust 类型导出版本化的 SDK、JSON Schema 和公开服务契约，包含清单、Wasm 请求 / 响应、effect / 回执、稳定错误、额度及取消语义。保留必要行为说明，不能只提供类型，也不能在市场手写另一套 DTO 或放宽校验。
2. 市场取得这些生成材料，记录宿主版本、完整源码 SHA 与文件摘要；文档、SDK、CLI、CI 指向同一提交，并在 CI 检查生成漂移。协议演进先更新权威来源，再同步材料，不凭可变 `develop` 链接承诺兼容。
3. 通过可信 Release 分发各平台 `extension-tool`、对应 SDK 和 SHA-256 清单，说明版本、来源与运行依赖；作者取得工具后直接设置 `STARTER_EXTENSION_TOOL`。源码构建保留为维护者与调试路径，不再是作者的必经步骤。
4. 在仅检出市场的干净环境，使用发布的工具和公共资料完成最小插件、fixture、打包及已安装宿主验收，全程不检出或读取宿主源码。服务替身、单机 CLI 通过和真实系统交互分别记录。

这些事项涉及宿主公共契约与跨仓库发行，应随对应实现、版本和配套发布验证，不能通过复制宿主源码、补几条跳转链接或宣称已有预编译包来替代。
