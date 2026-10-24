# 故事骰子

真正运行在 Extism Wasm 内的离线写作应用。奇幻、温柔科幻和生活小事三个题材各有五组中英词库，每组八个选项。锁定喜欢的人物、地点、冲突、物件或转折，再掷其余槽位；种子、序号和锁定状态相同即可复现结果，不用系统时间或网络随机数，也不用于密码等安全用途。

掷骰只刷新灵感，不替换正在写的草稿。“写入草稿”与“载入收藏”会明确替换当前草稿。最近八组灵感留在当前会话，标题与草稿可编辑，Markdown 产物通过宿主保存按钮选择位置；插件不会直接写文件。切换题材会解除槽位锁定。

`storage` 是唯一可选权限。拒绝时仍可完整掷骰、编辑和导出；授予后最多手动收藏八篇草稿。收藏使用真实 storage effect 和宿主 revision CAS，收到成功回执后才显示已保存，冲突、取消或响应省略均不会自动重试。载入与移除核对按钮对应的列表 revision，另一窗口修改收藏后先刷新列表、保留当前草稿，不把旧索引应用到新列表。存储未来版本或无效数据拒绝读取和覆盖，保留原数据供恢复；会话历史不冒充持久收藏。

协议为包格式 3 / API 2 / view 2，无网络或文本读取权限；宿主发送的文件与文本不作为草稿自动导入。状态、字段、动作、槽位范围、序号、历史、收藏和回执均在插件入口校验，草稿上限 24 KB，收藏存档版本独立于宿主注册表版本。

在宿主仓库根目录运行：

```powershell
cargo test --locked --manifest-path marketplace/plugins/story-dice/Cargo.toml --target-dir artifacts/market-story-target
cargo build --locked --release --target wasm32-unknown-unknown --manifest-path marketplace/plugins/story-dice/Cargo.toml --target-dir artifacts/market-story-target
pnpm extension:test --manifest marketplace/plugins/story-dice/extension.json --wasm artifacts/market-story-target/wasm32-unknown-unknown/release/starter_extism_story_dice.wasm --fixture marketplace/plugins/story-dice/smoke.json
```

原生单测验证确定性、锁定、草稿保留、历史额度、字段错误、越界与未来数据拒绝、固定安全导出名，以及收藏保存／重开／冲突。手写 `smoke.json` 对真实 Wasm 验证启动、动作、中英、完整 Markdown 产物、storage effect、成功／冲突回执和非法动作；宿主 CLI 另执行权威视图结构与额度校验，不加载桌面 GUI。
