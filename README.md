# Tauri Starter 扩展市场

由仓库维护者审核、发布和下架的 Extism Wasm 扩展目录。应用只读取此仓库 Release 中经过签名的 `catalog.json`，下载同仓库的扩展包；用户仍需审阅权限并明确启用。

## 开发与收录

示例位于 [文本工具箱](./plugins/text-tools/)。使用官方 Extism PDK，复制完整插件目录，修改 `extension.json` 中的 ID、版本、作者、命令与权限，并在 `build.json` 指定 Wasm 文件名。在 `listings.json` 增加目录、分类与推荐标记，通过 PR 申请收录。维护者负责审核实际源码、许可、权限与运行行为。

分类目前为 `text`、`data`、`tools`、`games`。清单仅支持当前包格式 2 / API 1，插件版本使用数字 `major.minor.patch`。构建输出、签名私钥和用户安装数据不进入仓库。

```sh
node --test scripts/*.test.mjs
rustup target add wasm32-unknown-unknown
```

## 发布与管理

先在宿主验证新增插件的安装、权限和实际运行，审核合并到 `main`，再在 Actions 手动运行“发布市场”。流程构建所有已收录插件、计算完整包与 Wasm 的独立哈希、使用 `MARKET_SIGNING_PRIVATE_KEY` Secret 签名目录，核对附件后上传到草稿 Release，上传全部成功才发布为 latest。目录修订号必须递增；需要撤回推荐版本时也发布更高修订号。签名公钥在 `public-key.txt`，应用公钥由宿主配置维护；轮换公钥需同步宿主。

删除 `listings.json` 中对应项并发布即可下架，客户端刷新后停止提供新下载。已经安装的用户数据保留。删除或隐藏 Release 不等于回收已分发的包。本仓库不提供远程卸载、账户或付费功能。

本地先设置 `MARKET_REVISION`，运行 `node scripts/build.mjs` 构建插件。随后将私钥放在仓库外的临时文件，使用 `MARKET_KEY_FILE` 指定路径，运行 `node scripts/sign-release.mjs dist/market-<修订号>`，再运行 `node scripts/verify.mjs dist/market-<修订号>`。正常开发与 PR 检查无需私钥；Actions 的编译与签名使用独立任务和工作目录，签名任务重新核对已审核的清单，不执行插件构建。

## 与宿主关联

宿主通过 `marketplace/` 子模块引用本仓库。先在此仓库提交并推送，再更新宿主的子模块提交引用。普通应用构建与内置扩展不依赖本仓库初始化。用户安装后的插件位于应用数据目录，不写入此开发目录。
