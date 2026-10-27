# 第三方归属与许可

本插件将 Roblox 平台的 Slither 移植为离线 HTML / Canvas 游戏。原作署名、固定来源与完整许可在此保留，并内嵌于 [`ui/main.html`](./ui/main.html)，随扩展包分发。以下许可分别适用于相应代码、图像、音效与字体，不表示原作者为本插件背书。

## Slither

- 项目：[littensy/slither](https://github.com/littensy/slither)，作者 Littensy。
- 固定提交：[56545f14df4d25d3df7e71fbaeff7b21cf8798a7](https://github.com/littensy/slither/tree/56545f14df4d25d3df7e71fbaeff7b21cf8798a7)。
- 原始许可：[MIT License](https://github.com/littensy/slither/blob/56545f14df4d25d3df7e71fbaeff7b21cf8798a7/LICENSE.md)。
- 复用与移植范围：Catppuccin Mocha 色板；主菜单、渐变标题与开始按钮、皮肤选择、观战、HUD 和圆形小地图的布局与视觉；连续角度转向、分数驱动的蛇身增长与转弯速度、冲刺消耗和掉落、撞蛇判定、死亡战利品及机器人的寻食与避让规则；实际内嵌的原版 UI 与蛇身图像，以及下文列出的原版界面和游戏音效。
- 平台适配：将 Roblox UI、Vector2、RunService、输入、声音、摄像机和服务端状态更新改写为浏览器 Canvas、Pointer / Keyboard / Gamepad 与 Web Audio；以本地 AI 竞技场替代 Roblox 服务器连接，使用插件私有存储保存设置和纪录。Roblox 账号、付费商品、聊天、语音、在线多人和平台成就服务不属于本地移植范围。
- 原版背景音乐仅提供 Roblox 资源 ID，仓库不包含可独立分发的音乐文件；本插件不分发这些曲目。

以下为上游 MIT 许可原文：

```text
MIT License

Copyright (c) 2023-present Littensy

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the “Software”), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

## Catppuccin 色板

- 项目：[Catppuccin](https://github.com/catppuccin/catppuccin)。
- 采用 Slither 固定提交的 [src/shared/constants/palette.ts](https://github.com/littensy/slither/blob/56545f14df4d25d3df7e71fbaeff7b21cf8798a7/src/shared/constants/palette.ts) 中实际使用的 **Mocha** 色板。
- 原始许可：[MIT License](https://github.com/catppuccin/catppuccin/blob/d09787dd98ca6fba08af5ef2ae94a7e09f17daca/LICENSE)。

```text
MIT License

Copyright (c) 2021 Catppuccin

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Kenney Interface Sounds

- 作者：[Kenney](https://kenney.nl)。素材包：[Interface Sounds](https://kenney.nl/assets/interface-sounds)。
- 来源：Slither 固定提交的 [assets/sounds/](https://github.com/littensy/slither/tree/56545f14df4d25d3df7e71fbaeff7b21cf8798a7/assets/sounds)；上游 [音效表](https://github.com/littensy/slither/blob/56545f14df4d25d3df7e71fbaeff7b21cf8798a7/src/shared/assets/sounds/sounds.ts) 明确指向此素材包。
- 许可：[Creative Commons CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/)。Kenney 官方素材页明确标记 CC0。
- 复用范围：实际内嵌的开始、冲刺开始 / 停止、进食、死亡、按钮和切换音效；原始 Ogg 字节直接嵌入，Web Audio 仅在播放时调整音量和播放速度，不将合成音色冒充原版录音。

## Inter 字体

- 字体：[Inter](https://github.com/rsms/inter)，The Inter Project Authors。Slither 原版使用 Inter；本插件从 Google Fonts 官方发布源取得离线 Web 字体。
- 原始文件：[Google Fonts Inter v20 Latin variable WOFF2](https://fonts.gstatic.com/s/inter/v20/UcC73FwrK3iLTeHuS_nVMrMxCp50SjIa1ZL7.woff2)，可变字重 100–900，保留原始字节，不做二次裁剪、字形或命名修改；中文由系统字体补齐。
- 原始文件 SHA-256：`3100e775e8616cd2611beecfa23a4263d7037586789b43f035236a2e6fbd4c62`。
- 许可：[SIL Open Font License 1.1](https://github.com/rsms/inter/blob/353b61b9f4430d5f420d56605a6e7993e0941470/LICENSE.txt)。字体许可独立适用。

以下为字体版权声明与许可原文：

```text
Copyright (c) 2016 The Inter Project Authors (https://github.com/rsms/inter)

This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
http://scripts.sil.org/OFL

-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded,
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION AND CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```
