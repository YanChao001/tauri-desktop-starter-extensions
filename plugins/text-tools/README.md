# 文本工具箱

离线 Rust / Extism 插件，使用包格式 3、API 2 和视图 2。`workshop` 提供侧栏与独立窗口入口；`uppercase`、`lowercase` 保留直接处理宿主文本的普通命令。

工坊可以手动输入、统计、选择转换、将结果作为下一次输入，或生成 TXT / Markdown 文件。所有计算都在 Wasm 中完成，不访问网络、文件系统、时钟、随机数或私有存储。生成文件后仍需用户在宿主中选择保存位置。

## 文本规则

- 统计分别显示 Unicode 标量数量、UTF-8 字节数、逻辑行和空白分隔词。组合音标及 Emoji 序列可能包含多个标量，不能把标量数量当作视觉字符数；中文没有分词器，词数仅表示空白分隔段。
- 空文本为 0 行；末尾换行保留一个空行，CRLF、单独 CR 和 LF 都视为逻辑换行。行处理结果统一使用 LF。
- 支持原文、大写、小写、清理首尾空白、清理每行首尾空白、去空白行、行去重、正反排序、反转行顺序、规范空白、行编号、行引用和统一换行，共 14 种操作。
- 去重按完整行精确比较并保留首次出现；大小写不同或空格不同的行保留为不同行。排序使用 Unicode 标量顺序，不声称是语言学排序。
- 工坊输入最多 32 KiB UTF-8，结果最多 96 KiB。超限报错并保留输入，绝不静默截断。结果超过输入框限额时仍可生成文件；“将结果作为输入”成功后切回原文模式，方便继续选择另一项处理。
- TXT 保存完整转换结果；Markdown 将结果放入缩进代码块并统一换行，使 HTML、链接和 Markdown 围栏保持字面文本。

## 权限

插件没有必需权限。工坊手动输入无需授权；可选的 `text:read` 用于用户明确点击“发送所选数据”后接收宿主输入，也用于普通大小写命令。未授予此权限时，普通命令入口不可用，工坊仍可使用。工坊不接受文件或网络数据，没有任何网络来源声明。

## 构建与验证

在市场仓库根目录执行：

```sh
cargo test --manifest-path plugins/text-tools/Cargo.toml --locked --target-dir target/text-tools
cargo build --manifest-path plugins/text-tools/Cargo.toml --release --locked --target wasm32-unknown-unknown --target-dir target/text-tools
```

在宿主仓库根目录，以生成的 Wasm 和本目录的 `smoke.json` 执行宿主 `extension:test`。fixture 覆盖 Unicode 大小写、中文初始化、排序、TXT / Markdown 导出、显式授权输入和非法动作拒绝；宿主 CLI 会真实调用 Wasm 并校验返回视图。

原生单测覆盖业务语义与错误边界，Wasm PDK 导出只在 `wasm32` 编译，避免原生单测链接到不存在的 Extism host imports。`build.json` 中维护发布所需 Wasm 文件名，编译结果不进入 Git。
