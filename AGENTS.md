# AGENTS.md

本文件适用于独立市场仓库及其作为宿主子模块的检出。先读 [README](./README.md) 与 [文档索引](./docs/README.md)，按任务阅读相关文档和源码；贡献要求见 [CONTRIBUTING](./CONTRIBUTING.md)。

## 工作边界

- 本仓库维护收录插件、构建脚本、目录签名与 Release 发布；宿主维护运行时、权限 broker、声明式渲染器与数据保护。协议与 DTO 以 [宿主扩展指南](https://github.com/YanChao001/tauri-desktop-starter/blob/develop/docs/extensions.md) 及其 Rust 实现为准，不在市场脚本复制另一套运行规则。
- 当前构建入口固定 Rust / Cargo，依赖使用提交的锁文件；其他语言需要先实现受审核的构建流程。插件输出文本或声明式 JSON，不引入任意网页脚本、HTML 或远程页面。
- 修改源码、清单和手写 fixture，不手改 `dist/`、`target/` 或已签名目录。新增依赖说明用途、许可、构建和平台影响，不借机全量升级。
- 插件自行校验动作、导入状态与业务数据。可选权限不阻塞无关的离线功能；存储写入使用 revision 与匹配的效果回执，处理失败、冲突和未来版本，保留用户原始数据。
- `extension-tool` 是清单、ABI 与运行校验的权威入口；市场 JSON 检查不能替代它。合成宿主上下文的离线 fixture 不能冒充真实网络、私有存储或凭据操作已验收。

## 验证与交付

从本仓库根目录按 [测试规范](./docs/testing.md) 选择检查。脚本回归为 `node --test scripts/*.test.mjs`；完整离线检查为准备宿主工具和未使用修订号后执行 `node scripts/check.mjs`。纯文档修改检查格式、链接和源码一致性，不需要重编全部插件。

本地 `check` 使用一次性测试密钥，不使用生产签名 Secret、不改变生产公钥。密钥、令牌、真实用户数据和环境文件不入库、不写日志；安全问题按 [报告渠道](./docs/security.md#漏洞与密钥事件) 处理。真实桌面检查使用隔离环境或已安排的前台时段，不占用用户正在工作的桌面。

按可审查的里程碑及时提交，采用 Conventional Commits，type / scope 英文，主题与正文中文。市场特性分支通过 PR 进入 `main`，宿主普通特性进入 `develop`；不直接推送或改写保护分支。市场是独立 Git 仓库，先提交市场修改，再按 [贡献流程](./CONTRIBUTING.md#分支提交与-pr) 更新宿主引用；本地提交不表示远端可获取。

签名、续签、下架与恢复遵循 [发布指南](./docs/publishing.md)。没有明确发布授权时不推送、合并或触发线上发布。交付时说明实际检查及未验证范围，区分源码提交、本地包、CI 成功与线上 latest。
