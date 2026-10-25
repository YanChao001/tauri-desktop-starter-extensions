# 测试与验收

本页定义市场插件和发布工具的验证范围。环境与宿主 `extension-tool` 的准备见 [开发指南](./development.md)，目录签名与上线见 [发布指南](./publishing.md)。构建、离线通过、CI 成功和线上发布是不同证据，交付时分别说明。

## 按改动选择检查

| 修改范围                             | 必做检查                                                     | 仍需单独验收                                      |
| ------------------------------------ | ------------------------------------------------------------ | ------------------------------------------------- |
| 文档、示例命令                       | 对照实际脚本、路径和 CLI 参数；检查相关 Markdown 格式及链接  | 平台特有命令未执行时明确说明                      |
| 市场脚本、目录协议、签名或发布流程   | `node --test scripts/*.test.mjs`；涉及产物时执行完整 `check` | 生产 Secret、GitHub 权限和真实发布结果            |
| 插件规则、state / fields 或 PDK 导出 | 插件原生规则单测、格式检查、真实 Wasm `validate` / `test`    | 实际宿主交互与边界                                |
| 清单、权限、视图、导出产物           | 完整 `check`，覆盖所有命令的手写 fixture                     | 安装、可选授权、中文 / 英文、窄屏、键盘与保存流程 |
| 私有存储、并发或宿主 effects         | 规则单测与成功 / 失败回执 fixture；宿主对应 broker 服务回归  | 多窗口 CAS、取消、重开及持久化恢复                |
| 宿主 API / ABI 协议联动              | 重新构建指定宿主工具后完整 `check`，同步宿主相关测试         | 目标平台桌面与正式安装环境                        |

按需运行，已有检查通过后，除非继续修改、失败或仍有明确疑点，不必反复执行所有编译。市场 `check` 会检查格式、构建和真实 Wasm，不会自动执行每个插件的原生 Rust 单测；两层都需要覆盖相关变更。

## 脚本与规则单测

从**市场仓库根目录**运行市场脚本回归：

```sh
node --test scripts/*.test.mjs
```

这些测试覆盖签名、包完整性、清单 / 目录规则、发布来源以及构建工具调用约束。它们不需要生产密钥，也不等同于编译并真实运行四个插件。

规则函数优先使用原生 Rust 单测，以便快速检查 Unicode、随机序列、合并或解谜规则、状态重放、撤销、额度和冲突处理。从**市场仓库根目录**分别运行受影响插件：

```sh
cargo test --locked --manifest-path plugins/text-tools/Cargo.toml --target-dir target/text-tools-native
cargo test --locked --manifest-path plugins/arcade-2048/Cargo.toml --target-dir target/arcade-2048-native
cargo test --locked --manifest-path plugins/nonogram/Cargo.toml --target-dir target/nonogram-native
cargo test --locked --manifest-path plugins/story-dice/Cargo.toml --target-dir target/story-dice-native
```

PDK 导出在原生单测中不运行。原生通过后仍可能存在 Wasm 目标依赖、导出函数、运行资源或视图契约错误，所以必须继续执行真实 Wasm 验收。

## 真实 Wasm 与手写 fixture

先按 [开发指南](./development.md#准备宿主校验工具) 构建 `extension-tool` 并设置 `STARTER_EXTENSION_TOOL`。从**市场仓库根目录**执行完整离线检查；PowerShell：

```powershell
$env:MARKET_REVISION = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds().ToString()
node scripts/check.mjs
```

macOS / Linux 从**市场仓库根目录**执行：

```sh
export MARKET_REVISION="$(date -u +%s)"
node scripts/check.mjs
```

使用未被本地 `dist/` 占用的新修订号。同一秒重跑可能重名，重复目录会被拒绝，不能把这一错误解释为插件校验失败。完整流程会：

1. 对 `listings.json` 的每个插件执行 `cargo fmt -- --check`，使用其锁文件编译 `wasm32-unknown-unknown` 发布模块。
2. 将清单、Wasm 和 fixture 读入有界快照，调用同一个宿主工具执行 `validate` 和 `test`，打包使用的就是已检查字节。
3. 检查每条清单命令都被 1–32 个 fixture 中至少一个用例覆盖，核对返回 ID / 版本以及各项通过结果。
4. 所有插件成功后才完成 `dist/market-<revision>`，失败清理本次暂存目录，不覆盖已有产物。
5. 使用一次性内存密钥签名并验签完整目录及包哈希，不读取生产私钥，不修改 `public-key.txt`，不产生可发布的签名 `catalog.json`。

`validate` 进行权威清单、导入 / 导出和模块静态编译校验，不创建实例，也不执行 Wasm start。`test` 在真实无界面 worker 内执行 fixture，按命令结果类型检查输出；view 结果总是经过完整 Rust 视图与额度校验。它们均不启动桌面窗口。市场侧的 JavaScript 元数据检查不能代替这两项。

### 聚焦单个插件

修改单个插件后，先从**市场仓库根目录**重新编译对应 Wasm。以文本工具箱为例，以下命令适用于 PowerShell 和 macOS / Linux shell：

```sh
cargo build --locked --release --target wasm32-unknown-unknown --manifest-path plugins/text-tools/Cargo.toml --target-dir target/text-tools
```

然后直接调用工具，仅验证该插件。PowerShell：

```powershell
& $env:STARTER_EXTENSION_TOOL validate --manifest plugins/text-tools/extension.json --wasm target/text-tools/wasm32-unknown-unknown/release/starter_extism_text.wasm
& $env:STARTER_EXTENSION_TOOL test --manifest plugins/text-tools/extension.json --wasm target/text-tools/wasm32-unknown-unknown/release/starter_extism_text.wasm --fixture plugins/text-tools/smoke.json
```

macOS / Linux 从**市场仓库根目录**执行：

```sh
"$STARTER_EXTENSION_TOOL" validate --manifest plugins/text-tools/extension.json --wasm target/text-tools/wasm32-unknown-unknown/release/starter_extism_text.wasm
"$STARTER_EXTENSION_TOOL" test --manifest plugins/text-tools/extension.json --wasm target/text-tools/wasm32-unknown-unknown/release/starter_extism_text.wasm --fixture plugins/text-tools/smoke.json
```

模块路径以 `build.json` 和实际 `--target-dir` 为准。工具成功返回 JSON，包含 `validated: true`；`test` 还包含通过的 smoke 报告。不要使用旧模块证明修改后的源码通过，源码变化后应先重新编译。

### 编写可发现回归的断言

每个 `smoke.json` 可以是一个对象，也可以是按顺序执行的 1–32 个对象。以下两条均可用于现有文本工具箱，分别检查完整文本和视图的稳定字段：

```json
[
  {
    "command": "uppercase",
    "input": "Straße 中文",
    "expectOutput": "STRASSE 中文"
  },
  {
    "command": "workshop",
    "input": {
      "version": 2,
      "action": null,
      "state": "",
      "fields": {},
      "locale": "en"
    },
    "expectJsonSubset": {
      "version": 2,
      "display": "Text workshop",
      "effects": [],
      "artifacts": []
    }
  }
]
```

这只是断言格式示例，不满足文本工具箱现有三条命令的完整覆盖要求；正式修改应维护其完整 [`smoke.json`](../plugins/text-tools/smoke.json)。

`expectOutput`、`expectJson`、`expectJsonSubset`、`expectError` 必须且只能选一个：分别比较完整字符串、JSON 等价、手写 JSON 对象子集和稳定错误码。`expectJsonSubset` 必须为非空对象；对象递归匹配声明字段，数组与标量仍完全相等，不进行前缀、模糊或字符串包含匹配。这样可以核对状态、产物或 effects 而不复制整个界面。涉及重要状态演进时，应声明能区分正确与错误行为的字段。

预期必须由规则推导并写入仓库，不允许在构建时捕获当前输出再作为同次预期自比。`cancelAfterMs` 可设为 1–5000 毫秒，配合预期错误验证取消；取消发生的实际阶段与机器速度有关，应选择执行时间确定长于取消延迟的受控用例。主动取消通常预期 `USER_CANCELLED`，不应与执行预算耗尽的 `EXTENSION_TIMEOUT` 混淆。

fixture 数组不会自动把上一条输出 state / fields 传给下一条，也不会共享 Wasm 实例全局状态。每条输入必须明确构造其初始请求；它们可复用 worker 进程来验证取消后恢复等行为。完整契约以 [宿主开发工具说明](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/extensions.md#构建-rust-示例) 和宿主 CLI 源码为准。

## 合成宿主上下文与真实效果

离线 fixture 可以构造 `host.permissions`、`host.storage` 和 `action: "host-result"` / `results`，真实执行插件生成或消费这些数据的逻辑。例如 [2048 fixture](../plugins/arcade-2048/smoke.json) 同时检查 storage 请求、成功 revision 回执和 `CONFLICT` 回执。它证明 Wasm 会按协议处理这些输入，**不会真的写入宿主存储或执行网络请求**。

真实应用中，这些保留字段由宿主注入，页面请求不能自行伪造。需要同时验证：

- 未授权时离线功能仍可用；插件不把可选授权变成启动必需条件，也不在初次打开时自动申请写入。
- 点击保存后先保持待完成状态，收到匹配 effect ID 的成功回执才宣告保存；失败、取消或响应省略不会自动重试有副作用的操作。
- 写入使用宿主当前 revision；另一窗口修改后，旧 revision 发生冲突，保留本窗口草稿并提示重新审阅。
- 具有列表操作的界面绑定稳定身份或列表 revision，刷新后不把旧索引误应用到另一条记录。
- 未来或损坏的插件数据拒绝覆盖，原始数据保留；撤销、新游戏或切换题目不会被误当作已经持久保存。

涉及宿主 effects 或协议变化时，应在宿主仓库执行相关服务回归，包括权限、storage CAS、执行代次与 effect 回执。真实网络、系统凭据、文件对话框以及跨窗口生命周期，还需要在相应平台上独立验收，不能仅凭合成 fixture 声称通过。

## 界面与安装验收

插件没有自己的网页，UI 由宿主渲染器绘制；测试时仍需考虑数据和布局。中文与英文分别检查初始化及关键动作，在窄屏和常见桌面宽度下确认长文本、网格、线索、错误信息与保存入口可读。用键盘检查 Tab 顺序、焦点、表单提交和操作按钮；普通文本应保持字面内容，Markdown 和生成文件遵循宿主安全边界。

宿主浏览器测试默认使用无头模式，不抢占正在工作的桌面。已有生产构建和 Playwright 用例可从**宿主仓库根目录**运行：

```sh
pnpm test:ui
```

该命令验证宿主已有浏览器用例，不会自动安装或遍历市场全部包。为新插件增加渲染用例时，应使用其真实生成的视图，并保留实际动作 / 状态规则测试；不能用截图或 mock 结果替代 Wasm 执行验证。浏览器演示也不能证明桌面 IPC、系统凭据或持久化已成功。

桌面验收使用独立测试数据目录或已安排的前台时段，从本地生成的包完成安装、权限审阅、启用、页面 / 独立窗口、保存 / 重开、会话导出 / 导入和卸载。两窗口同时编辑时验证 CAS；插件更新、权限撤销或替换期间验证旧请求不会串入新安装。不在用户正在工作的应用中直接移动存档、注入测试数据或启动抢焦点窗口。正式安装版、平台差异和升级恢复遵循 [宿主桌面测试规范](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/testing.md#避免干扰本机桌面)。

## CI 与交付证据

[`ci.yml`](../.github/workflows/ci.yml) 在 PR 和 `main` 推送时使用 Ubuntu，先运行市场脚本回归，再检出指定宿主、构建工具、编译所有插件并执行离线 `check`。CI 的新工作区使用固定测试修订号，不代表本地可以反复复用同一产物目录。宿主由 `STARTER_HOST_REF` 仓库变量选择，未配置时为 `develop`；应在跨仓库协议修改时固定到已经推送且审核过的兼容提交。

CI 配置存在不表示某次已经成功运行，也不表示 Windows / macOS、真实桌面或正式 Release 已通过。PR 描述记录本次实际的宿主提交、平台、执行命令及结果，说明未验证范围。发布前还要核对生产公钥、目录有效期、增量修订号、完整附件和线上 latest，步骤统一维护在 [发布指南](./publishing.md)。
