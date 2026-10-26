# AGENTS.md

本文件适用于独立市场仓库及其作为宿主子模块的检出。先读 [README](./README.md) 与 [文档索引](./docs/README.md)，按任务阅读相关文档和源码；贡献要求见 [CONTRIBUTING](./CONTRIBUTING.md)。

## 工作边界

- 本仓库维护收录插件、构建脚本、目录签名与 Release 发布；宿主维护运行时、权限 broker、通用视图、隔离 UI 与数据保护。协议与 DTO 以 [宿主扩展指南](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/extensions.md) 及其 Rust 实现为准，不在市场脚本复制另一套运行规则。
- 当前四款为科学计算器、贪吃蛇、局域网信使和壁纸库，收录项以 `listings.json` 为准。构建入口固定独立 Rust / Cargo 项目，使用提交的锁文件；自定义界面是本地单文件 HTML / CSS / JavaScript，不增加没有实际需要的 React / Vite 构建链。其他业务语言需先实现受审核的构建流程。
- 界面必须使用宿主隔离与消息桥，不直接访问宿主 DOM、原生 IPC 或远程页面。SDK 注入主题、语言和可覆写的默认样式；插件仍负责键盘、窄屏、减少动效及真实布局。独立窗口提供语言与主题入口，工具菜单不承担返回扩展中心导航。
- 说明、提示与长文遵循 [宿主界面规范](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/conventions/ui-guidelines.md#说明与布局稳定)：精简常驻解释，操作反馈使用 SDK `notify`，避免动态信息挤压布局，并保留必要正文、关键错误和无障碍信息。
- 修改源码、清单和手写 fixture，不手改 `dist/`、`target/` 或已签名目录。新增依赖说明用途、许可、构建和平台影响，不借机全量升级。
- 插件自行校验动作、导入状态与业务数据。可选权限不阻塞无关的离线功能；存储写入使用 revision 与匹配的效果回执，处理失败、冲突和未来版本，保留用户原始数据。
- `extension-tool` 是清单、ABI 与运行校验的权威入口；市场 JSON 检查不能替代它。合成宿主上下文的离线 fixture 不能冒充真实网络、私有存储或凭据操作已验收。
- `starter.services.request` 仅提供宿主固定的 LAN 与壁纸服务，分别核验 `messaging:lan` / `wallpapers:manage`，不能自行注册任意服务。新权限与操作必须先有配套宿主实现，再用确定提交验证。
- LAN 只连接显式指定的私有 / 回环 IP，明文且无远端身份认证；后台连接属于原生窗口，导航不等于停止服务。壁纸默认搜索必应图片，不按许可筛选；预览和下载仅使用宿主搜索所得、绑定当前身份且未过期的结果标识，不接受界面传入的任意 URL。保留 HTTPS、SSRF / DNS、重定向及图片额度校验，原图另存与压缩后入现有应用图库分别验证；不能放开路径、任意代理或关闭公网地址检查来让测试通过。旧 Commons page ID 操作只用于兼容，不扩大其来源与许可规则。

## 验证与交付

从本仓库根目录按 [测试规范](./docs/testing.md) 选择检查。脚本回归为 `node --test scripts/*.test.mjs`；完整离线检查为准备宿主工具和未使用修订号后执行 `node scripts/check.mjs`。纯文档修改检查格式、链接和源码一致性，不需要重编全部插件。

本地 `check` 使用一次性测试密钥，不使用生产签名 Secret、不改变生产公钥。密钥、令牌、真实用户数据和环境文件不入库、不写日志；安全问题按 [报告渠道](./docs/security.md#漏洞与密钥事件) 处理。真实桌面检查使用隔离环境或已安排的前台时段，不占用用户正在工作的桌面。

按可审查的里程碑及时提交，采用 Conventional Commits，type / scope 英文，主题与正文中文。市场特性分支通过 PR 进入 `main`，宿主普通特性进入 `develop`；不直接推送或改写保护分支。市场是独立 Git 仓库，先提交市场修改，再按 [贡献流程](./CONTRIBUTING.md#分支提交与-pr) 更新宿主引用；本地提交不表示远端可获取。

签名、续签、下架与恢复遵循 [发布指南](./docs/publishing.md)。没有明确发布授权时不推送、合并或触发线上发布。交付时说明实际检查及未验证范围，区分源码提交、本地包、CI 成功与线上 latest。
