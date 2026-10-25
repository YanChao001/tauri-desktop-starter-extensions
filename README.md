# Tauri Starter 扩展市场

由仓库维护者审核、发布和下架的 Extism Wasm 扩展目录。应用只读取此仓库 Release 中经过签名的 `catalog.json`，下载同仓库的扩展包；用户仍需审阅权限并明确启用。

## 开发与收录

所有插件使用官方 Extism Rust PDK，编译为真实 Wasm，由宿主绘制视图并执行受控效果，不依赖网页脚本或远程页面。

| 插件                                | 功能                                               | 权限         |
| ----------------------------------- | -------------------------------------------------- | ------------ |
| [文本工具箱](./plugins/text-tools/) | Unicode 大小写命令、清理空白、去重排序、统计与导出 | 可选文本读取 |
| [2048 街机](./plugins/arcade-2048/) | 方向移动、合并计分和可复现棋局                     | 可选私有存储 |
| [数织谜题](./plugins/nonogram/)     | 根据行列线索填格、标记和检查谜题                   | 可选私有存储 |
| [故事骰子](./plugins/story-dice/)   | 五槽锁定、题材切换、种子复现、草稿与故事导出       | 可选私有存储 |

文本工坊无需授权即可手动输入，授权后才可显式发送宿主文本；快捷大小写命令需要该文本授权。其余插件未授权存储时仍可使用当前会话，授权后才可提交持久化效果。纪录、进度和收藏属于各插件自己的存储，不请求文件系统、网络、凭据或系统命令。每个插件的精确权限、导出和版本以对应 `extension.json` 为准。

复制一个完整插件目录，修改 `extension.json` 中的 ID、版本、作者、命令与权限，在 `build.json` 指定 Wasm 文件名，并为每条命令提供 `smoke.json` 测试。随后在 `listings.json` 增加目录、分类与推荐标记，通过 PR 申请收录。维护者负责审核实际源码、许可、权限与运行行为。

分类目前为 `text`、`data`、`tools`、`games`。清单使用包格式 3 / API 2，声明 `minHostVersion`；每个命令分别声明输入、输出、界面入口和所需权限，扩展级声明必需权限、可选权限及精确网络 origin。版本使用数字 `major.minor.patch`。构建输出、签名私钥和用户安装数据不进入仓库。

当前处于开发阶段，本轮不提供旧协议兼容。宿主与市场工具须配套更新，并重新构建、签名和发布 schema 2 目录；旧 schema 1 Release 不能被新宿主使用。普通开发和自动化回归只使用临时测试密钥，不需要真实发布签名。

```sh
node --test scripts/*.test.mjs
rustup target add wasm32-unknown-unknown
```

本地构建和真实运行校验使用宿主的无界面 `extension-tool`，复用 Rust 清单校验、Extism 导入 / 导出限制和视图协议解析。市场侧 JSON 检查仅用于目录审核与签名前检查，不能代替宿主 ABI 和运行校验。首次从宿主仓库构建工具：

```powershell
# 在宿主仓库根目录执行；首次还需按宿主开发指南准备平台依赖。
pnpm install --frozen-lockfile
pnpm build
cargo build --locked --manifest-path src-tauri/Cargo.toml --bin extension-tool
$env:STARTER_EXTENSION_TOOL = (Resolve-Path src-tauri/target/debug/extension-tool.exe).Path
Set-Location marketplace
$env:MARKET_REVISION = "1"
node scripts/check.mjs
```

在 macOS / Linux 上指定同目录的 `extension-tool` 无后缀可执行文件。作为宿主子模块时也会默认探测 `../src-tauri/target/debug/extension-tool(.exe)`；独立检出时必须通过 `STARTER_EXTENSION_TOOL` 指定绝对路径。工具只运行校验和 worker，不启动桌面应用。

`build` 对所有收录项检查 Rust 格式、使用锁文件编译真实 Wasm，再将清单、模块和 fixture 固定为一次快照，逐个调用宿主 `validate` / `test`；全部通过后才生成完整的 `dist/market-<revision>`，失败不会留下看似完成的目录，也不覆盖已有版本。`smoke.json` 遵循宿主 fixture 格式，1–32 个测试必须覆盖所有命令；view 输出也经过 Rust 视图校验。`expectJsonSubset` 可手工声明关键 state、display 或 effects，对象递归匹配所列键，数组和标量完全比较；不能在构建时把当前输出捕获为预期再自比。离线测试通过合成 `host` / `host-result` 检查可选授权、存储请求及失败恢复，不访问真实用户数据，也不代替宿主 broker 的端到端验收。

`check` 在 `build` 之后使用仅驻留内存的一次性密钥签名并验签目录，逐个核对完整包、模块哈希和元数据，不读取生产私钥，不改变 `public-key.txt`，不写入可发布的 `catalog.json`。因此普通开发、PR 检查和产物验签不需要签名 Secret。

## 发布与管理

先在宿主验证新增插件的安装、权限和实际运行，审核合并到 `main`，再在 Actions 手动运行“发布市场”。流程编译并真实运行所有已收录插件、计算完整包与 Wasm 的独立哈希、使用 `MARKET_SIGNING_PRIVATE_KEY` Secret 签名目录，核对附件后上传到草稿 Release，上传全部成功才发布为 latest。Release 标签通过 `GITHUB_SHA` 绑定本次构建的完整提交；本地发布必须显式设置 `MARKET_SOURCE_SHA`，缺失或不是完整 40 位 SHA 时拒绝发布，不使用可变分支头。目录修订号必须递增；需要撤回推荐版本时也发布更高修订号。签名公钥在 `public-key.txt`，应用公钥由宿主配置维护；轮换公钥需同步宿主。

独立市场 CI 会检出宿主仓库、准备前端编译资源并构建同一 `extension-tool`。`STARTER_HOST_REF` 仓库变量指定已经推送、审核过的宿主提交；发布工作流的 `host_ref` 输入可显式覆盖。未配置时读取 `develop`，实际解析到的提交会写入 Actions 摘要。宿主和市场同时修改协议时，应先把宿主提交推送并将变量固定到该提交，再检查并发布市场；本地尚未推送的提交不能被独立 CI 使用，旧宿主会明确拒绝不支持的清单，而不会跳过验证。这里的 CI 配置与本地离线通过不表示线上目录已经发布。

签名 payload 包含 UTC Unix 秒 `issuedAt` / `expiresAt`，签名时生成 7 天有效期，宿主允许最长 30 天并拒绝超过 5 分钟的未来签发时间。维护者应在到期前发布更高修订号的目录；过期缓存不能用于下载或安装。宿主按仓库与公钥指纹持久化最高修订号和 payload 摘要，重启后仍拒绝回退或同修订替换。更换仓库或公钥形成新的信任范围，不继承旧来源缓存。目录可展示要求更高宿主版本的扩展，实际安装会明确拒绝不兼容的包。

市场下载附带仓库、公钥指纹、目录修订和完整包哈希的来源收据；宿主安装入口再次对照已验签目录核验，不能通过本地包自行声明市场来源。收据说明目录发布方与包完整性，不代替源码审核，也不自动授权。同 ID 的本地安装不能获得市场身份，替换和启用仍需用户确认。

删除 `listings.json` 中对应项并发布即可下架，客户端刷新后停止提供新下载。已经安装的用户数据保留。删除或隐藏 Release 不等于回收已分发的包。本仓库不提供远程卸载、账户或付费功能。

本地先设置 `MARKET_REVISION`，运行 `node scripts/build.mjs` 构建插件。随后将私钥放在仓库外的临时文件，使用 `MARKET_KEY_FILE` 指定路径，运行 `node scripts/sign-release.mjs dist/market-<修订号>`，再运行 `node scripts/verify.mjs dist/market-<修订号>`。正常开发与 PR 检查无需私钥；Actions 的编译与签名使用独立任务和工作目录，签名任务重新核对已审核的清单，不执行插件构建。

## 与宿主关联

宿主通过 `marketplace/` 子模块引用本仓库。先在此仓库提交并推送，再更新宿主的子模块提交引用。普通应用构建与内置扩展不依赖本仓库初始化。用户安装后的插件位于应用数据目录，不写入此开发目录。
