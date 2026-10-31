# 测试与验收

本页定义市场插件和发布工具的验证范围。环境与宿主 `extension-tool` 的准备见 [开发指南](./development.md)，目录签名与上线见 [发布指南](./publishing.md)。构建、离线通过、CI 成功和线上发布是不同证据，交付时分别说明。

## 按改动选择检查

| 修改范围                             | 本地检查起点                                                | 按影响补充的验收                             |
| ------------------------------------ | ----------------------------------------------------------- | -------------------------------------------- |
| 文档、示例命令                       | 对照实际脚本、路径和 CLI 参数；检查相关 Markdown 格式及链接 | 平台特有命令未执行时明确说明                 |
| 市场脚本、目录协议、签名或发布流程   | 对应脚本测试；影响共享构建或产物时执行完整 `check`          | 生产凭据与权限、真实发布结果                 |
| 插件规则、state / fields 或 PDK 导出 | 受影响插件的规则单测、格式 / Clippy、真实 Wasm `test`       | 实际宿主交互；输入、状态与额度边界           |
| 清单、权限、视图、导出产物           | 单插件真实 Wasm fixture 与相关界面回归；交付前完整 `check`  | 安装与授权；受影响语言、布局、键盘或保存流程 |
| 私有存储、并发或宿主 effects         | 相关规则、成功 / 失败回执 fixture；宿主服务变更测对应边界   | CAS、取消、重开及持久化恢复中受影响的行为    |
| 宿主 API / ABI 协议联动              | 重建指定宿主工具后完整 `check`，同步相关宿主测试            | 受影响平台、桌面生命周期与正式安装环境       |

先验证受影响行为，共享入口、协议或独立失败场景改变时再扩大范围。检查通过后，仅在继续修改、失败或仍有明确疑点时重跑。市场 `check` 覆盖格式、构建、真实 Wasm 和离线验签，不执行 Node 回归、原生 Rust 单测或 Clippy；CI 与发布 build job 另行执行这些检查。

## 脚本、界面与规则单测

从**市场仓库根目录**运行市场脚本与插件界面回归：

```sh
node --test scripts/*.test.mjs plugins/*/ui/*.test.mjs
```

这些测试覆盖签名、包完整性、清单 / 目录规则、发布来源、构建工具调用约束，以及插件界面的状态恢复和异步交互。界面测试在 Node 中执行真实界面脚本，并使用必要的 DOM / SDK 替身；它们不需要 npm 依赖或生产密钥，也不等同于真实 WebView、原生服务或编译并运行四个插件的 Wasm。

同一规则的输入边界集中在对应单元测试；打包等上层入口验证接线、字节快照、身份和失败传播。完整入口已包含下层校验时，无需再对相同输入重复启动校验进程。

规则函数优先使用原生 Rust 单测，覆盖业务不变量和对应的非法输入。从**市场仓库根目录**选择受影响插件运行：

```sh
cargo test --locked --manifest-path plugins/scientific-calculator/Cargo.toml --target-dir target/scientific-calculator-native
cargo test --locked --manifest-path plugins/snake/Cargo.toml --target-dir target/snake-native
cargo test --locked --manifest-path plugins/lan-chat/Cargo.toml --target-dir target/lan-chat-native
cargo test --locked --manifest-path plugins/wallpaper-browser/Cargo.toml --target-dir target/wallpaper-browser-native
```

PDK 导出在原生单测中不运行。原生通过后仍可能存在 Wasm 目标依赖、导出函数、运行资源或视图契约错误，所以必须继续执行真实 Wasm 验收。

修改 Rust 代码时，同时对相同清单和 target 目录运行 `cargo clippy --locked --manifest-path <Cargo.toml> --target-dir <原生测试目录> --all-targets -- -D warnings`。CI 与发布构建会按 `listings.json` 遍历全部收录插件执行原生单测和 Clippy，市场脚本与插件使用市场工具链，宿主 CLI 单独使用宿主工具链。

## 真实 Wasm 与手写 fixture

按 [开发指南](./development.md#准备宿主校验工具) 准备工具，再从市场根目录执行 [首次完整离线检查](./development.md#首次完整离线检查)。使用未被本地 `artifacts/` 占用的新修订号；同一秒重跑可能重名，重复目录不属于插件校验失败。完整流程会：

1. 对 `listings.json` 的每个插件执行 `cargo fmt -- --check`，使用其锁文件编译 `wasm32-unknown-unknown` 发布模块。
2. 将清单、Wasm、被引用的 UI 文档和 fixture 读入有界快照，调用宿主工具执行一次 `test`，打包使用的就是已检查字节。
3. 检查每条清单命令都被 1–32 个 fixture 中至少一个用例覆盖，核对返回 ID / 版本以及各项通过结果。
4. 所有插件成功后才完成 `artifacts/market-<revision>`，失败清理本次暂存目录，不覆盖已有产物。
5. 使用一次性内存密钥签名并验签完整目录及包哈希，不读取生产私钥，不修改 `public-key.txt`，不产生可发布的签名 `catalog.json`。

`test` 包含权威清单、UI 文档、导入 / 导出和模块静态编译校验，随后在真实无界面 worker 内执行 fixture，按命令结果类型检查输出；view 结果总是经过完整 Rust 视图与额度校验。仅需静态检查时可单独使用 `validate`，它不创建实例，也不执行 Wasm start。完整流程无需先额外调用 `validate`；两个命令均不启动桌面窗口，市场侧的 JavaScript 元数据检查不能代替宿主校验。

### 聚焦单个插件

修改单个插件后，先从**市场仓库根目录**重新编译对应 Wasm。以计算器为例，以下命令适用于 PowerShell 和 macOS / Linux shell：

```sh
cargo build --locked --release --target wasm32-unknown-unknown --manifest-path plugins/scientific-calculator/Cargo.toml --target-dir target/scientific-calculator
```

然后从市场根目录使用相同的快照校验入口，PowerShell / macOS / Linux 均可执行：

```sh
node --input-type=module -e 'import {validatePlugin,resolveHostTool} from "./scripts/host-tool.mjs"; await validatePlugin({manifestPath:"plugins/scientific-calculator/extension.json",wasmPath:"target/scientific-calculator/wasm32-unknown-unknown/release/starter_extism_scientific_calculator.wasm",fixturePath:"plugins/scientific-calculator/smoke.json",uiDirectory:"plugins/scientific-calculator/ui",executable:await resolveHostTool()});'
```

模块路径以 build.json 和实际 target-dir 为准。入口把同一份清单 / Wasm / HTML / fixture 快照传给原生 `test`；不能省略自定义 UI 文档或用旧二进制证明新源码已通过。

### 编写可发现回归的断言

每个 `smoke.json` 是一个用例对象或顺序执行的 1–32 个对象，覆盖所有清单命令。以下用例检查计算器启动的稳定字段；完整业务断言参考手写的 [`smoke.json`](../plugins/scientific-calculator/smoke.json)，不要复制整份视图作为脆弱快照。

```json
{
  "command": "workspace",
  "input": {
    "version": 2,
    "action": null,
    "state": "",
    "fields": {},
    "locale": "en"
  },
  "expectJsonSubset": {
    "version": 2,
    "display": "Scientific calculator",
    "effects": [],
    "artifacts": []
  }
}
```

`expectOutput`、`expectJson`、`expectJsonSubset`、`expectError` 必须且只能选一个：分别比较完整字符串、JSON 等价、手写 JSON 对象子集和稳定错误码。`expectJsonSubset` 必须为非空对象；对象递归匹配声明字段，数组与标量仍完全相等，不进行前缀、模糊或字符串包含匹配。这样可以核对状态、产物或 effects 而不复制整个界面。涉及重要状态演进时，应声明能区分正确与错误行为的字段。

预期必须由规则推导并写入仓库，不允许在构建时捕获当前输出再作为同次预期自比。`cancelAfterMs` 可设为 1–5000 毫秒，配合预期错误验证取消；取消发生的实际阶段与机器速度有关，应选择执行时间确定长于取消延迟的受控用例。主动取消通常预期 `USER_CANCELLED`，不应与执行预算耗尽的 `EXTENSION_TIMEOUT` 混淆。

fixture 数组不会自动把上一条输出 state / fields 传给下一条，也不会共享 Wasm 实例全局状态。每条输入必须明确构造其初始请求；它们可复用 worker 进程来验证取消后恢复等行为。完整契约以 [宿主开发工具说明](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/extensions.md#构建-rust-示例) 和宿主 CLI 源码为准。

## 合成宿主上下文与真实效果

离线 fixture 可以构造 `host.permissions`、`host.storage` 和 `action: "host-result"` / `results`，真实执行插件生成或消费这些数据的逻辑。例如 [贪吃蛇 fixture](../plugins/snake/smoke.json) 同时检查 storage 请求、成功 revision 回执和 `CONFLICT` 回执。它证明 Wasm 会按协议处理这些输入，**不会真的写入宿主存储或执行网络请求**。

真实应用中，这些保留字段由宿主注入，页面请求不能自行伪造。按涉及的能力验证：

- 未授权时离线功能仍可用；插件不把可选授权变成启动必需条件，也不在初次打开时自动申请写入。
- 点击保存后先保持待完成状态，收到匹配 effect ID 的成功回执才宣告保存；失败、取消或响应省略不会自动重试有副作用的操作。
- 写入使用宿主当前 revision；另一窗口修改后，旧 revision 发生冲突，保留本窗口草稿并提示重新审阅。
- 具有列表操作的界面绑定稳定身份或列表 revision，刷新后不把旧索引误应用到另一条记录。
- 未来或损坏的插件数据拒绝覆盖，原始数据保留；撤销、新游戏或切换题目不会被误当作已经持久保存。

涉及宿主 effects 或协议变化时，应在宿主仓库执行相关服务回归，包括权限、storage CAS、执行代次与 effect 回执。真实网络、系统凭据、文件对话框以及跨窗口生命周期，还需要在相应平台上独立验收，不能仅凭合成 fixture 声称通过。

## 固定原生服务验收

服务调用与 Wasm fixture 是两条边界。内网通信的 Wasm 用例只验证入口、安全文本及附件标签导出；壁纸的离线用例可验证图源 HTTP effect 与匹配回执的解析链，但不执行真实公网请求、图片下载、图库或设置操作。服务请求应在宿主独立测试中验证授权、安装代次、窗口归属、入参、真实副作用和回收；浏览器使用替身时明确标识，不能写成真实服务已通过。

| 变更边界   | 本地或宿主服务层检查                       | 按影响安排的真实环境检查                    |
| ---------- | ------------------------------------------ | ------------------------------------------- |
| LAN        | 规则与坏帧、真实环回 TCP、附件与历史、回收 | 两台设备、路由 / 防火墙、对话框与跨导航收信 |
| 图源规则   | 解析、输入边界、图片身份、effect / 回执    | 变更图源在目标网络的 API、限流及图片可用性  |
| 图片资源   | 授权、URL / SSRF、解码额度、句柄身份与清理 | 真实下载、WebView 预览、另存与取消          |
| 图库与背景 | 导入、配额、revision CAS、其他偏好保留     | 真实图库操作及背景多窗口同步                |
| 外部链接   | 独立授权和非法 URL 拒绝                    | 目标平台打开系统浏览器                      |

此表按变更边界选择，不要求每次修改遍历所有服务、图源与网络组合。坏输入、授权和资源所有权等信任边界应有对应回归；网络及资源规则以 [安全规范](./security.md#原生服务与外部来源) 链接的宿主契约为准。图源 parser 的具体业务用例就近维护在 [壁纸库测试](../plugins/wallpaper-browser/README.md#开发与验证)，真实联网记录所测图源、筛选、传输模式与结果，零 / 少量结果按业务语义判断。服务替身不能证明公网或系统交互成功。

## 界面与安装验收

普通视图由宿主绘制，自定义 HTML 由插件组织；两种路线都需检查数据和布局。中文与英文分别检查初始化及关键动作，在窄屏和常见桌面宽度下确认长文本、网格、错误与保存入口可读。用键盘检查 Tab 顺序、焦点和操作按钮；普通文本保持字面内容，Markdown 和生成文件遵循宿主安全边界。

按变更涉及的操作验证语言、主题、尺寸与生命周期；切换语言保留业务进度，临时游戏与未保存编辑使用正确 dirty 语义。各插件的操作与恢复边界就近维护在插件 README，不将所有插件的界面细节扩展为每次检查清单。

自定义 UI 定向验收包含以下独立行为，完整 profile、窗口字号和宿主控件矩阵由 [宿主扩展验证说明](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/testing.md#自定义扩展界面) 与 runner 维护：

- 首屏使用真实加载骨架和已应用的主题；前台握手停滞可重试，后台 / 离屏不误报，后续 busy 保持正文和焦点。呈现握手不证明全部业务渲染成功。
- 工具入口、内容高度、字号、窄屏、键盘、默认样式覆写和原生 select 例外保持可用，正文不重复宿主标题。
- 草稿合并、轮询与业务动作存在竞争时，旧回执不覆盖新输入或状态，取消不显示成功；对应竞争场景在插件界面测试中维护。
- 可选权限拒绝后保留无关离线功能；真实 Wasm、资源服务替身与原生结果分别记录。

宿主浏览器测试默认使用无头模式，不抢占正在工作的桌面。已有生产构建和 Playwright 用例可从**宿主仓库根目录**运行：

```sh
pnpm test:ui
```

该命令验证宿主已有浏览器用例，不会自动安装或遍历市场全部包。为新插件增加渲染用例时，应使用其真实生成的视图，并保留实际动作 / 状态规则测试；不能用截图或 mock 结果替代 Wasm 执行验证。浏览器演示也不能证明桌面 IPC、系统凭据或持久化已成功。

桌面验收使用独立测试数据目录或已安排的前台时段，从实际生成包验证受影响的安装、权限、窗口、保存恢复或卸载行为。并发、撤权或安装替换变化分别验证 CAS 与旧请求隔离。不在用户正在工作的应用中移动存档、注入测试数据或启动抢焦点窗口。正式安装版、平台差异和升级恢复遵循 [宿主桌面测试规范](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/testing.md#避免干扰本机桌面)。

## CI 与交付证据

[`ci.yml`](../.github/workflows/ci.yml) 在 PR 和 `main` 推送时使用 Ubuntu，先运行市场脚本与插件界面回归、全部收录插件的原生单测和 Clippy，再检出指定当前宿主、构建工具、编译所有插件并执行离线 `check`。CI 的新工作区使用固定测试修订号，不代表本地可以反复复用同一产物目录。宿主由 `STARTER_HOST_REF` 仓库变量选择，未配置时为 `develop`；应在跨仓库协议修改时固定到已经推送且审核过的当前提交，生产发布强制要求完整 SHA。开发阶段不维护历史宿主矩阵，也不把当前检查写成旧版兼容承诺，详见 [固定配套宿主](./publishing.md#固定配套宿主)。

CI 配置存在不表示某次已经成功运行，也不表示 Windows / macOS、真实桌面或正式 Release 已通过。PR 描述记录本次实际的宿主提交、平台、执行命令及结果，说明未验证范围。发布前还要核对生产公钥、目录有效期、增量修订号、完整附件和线上 latest，步骤统一维护在 [发布指南](./publishing.md)。

## 自定义界面无头回归

构建 CLI、内置包与市场 Wasm 后，从宿主根目录运行：

```sh
pnpm test:extensions:ui
```

它覆盖真实 HTML / SDK / 离线 Wasm、主题、键盘、草稿重载、无障碍和隔离负例，使用浏览器协议 / 对话框替身。准备步骤、临时端口、筛选、报告与 `STARTER_EXTENSION_UI_RUN_ID` 统一见 [宿主测试说明](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/testing.md#自定义扩展界面)。只检查壁纸权限组合时可运行 `pnpm test:extensions:ui wallpaper-browser --capabilities-only`，该范围不覆盖完整主题、布局和会话，也不证明原生窗口或公网已验收。
