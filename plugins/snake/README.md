# 贪吃蛇

离线 Canvas 贪吃蛇，包格式 3 / API 2 / view 2，自定义界面入口为 [`ui/main.html`](./ui/main.html)。速度、绘制和方向输入在隔离界面中本地运行，不为每个画面或移动调用 Wasm / 原生 IPC；Wasm 负责入口、进度重放校验和真实的私有纪录存储流程。

## 玩法

- 点击开始，使用方向键、WASD、触控方向按钮或在棋盘上滑动。方向不能立即反转，最多预排两个转向，避免同一帧快速输入直接掉头。
- 每颗食物 10 分。撞到墙、身体或障碍结束；移动到刚刚释放的尾格允许，满盘时获胜。
- 空格或暂停按钮切换暂停；失焦、文档隐藏、超过 450 ms 的长帧和宿主操作都会暂停，返回后手动继续。恢复时不追赶离开期间的游戏时间，也不会跳过碰撞。
- 下一局可选择悠闲 160 ms、标准 110 ms、疾速 85 ms 三档逻辑步长，以及穿墙和固定障碍。设置在重开时生效；有进度时重开需要在页面中明确确认。
- 种子为 1–4294967295 的整数。同种子、模式和方向历史会得到相同食物序列。种子随机按钮仅用于游戏，不用于密码或安全用途。
- 每局最多 8192 个逻辑步，达到后暂停并要求新开局。游戏没有隐形计时得分，不将客户端分数作为可信保存输入。

## 进度、音效与权限

每个逻辑步通过宿主 MessagePort 留下有界 checkpoint，仅同步草稿，不调用 native / Wasm；最高速度约 12 次 / 秒，只有内容变化才发送，在途时合并为最新值。宿主忙碌期间延后同步，保留失败时暂停并允许手动重试。暂停、结束、静音切换和重开也会保留。返回界面时先重放校验，再以暂停状态恢复；游戏临时进度不会单独触发未保存编辑提示，需要长期保留时通过宿主会话工具导出。检查点只含 schema、非零种子、模式、静音设置和方向历史，不接受直接传入棋盘、食物或分数。未知字段、未来 schema、超长日志、反向操作或结束后的额外操作均拒绝；损坏导入不会自动覆盖原进度，需明确新开局后才开始保留新棋局。

音效由 Web Audio 本机合成，无音频文件、远程资源或网络权限。必须先有用户点击才能初始化声音；支持静音，暂停或隐藏后挂起，卸载界面时关闭音频并清理动画、监听和观察器，拒绝音频或环境不支持时仍能正常玩。界面不使用持续装饰动效，减少动效设置不会影响碰撞及游戏速度。

插件无必需权限，`storage` 是唯一可选权限。未授权仍可完整游玩和导出会话；授权后点击“保存模式纪录”，Wasm 先从种子重放整局计算分数，再按速度 / 穿墙 / 障碍组合保存到本插件 `snake-records` 私有键。12 种模式分别记录，分数不会把更高的现有纪录降低，也不会覆写其他模式。

保存使用宿主当前 revision CAS，成功回执核对 effect ID、revision 和全部模式的预期保存值，再用确认的纪录更新显示。另一窗口改写、保存取消、失败或响应省略时，当前棋局和暂停状态保留，不自动重试。不兼容或损坏的纪录版本保留原件并禁用保存，离线游玩与会话导出仍可使用。纪录存储不是棋局备份，保存纪录后仍通过当前会话 checkpoint 保留棋局；卸载不承诺删除私有存档。重放核验保证得分符合种子和操作历史，不提供防作弊计时或联网排行榜认证。

## 构建与验收

先按 [开发指南](../../docs/development.md#准备宿主校验工具) 准备宿主 `extension-tool`。从**市场仓库根目录**验证规则与 Wasm：

```sh
cargo test --locked --manifest-path plugins/snake/Cargo.toml --target-dir target/snake-native
node --test plugins/snake/ui/main.test.mjs
cargo build --locked --release --target wasm32-unknown-unknown --manifest-path plugins/snake/Cargo.toml --target-dir target/snake
```

从**宿主仓库根目录**对真实模块与 HTML 快照执行 fixture：

```sh
pnpm extension:test --manifest marketplace/plugins/snake/extension.json --wasm marketplace/target/snake/wasm32-unknown-unknown/release/market_snake.wasm --ui-dir marketplace/plugins/snake/ui --fixture marketplace/plugins/snake/smoke.json
```

原生测试覆盖确定性食物、边界 / 穿墙、增长、满盘、障碍、尾格释放、反向与非法进度、未来数据隔离、可选存储、分模式保留、CAS 和回执。界面脚本回归覆盖检查点合并、保存与恢复等状态变化。手写 [`smoke.json`](./smoke.json) 验证真实 Wasm 中英启动、吃食物得分重放、存储请求、成功 / 冲突 / 响应省略以及非法输入；合成宿主回执不等同真实桌面存储验收。

界面验收还需覆盖键盘、窄屏触控、滑动、暂停 / 失焦、声音开关、重开确认、切换恢复和保存取消。Playwright 无头验收可验证业务 DOM 和 checkpoint，但 WebView 音频策略及真实平台生命周期仍需单独验收。
