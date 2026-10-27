# 第三方归属与许可

本文件记录计算器插件实际复用的第三方内容及其许可。插件自身采用仓库 [MIT 许可证](../../LICENSE)；第三方内容仍遵循各自的许可或服务条款。

## Microsoft Calculator 单位目录与本地化资源

- 上游：[microsoft/calculator](https://github.com/microsoft/calculator)。
- 固定提交：[`79e1ac53bba690cdf930e5dcd6ed9dbc4cbfb128`](https://github.com/microsoft/calculator/tree/79e1ac53bba690cdf930e5dcd6ed9dbc4cbfb128)。
- 实际复用：从 [UnitConverterDataLoader.cs](https://github.com/microsoft/calculator/blob/79e1ac53bba690cdf930e5dcd6ed9dbc4cbfb128/src/Calculator.ViewModels/DataLoaders/UnitConverterDataLoader.cs)、[英文资源](https://github.com/microsoft/calculator/blob/79e1ac53bba690cdf930e5dcd6ed9dbc4cbfb128/src/Calculator/Resources/en-US/Resources.resw) 与 [简体中文资源](https://github.com/microsoft/calculator/blob/79e1ac53bba690cdf930e5dcd6ed9dbc4cbfb128/src/Calculator/Resources/zh-CN/Resources.resw) 提取单位标识、名称、缩写、系数、顺序、区域条件与默认单位，保存为本插件的 [converter-catalog.json](./src/converter-catalog.json)。
- 本地修改：将目录转换为 JSON，由 Rust 读取并按当前中英文界面的地区条件过滤；温度换算中的 `5/9` 使用有理数表达式，其余系数保留文本。界面参照官方 XAML 的布局与交互，以 HTML、CSS、SVG 与 JavaScript 实现。
- 未复制 Microsoft 的位图、安装图标或字体文件。字体来自用户系统已安装资源；未将 Windows 系统字体重新打包。

以下完整保留该固定提交的 [MIT 许可原文](https://github.com/microsoft/calculator/blob/79e1ac53bba690cdf930e5dcd6ed9dbc4cbfb128/LICENSE)：

```text
Copyright (c) Microsoft Corporation. All rights reserved.

MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED *AS IS*, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Rust Num 数值组件

十进制与有理数引擎使用以下纯 Rust 组件，版本以 [Cargo.toml](./Cargo.toml) 与 [Cargo.lock](./Cargo.lock) 为准。三者均可选择 MIT 或 Apache-2.0 许可，本插件选择 MIT 许可分发。

| 组件           | 用途               | 上游与许可                                                                                                                                                                                                     |
| -------------- | ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `num-bigint`   | 有界任意精度整数   | [仓库](https://github.com/rust-num/num-bigint) · [MIT](https://github.com/rust-num/num-bigint/blob/master/LICENSE-MIT) · [Apache-2.0](https://github.com/rust-num/num-bigint/blob/master/LICENSE-APACHE)       |
| `num-rational` | 分数及有理数运算   | [仓库](https://github.com/rust-num/num-rational) · [MIT](https://github.com/rust-num/num-rational/blob/master/LICENSE-MIT) · [Apache-2.0](https://github.com/rust-num/num-rational/blob/master/LICENSE-APACHE) |
| `num-traits`   | 数值特征与类型转换 | [仓库](https://github.com/rust-num/num-traits) · [MIT](https://github.com/rust-num/num-traits/blob/master/LICENSE-MIT) · [Apache-2.0](https://github.com/rust-num/num-traits/blob/master/LICENSE-APACHE)       |

本机 Cargo 下载的 `num-bigint 0.4.8`、`num-rational 0.4.2`、`num-traits 0.2.19` 所附 `LICENSE-MIT` 完全相同；以下版权与完整许可正文分别适用于这三个组件，并随打包界面中的静态 HTML 注释一同保留：

```text
Copyright (c) 2014 The Rust Project Developers

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

其他 Cargo 直接依赖与传递依赖由锁文件固定，保留各上游包随附的许可；本文件不替代完整分发链的依赖许可证审查。

## ExchangeRate-API 在线汇率

货币数据来源于 AYR Tech (Pty) Ltd 运营的 [ExchangeRate-API](https://www.exchangerate-api.com)，使用其 [Open Access 端点](https://www.exchangerate-api.com/docs/free)。插件固定请求 `https://open.er-api.com/v6/latest/USD`，取得的数据用于终端用户的本地货币换算，并在货币界面保留“[Rates By Exchange Rate API](https://www.exchangerate-api.com)”署名链接。

汇率数据受提供商 [服务条款与隐私政策](https://www.exchangerate-api.com/terms) 约束，不属于本插件 MIT 许可证覆盖的内容。该开放端点要求署名，允许个人、商业换算与终端用户缓存，不允许将所取得的数据再分发为原始汇率服务。插件不预置提供商的汇率快照，不提供面向第三方的原始汇率分发接口。该服务每日更新并有请求限制，其汇率只用于信息与示例用途。
