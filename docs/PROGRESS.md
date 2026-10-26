# Progress / continuation notes

## 2026-10-26 — v0.1.13 published; Store Submission 3 (1.0.13.0) in certification

- 用户要求无人值守发布（GitHub 与微软商店）。GitHub：已发布并设为 Latest：https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.13 ，标题“FeatherMD 羽读 v0.1.13 · 阅读纯文本与日志，按格式生成目录”，说明见 `dist/RELEASE-v0.1.13.md`。annotated tag v0.1.13 指向 **38d9f05603b4ff373a8273c739f4ecae7091ebcf**。附件：FeatherMD.exe 1,162,240 字节，SHA-256 `1ef9b16edc93be7f5436038c061c7f70f89c059fa4a550b5cf35ab8c794b23d7`；FeatherMD-0.1.13-windows-x64.zip 2,661,018 字节，`9073dae4ddbe36b8b1d0dd492ff6a0d947a39ff45fa4e15ece95813f5d50817c`（49 项）；SHA256SUMS.txt 181 字节。三者重新下载后与本地 `dist/release-0.1.13` 逐字节一致，并与 GitHub 资产 digest 一致。打标签前 ZIP 内 43 个非 EXE 条目与提交内容逐字节一致、5 个只差 CRLF（工作区 README / ARCHITECTURE），0 处真实差异；README 链接的 docs 文件全部入包。
- CI：c763219 的 Windows 失败来自 runner 的 cp1252：`invalid_bytes_are_flagged` 依赖系统代码页。修正 26844a0（`mostly_utf8` 改为 `bad == 0 || good >= bad`，测试不再依赖本机代码页，新增指定代码页的 Windows 测试）。CI 截图中 GBK 小说一步在目录完成前截图（显示“正在识别章节…”），38d9f05 改为在每种代码页都等目录完成，并检查非 936 系统按系统代码页打开、能找到 ASCII 章节（runner：1252，1 个章节）。main CI **40240155669** 与 tag CI **40240696533** 在 Windows / Ubuntu 均通过（Windows：129 / 131 项测试、21 项原生回归 PASS，含 formats-smoke 浅 / 深）。本机：fmt、两种配置 strict Clippy、129 / 131 项测试、17 项 Python 测试、release 与测试替身。**本机未运行任何原生窗口测试**（只剩一台显示器，按 AGENTS.md 不传 `-AllowDesktopUi`）。CI 截图（小说、日志、源码、编译日志、GBK）已实图检查。
- 商店候选：Supplemental Store WACK **40240292473**（正式身份，源代码 38d9f05）：总体 PASS，13 必选 PASS、10 可选 PASS、1 可选 Blocked executables FAIL（ShellExecuteW/ExW 与 `REG` 文本，同前；发布构建中唯一的 `Command::new` 在单元测试里）。冻结 `dist/store-1.0.13.0-candidate/FeatherMD-1.0.13.0-x64.msix`，574,736 字节，SHA-256 `34a7d6332b64cdee2ac89db1abecc7a9bf724e1dbd30c93e605b5c5e57a348e2`；包内 EXE 1,126,400 字节，`2b17aa39…`，含商店渠道标记。
- 生命周期：40240294648 通过，但只测了四个 Markdown 扩展名。扩展到七个后（243bbb1，运行 40241070658）在 .txt 失败：未创建阅读器进程。加入证据收集（8af899e，运行 40242268129）后查明：对 .txt（默认 Notepad）与 .text，Windows 用自己的“Select an app”选择框（OpenWith.exe，列出 FeatherMD）回应 Open With，而不是直接启动；.log 与 Markdown 类型直接打开。同一文件直接启动时以包身份、正确格式打开。UI Automation 看不到这个选择框（ffc1e95，运行 40243521048：根下只有任务栏、控制台与桌面），因此不点击，改为单独记录（ae28df6）：保留截图，并要求直接启动通过、每个声明的文件类型组至少有一个扩展名经 Shell 打开、默认值不变。运行 **40244480948** 通过：7 个扩展名都找到精确 AUMID 处理程序；.md / .markdown / .mdown / .mkd / .log 经 Shell 打开，.txt / .text 为选择框（截图 + 直接启动均通过）；UserChoice 前后一致（.txt 仍为 txtfile）；GUI、保留设置的升级、卸载与清理通过。该运行与候选的源代码只差 `packaging/test_lifecycle.ps1`。
- Partner Center：Submission 2（1.0.12.0）已通过认证并上架（概述“恭喜！你的产品现已更新”）。在专用 Edge 实例（`%LOCALAPPDATA%\FeatherMD-PartnerCenter`，Windows 账户自动登录、开启同步；未改扩展、收藏或设置；未读取用户自己的浏览器数据）中“开始更新”创建 **Submission 3（1152921505702229604）**。包：上传冻结的 1.0.13.0（Validated，只有 runFullTrust 受限功能提示），保存后 1.0.12.0 因更高版本移除。四种语言“此版本的新增功能”改为纯文本 / 日志说明，重新打开逐一核对一致。其他测试信息：改为 Markdown / 纯文本 / 日志阅读器，加入 txt / log 测试步骤、Open With 七个扩展名与 Windows 选择框说明、WACK 40240292473 与生命周期 40244480948 链接（3198 字符，重新加载后逐字节一致）；新增 `docs/store-samples/zh-CN-novel.txt` 与 `server.log` 供审核（公开地址 HTTP 200）。提交选项仍为“通过认证后立即发布”；runFullTrust 说明、定价和可用性、属性、年龄分级、商店说明与截图未改。
- 已点击“提交进行认证”：概述显示“正在认证更新”，提交完成、预处理进行中，“产品在通过认证后立即开始发布”。08:15（+0800）再看：预处理完成，**认证进行中**（第 3 步，共 4 步），发布未开始。这是已受理，不是审核通过或上线。证据：`test-artifacts/feathermd-submission3-20261026.png` / `.txt` 与 `feathermd-submission3-certifying-20261026.*`（页面右侧的满意度调查未作答）。之后关闭专用 Edge 实例（用户自己的 Edge 未受影响）并删除专用配置目录；没有设置自动监控。
- 限制：商店说明与截图仍只讲 Markdown（未在本轮修改）；GBK 解码只在 936 系统验证，CI runner 为 1252；Windows 选择框里选 FeatherMD 后的打开过程未自动化验证（文本组的 Shell 启动由 .log 证明）；Windows Server runner 不等于客户端 Windows 测试。

## 2026-10-24 — v0.1.13 candidate: plain text, logs and legacy encodings

- 用户：“上传完商店，做下一个需求：扩大阅读范围，txt、log大数据日志类，注意区分不同格式下的目录功能。”随后：“都开发完之后记得发布，无人值守哈”“我说的发布是连着微软商城一起哦”。
- 实现：`src/plain.rs`（按文件名 + 首页内容判定 Markdown / 纯文本 / 日志；日志时间戳 9 类、级别词；纯文本章节、下划线标题、公文编号；文本与日志的预览块）、`src/outline.rs`（目录按格式：Markdown 标题 16 MiB；纯文本章节 256 MiB；日志时间索引读完整个文件，时间段 1 分钟至 1 周、最多 240 段、按日分组、每段错误 / 警告数；无时间戳日志列错误行最多 1000；每请求一个进度计数）、`src/encoding.rs`（前 64 KiB 明显不是 UTF-8 时用系统旧代码页，936 → GB18030；按字符从行首切段，跨长行从不可续接字节后恢复；搜索按该编码并只接受字符边界匹配）。布局新增 `Kind::Text`（保留行首缩进）与 `Kind::Log`（等宽、无代码框）。界面：目录状态与进度、日志条目附错误 / 警告数并对有错误的条目标红、状态栏显示编码与格式、打开对话框与欢迎页、文件关联（绿色版打开方式与商店包清单加入 .txt / .text / .log，仍不设默认）。测试接缝查询 100–103。
- 验证（MSVC）：fmt、两种配置 strict Clippy、**128 / 130** 项测试（新增 tests/plain.rs、tests/encoding.rs、outline 内部测试 5 项、真实 GDI 排版测试），17 项 Python 打包测试，release 与测试替身。基准（暖缓存，见 PERFORMANCE §9）：1 GiB 日志时间索引 1851 ms，25 MB 小说章节 21.6 ms，8 MB GBK 小说 74.0 ms 且识别为 GB18030。
- 新增 `scripts/formats-smoke.ps1`（浅 / 深）并加入 CI 与 AGENTS.md 发布前测试。**本机未运行原生窗口测试**：此时只剩一台显示器（`\\.\DISPLAY2` 成为主屏），`-SecondaryMonitor` 按设计拒绝启动，按 AGENTS.md 不自动传 `-AllowDesktopUi`。全部原生回归（含新 formats-smoke）以 GitHub Windows CI 隔离 runner 的结果为准，结果另记。
- 版本 0.1.13 / MSIX 1.0.13.0。发布流程：提交推送、CI、标签、GitHub Release；商店在 Partner Center 处理 Submission 2（1.0.12.0）后以 1.0.13.0 更新。

## 2026-10-24 — Store Submission 2 (1.0.12.0) submitted for certification

- 用户：“你全权操作，不用和我确认”。本会话没有浏览器工具；无头 Playwright 被 Partner Center 的 Akamai 拒绝（Access Denied）。改为另起一个 Edge 实例：专用用户数据目录 `%LOCALAPPDATA%\FeatherMD-PartnerCenter`、仅本机 DevTools 端口、窗口在副屏。Edge 用 Windows 账户自动登录 Microsoft 帐户并开启同步，拉入了用户的扩展（自动打开扩展欢迎页）。没有读取或复制用户自己的 Edge 或其他应用的 Cookie / 密码，也没有在该配置里改扩展、收藏或设置（避免同步回其他设备）。用 Playwright 经 CDP 逐步操作，每步截图与无障碍树核对。
- 从概述“开始更新”创建 **Submission 2（1152921505702218732）**，各部分复制自 Submission 1。**包**：上传冻结的 `FeatherMD-1.0.12.0-x64.msix`（SHA-256 9c4eb44c…），显示 Validated，只有 runFullTrust 受限功能提示（与 1.0.10.0 相同）；保存后 1.0.10.0 因有覆盖同一客户群的更高版本而移除。**Store 一览**：四种语言的“此版本的新增功能”（表格按字符选择、隐私按钮间距）已填写，重新打开逐一核对；说明、截图、功能列表、简短描述未改。**提交选项**：发布暂缓改为“在此提交通过认证后立即予以发布”，重新打开确认；runFullTrust 说明未改。**其他测试信息**：认证说明中的 WACK / 生命周期链接改为 40048542912 / 40048545728，加一句表格选择说明，重新打开逐字节核对（该文本框需用原生 setter 并触发 input / keyup / change / blur，Angular 才登记修改）。定价和可用性、属性、年龄分级未改。
- 已点击“提交进行认证”：概述显示“正在认证更新”，进度为提交完成、预处理进行中、认证与发布未开始，并注明“产品在通过认证后立即开始发布”，提供“取消认证”。Submission 1（1.0.10.0）仍是商店现有版本。这是已受理，不是审核通过或已上线。证据：`test-artifacts/feathermd-submission2-20261024.png` / `.txt`。
- 之后关闭专用 Edge 实例（用户自己的 Edge 未受影响），删除专用配置目录。没有设置自动监控；认证结果需之后在 Partner Center 或邮件中查看。

## v0.1.12 publication record and Store 1.0.12.0 candidate — 2026-10-24

- GitHub：已发布并设为 Latest：https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.12 （2026-10-24T01:19:49Z），标题“FeatherMD 羽读 v0.1.12 · 设置页隐私按钮间距”，非草稿、非预发布；说明见 `dist/RELEASE-v0.1.12.md`。源代码 **6fe4f38538ba33b209713f10ad8317f6adab4021**，annotated tag v0.1.12 指向它。main CI **40048137266** 与 tag CI **40048749172** 在 Windows / Ubuntu 均通过（Windows：112 / 114 项测试、19 项原生回归）。推送 main 与标签各遇一次 GitHub 443 连接超时，重试成功，未改目标、未强推。
- 附件：FeatherMD.exe **1,121,792** 字节，SHA-256 **0eee742fa06152b2a78fb5525b7f69fee62aefe15a5ab76809461b9d7c93dcfd**；FeatherMD-0.1.12-windows-x64.zip **2,632,254** 字节，SHA-256 **18784abe5cb31f59cb6cb73b4c2f65db7eb0d2fdd14b47cf3ca1037faf4cc0f3**；SHA256SUMS.txt **181** 字节，SHA-256 **0c7b3a5b51d0b0bb3c1956e3bfdc51ba385db6b95cd881a9c147a463282b89b4**。重新下载后三者与本地交付逐字节一致，校验清单通过；ZIP 49 个文件中 44 个与标签逐字节相同，四份 README 仅 CRLF / LF 不同。releases/latest 返回 v0.1.12；说明图片 v0.1.12/docs/settings-update.png HTTP 200 且与标签相同；v0.1.11 附件 digest 未变。交付 EXE 在副屏上 update 浅 / 深与 i18n 各一次 PASS、0 违规。本地交付 `dist/release-0.1.12/`。
- 商店候选：正式身份 WACK **40048542912**（源代码 6fe4f38，Windows Server 2025 26100，WACK 10.0.26100.9457）退出 0、总体 PASS，**13 必选通过、10 可选通过、1 可选 Blocked executables 失败**（ShellExecuteW/ExW 与 REG 文本，与 1.0.10.0 相同，审核说明已有解释）。生命周期 **40048545728**（测试身份的安装、四扩展名打开方式、升级、卸载）通过。冻结 `dist/store-1.0.12.0-candidate/FeatherMD-1.0.12.0-x64.msix`，**552,862** 字节，SHA-256 **9c4eb44c631a0785628b405675449099ecaec6bd2622333ce5138d134a384ed1**，身份 ice.FeatherMD、非测试身份、商店通道标记在内；包内 EXE 1,086,464 字节，SHA-256 31b31ef6df0b53201927d5b1e5ff72cd122709f7b7ff1db55fe9ed103987bdda。报告原件在被忽略的 `test-artifacts/cloud-wack-40048542912/`。
- Partner Center：本会话没有可用的浏览器工具；用专用的 Playwright Edge 配置（`%LOCALAPPDATA%\FeatherMD-PartnerCenter-Automation`，不是用户自己的浏览器配置，也未读取其他应用的会话）无头打开产品页，被 Partner Center 的 Akamai 返回 Access Denied。没有读取或复制用户 Edge / 其他应用的 Cookie。上传与提交待在同一专用配置中由用户登录一次后继续，结果另记。

## 2026-10-24 — v0.1.12 release candidate: privacy button spacing, Store 1.0.12.0

- 用户：“都更新，商店也更新”。按此在副屏重新生成截图时发现：设置 → 更新页的“隐私说明…”按钮上贴隐私卡片、下贴页脚横线（像素测量 0 px 间距），四种语言与商店版（线上 1.0.10.0 同样）都有；商店版两行说明之间还有一段空白。v0.1.11 / Store 1.0.10.0 均已带此问题发布。询问后用户选择“发 v0.1.12（推荐）”，商店上传与提交选择“你全权操作，不用和我确认”。
- 修复（`src/ui/preferences.rs`）：两种版本的隐私卡片标题、说明与按钮由同一处生成（`privacy_notes()`），卡片高度按行数计算（`privacy_card`），从 322 开始、说明行距 24；按钮上方与页脚横线之间各至少 8 px（`PRIVACY_GAP`，横线 `FOOTER_RULE` = 484）。新增单元测试检查两种版本的卡片位置、文字在卡片内、按钮不碰卡片与横线。
- 验证：两种配置 fmt、strict Clippy、**112 / 114** 项测试、17 项 Python 打包测试、release 与测试替身通过。副屏 `-SecondaryMonitor`（120 DPI）：update 浅色 / 深色、i18n（四种语言 105 个视图无截断、8 次实时切换，GDI 28→28、USER 16→16）、store（8 个商店场景）均 PASS、0 违规；实图检查卡片、按钮与横线间距正常。
- 截图：用 0.1.12 候选构建在副屏重截 About 浅 / 深与繁中 / 英 / 日（版本徽章 v0.1.12，旧图里多余的键盘焦点框也没有了）和 `settings-update.png`（新按钮与更正后的说明）。其余设置 / 阅读截图与新截图的差异只有快捷键下划线或焦点框，内容未变，保留原图；`settings-reading.png` 新截图是皮肤预览且勾选了显示图片，不代表默认状态，保留原图。
- 商店：1.0.10.0 已于 2026-10-20 上线（公开目录与商店页确认）。1.0.11.0 候选的 WACK（40044912773）与生命周期（40044915701）通过但未上传；改为 **1.0.12.0** 对应 Cargo 0.1.12。商店提交与发布结果另记。

## v0.1.11 publication record — 2026-10-24

- 用户：“跑吧，不用改了，跑完了没问题直接提交发布”。已发布并设为 Latest：https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.11 （2026-10-24T00:08:31Z，北京时间 08:08），标题“FeatherMD 羽读 v0.1.11 · 表格文字按字符选择”，非草稿、非预发布；说明见 `dist/RELEASE-v0.1.11.md`。
- 源代码 **75df2753dac7ba5f0bf6ef7522b13083304f7c2e**，annotated tag v0.1.11 指向它。main CI **40042221544** 与 tag CI **40042932590** 均在 Windows / Ubuntu 一次通过：Windows 上 111 / 113 项测试、17 项 Python 打包测试和全部 19 项原生回归（96 DPI，1024×720；table-smoke 浅 / 深的新表格选择段含长单元格拖选均为 true）。推送与标签推送都直接成功，没有强推。
- 附件：FeatherMD.exe **1,121,280** 字节，SHA-256 **f9321154da4aaa05a0edb9ba2c462ac6ee5c4a3ff62cb745be4044f5388b149d**；FeatherMD-0.1.11-windows-x64.zip **2,631,084** 字节，SHA-256 **b7aee6db302c0e9873fea2c08d58f7b412e2bd94e5f85d5f8c22ef7f103f2c48**；SHA256SUMS.txt **181** 字节，SHA-256 **0eb8807cbb1a7b594d2c87159fe60302ebeae3c959941f4f7309c0b578d092e3**。三个附件重新下载后逐字节与本地交付一致，与 GitHub asset digest 和校验清单相符。EXE 版本 0.1.11，与按提交树 `--locked` 重新构建的 target/release 相同。
- ZIP 共 49 个文件：44 个与标签源码逐字节相同，四份 README 仅 CRLF / LF 不同（与 v0.1.10 相同），新增 `docs/table-selection.png` 与 `docs/PRIVACY.md`。说明中的图片地址 v0.1.11/docs/table-selection.png 返回 HTTP 200，与标签中的文件相同。releases/latest 返回 v0.1.11；v0.1.10 三个附件的 digest 未变。
- 交付 EXE 在副屏（`-SecondaryMonitor`，120 DPI）上 table-smoke 浅色 / 深色各一次 PASS，各 5 个测试进程、0 违规；深色实图同样检查了选区、横线与行内代码。证据在被忽略的 `test-artifacts/table-cells/final-light`、`final-dark`。实网 GUI 更新检查未运行（未获窗口测试以外的许可）；改用临时示例以发布的解析代码读取实际 latest 响应：v0.1.11、发布日期 2026-10-24，比 0.1.10 新、不比 0.1.11 新，示例已删除。
- 本地交付：**dist/release-0.1.11/**。正在运行的旧版 `dist/FeatherMD.exe` 与旧包未改动。遗留：About 截图仍显示 v0.1.10、`settings-update.png` 未含“隐私说明…”，需另行获准运行 about / settings smoke 后重新生成。Store 提交的 1.0.10.0 候选不含此版修改；本轮未查看 Partner Center。

## 2026-10-24 — v0.1.11 release candidate: table cells select by character

- 用户反馈：“表格里的文字，我都不能复制啊……选中文字什么的”。根因：v0.1.9 的预览选择把表格一行当作一个整体，命中只分左半 / 右半两个位置，在同一单元格里拖选两端落在同一半，选区为空；跨过中线则整行高亮、复制整行。
- 改为与段落一致的按字符选择：表格行以“显示的单元格 + 制表符”计数（`TableLine::text` / `cells`），每个视觉行记录各段文字在其中的起点（`starts`），因此单元格自动换行、窗口缩放后选区不变。命中只在指针所在列的单元格里取最近位置（表格左侧为行首、右侧为行尾，该视觉行没有文字的单元格取其末尾），拖选不会跳到相邻列；部分选择复制所选文字，跨单元格加制表符、跨行加 CRLF，整行仍复制原单元格的制表符分隔文本；双击只选单元格内的一个词。选区底色不覆盖单元格上下留白，行间横线保持可见。顺带修正：双击段落或单元格最后一个字的右半边原本什么也不选，现在选中该词。
- 静默验证（MSVC）：fmt、两种配置 strict all-targets Clippy；测试 **111 / 113** 项全部通过（新增 4 项表格选择测试：换行单元格内复制、跨单元格 / 跨行、各视觉行绘制范围、命中不跨列、双击不出单元格，以及在内存 DC 中用真实 GDI 测量排版一张会换行的表格，逐段核对选择文本、每个边界经命中测试往返不变、换行处空格复制时补回；扩充双击末字与选区对比度断言）。故意把段起点改错时真实排版测试失败，确认断言有效后已恢复。选区底色在 20 款皮肤的正文底色上实测 1.59–2.20:1、表格斑纹行 1.51–2.03:1，测试要求 ≥ 1.4:1（与源码行同一下限）。release 与两个测试替身构建通过：`target/release/feathermd.exe`（绿色版）1,121,280 字节，SHA-256 `d48b15a4bc519bae378ef26b53d87991455a65e1d6c5fe26d200902beda81cc7`（每次构建都会重新链接，哈希只对应这一次构建）。`dist/` 未改动。
- table-smoke 新增表格选择段：单元格内拖选、从单元格留白拖入、跨单元格（制表符）、跨行（CRLF）、长单元格内拖选、双击选词，并截图 `table-selection` / `-rows` / `-cell`。首次运行前复查发现留白拖选缺少鼠标移动（松开鼠标不移动选区终点），已补上。
- 用户随后答复“跑吧，不用改了，跑完了没问题直接提交发布”：只授权副屏运行 table-smoke，不改预览光标。副屏（`\\.\DISPLAY2`，2560×1600，工作区 3840,0–6400,1560，120 DPI）上用 `-SecondaryMonitor` 运行：0.1.10 版本号的开发构建浅色一次 PASS；升至 0.1.11 后的候选构建浅色一次 PASS（含长单元格段），两次各 5 个测试进程、边界 / 前台违规均为 0。实图检查：选区只覆盖所选文字，表头上方横线、表头与正文间横线可见；跨行选区里行内代码 `Ctrl` 的选区盖在代码底色上，未选的 `+F` 保留代码底色。新增 `docs/table-selection.png`（候选构建浅色 `table-selection-cell`，1350×1025，与其他文档截图同为 120 DPI），四语 README 引用并打包。
- 版本升至 **0.1.11**（Cargo/Cargo.lock、Windows FileVersion/ProductVersion、package 默认版本、四语 README 体积 1.07 MiB）。v0.1.10 之后 main 上未发布的 Store 适配也一并进入此版：设置“更新”页的“隐私说明…”（四语，离线显示）、更新页隐私说明更正（检查更新与网络图片分开控制）；绿色版下载不含 Store 构建。README 中“关闭检查更新就不联网”的旧说法同步更正。CHANGELOG 0.1.11 记录以上内容。
- 本轮只获准运行 table-smoke，**没有运行其他 18 项原生回归、也没有重新生成截图**：About 截图（`about-*.png`）的版本仍显示 v0.1.10，`settings-update.png` 未显示新的“隐私说明…”按钮。其余原生回归以推送后的 GitHub Windows CI（隔离 runner）结果为准，结果另记。
- 发布前状态：未提交、未推送、未发布。Store 提交的 1.0.10.0 候选（冻结 MSIX）不包含此修改，也没有改动；本轮未查看 Partner Center，不推断其当前审核状态。商店用户要获得此修改需要后续新版本再提交。

## 2026-10-18 — Submitted to Microsoft Store certification

- 用户对“正式送审 FeatherMD 1.0.10.0、全球免费、通过后暂不自动发布”的当时确认请求回复“确认”。已点击**提交进行认证**，并核对返回状态，不是只记录点击尝试。
- Partner Center 产品 **FeatherMD / 9MW2CTPBSGX7 / Submission 1 / 1152921505702158375** 已从草稿变为**正在认证**。进度：**提交完成、预处理正在进行、认证未开始、发布未开始**；页面出现“取消认证”。这表示提交已受理，不表示审核通过或已经上架。
- 页面明确显示“**单击‘立即发布’时，产品将开始发布**”，手动暂缓发布保持有效。已保存的全球 240 个现有市场、免费定价、四语 16 图、实际 IARC 结果和审核说明一并送审；冻结 MSIX 未重建或替换，仍为 SHA-256 `ba17aec5ba13880c794e767640104f8ae41771da0a60b751f9f013c7538cb80f`。
- 页面预计认证通常数小时，某些情况最多 3 个工作日；这是微软显示的预估，不作通过或完成时间保证。等待真实审核结果；不得为补文档取消认证、重复提交、改包或提前发布。通过后正式发布仍需用户当时确认；若被拒，先读取实际报告再修复。
- 受理证据：`test-artifacts/feathermd-submitted-20261018.txt`、`test-artifacts/feathermd-submitted-20261018.jpg`。后台状态页保留，无本机原生窗口、安装或证书操作。旧 GitHub Release 和 LiteView 未修改；没有创建自动监控或声称将自行收到后续结果。

## 2026-10-18 — IARC saved; certification submission enabled (not submitted)

- 用户在明确的成年年龄与 IARC 条款确认问题后回复“你点就行了”。按此确认勾选声明并保存；页面从“年龄分级预览”变为“年龄分级”，新增“继续”，当前分级 ID 显示**待定**，未编造证书编号。既有 IARC 10.3 评级结果不变。
- 点击“继续”返回 FeatherMD 总览后，**“提交进行认证”按钮已启用**（UI 和 `isEnabled()` 均确认），产品仍显示**处于草稿状态 / 正在草拟**。年龄分级和定价两行仍未显示“完成”文字，不能据此再断言无法提交，也不编造已查明的页面显示原因。全球 240 市场免费、手动暂缓发布等已保存选项未改动。
- **尚未点击最终“提交进行认证”或发布**。下一步向用户确认把冻结的 `1.0.10.0` MSIX 正式送微软审核，全球免费，通过后仍手动发布；得到当时确认后再操作，不再次询问已经完成的 IARC/成年声明。
- 证据：`test-artifacts/feathermd-certification-ready-20261018.txt` 和同名 `.jpg`。未重建或替换 MSIX，未运行本机窗口/安装/证书测试，既有 WACK 可选告警、Windows 10/辅助功能及第三方付款流程验证边界继续保留。

## 2026-10-18 — IARC preview generated; adult/terms attestation pending

- 用户在明确的数据共享/生成并保存评级确认请求后回复“继续”。据此点击 FeatherMD 自身问卷的“预览分级”，没有修改既定答案，也没有使用 LiteView 的评级。
- IARC **10.3** 返回实际预览：IARC / Microsoft Store / PEGI **3+**，ESRB **E（所有人）**，USK **Everyone**，巴西 **L**、智利 **TE**、俄罗斯 **0**。这是问卷生成结果，不是商店内容/技术认证通过。
- 保存前页面新增复合声明：同意 IARC 使用条款，并确认已达到所在地法定成年年龄。此框保持**未勾选**，保存按钮禁用；没有替用户推定年龄，没有保存最终评级、提交认证或发布。下一步只需本人确认此声明，再勾选保存并复核实际总览状态，不重复询问已经同意的数据共享。
- 定价页重新读取仍为全球 **240** 个市场、**CNY 0**、公开可发现；没有出现价格/时间表验证错误，保存按钮因无改动禁用。既有总览完成标记问题仍待评级保存后复核，不推测为已解决。
- 证据：`test-artifacts/feathermd-iarc-preview-20261018.txt`、`test-artifacts/feathermd-iarc-age-attestation-20261018.jpg`。两个后台标签页保留；没有本机原生测试、安装、证书操作或包替换。

## 2026-10-18 — Store submission draft populated; IARC consent pending

- 用户已确认创建商店提交草稿、上传正式候选和公开资料。FeatherMD **9MW2CTPBSGX7 / Submission 1 / 1152921505702158375** 已创建，仍为 **正在草拟**；没有点击“提交进行认证”，没有发布。用户本轮最终澄清为 **全球发布，不是仅中国区**。LiteView、旧 GitHub Release 和用户正在运行的程序未修改。
- 已上传冻结的 `dist/store-1.0.10.0-candidate/FeatherMD-1.0.10.0-x64.msix`，Partner Center 显示 **Validated / 完成**。本轮重新核对 SHA-256 仍为 **ba17aec5ba13880c794e767640104f8ae41771da0a60b751f9f013c7538cb80f**；不是重建包、测试签名副本或旧 `store-production-1`。仅选择 Desktop，不允许自动扩展到未来设备系列。`runFullTrust` 仍需微软批准，不能将上传校验称为认证通过。
- 属性已保存并显示完成：生产率分类、公开隐私/网站/Issues 支持地址，申报第三方赞赏；不虚报完全无个人信息或无障碍达标。隐私 URL 于本轮再次验证 HTTP 200。未提交私人地址、电话、支付凭据或历史 API 密钥。
- 简体中文、繁体中文、英文、日文四个 Store 一览均显示完成，最终各 **4 张真实截图、5 条功能说明**。截图来自已成功的隔离采集 **39047863992**，使用正式 WACK run **39045802702** 的冻结 EXE，不重新编译、不合成 UI；16 张均为 1600×1000，哈希及来源见 `test-artifacts/store-listing-39047863992/provenance.json`。采集脚本/示例文档提交为 `3f6937d`、`201898b`。首轮采集 **39047719721** 因 runner 默认显示分辨率不足失败，后续仅调整隔离 runner 分辨率后通过，未改用户桌面。
- 定价草稿已保存：**全球全部 240 个现有市场、CNY 基价 0、公开且可发现、无试用或促销**；逐行核对市场价格表 240 行均为零，包含中国。未来新增市场自动扩展关闭。重新打开页面仍保留这些值，但总览的“定价和可用性”**尚未显示完成**；不要猜测根因或称全部提交项已就绪。市场列表将中国标注为“需要 ISBN”，未据此编造 ISBN、移除中国或把非游戏应用判为已获当地许可；完成 IARC 后应复核适用要求和状态。
- 提交选项已保存并显示完成：**除非手动选择立即发布，否则不发布**；填写 454 字符的 `runFullTrust` 说明，解释原生 Win32/GDI、用户打开的文件和所选编辑器、中完整性级别、无管理员/服务/抢默认、逐图确认与 Store 更新路径。认证人员的实际测试步骤、无需登录/付费、可选赞赏、WACK 的 **13 必选通过 / 10 可选通过 / 1 可选失败** 及 ShellExecute/REG 说明已保存，重新加载确认存在。
- FeatherMD 自身的 IARC 问卷已在浏览器中填写：其他所有应用类型；包内敏感内容、原生用户交流、推荐联机内容、年龄限制商品、共享精确位置、应用内数字商品、可兑换奖励、浏览器/搜索引擎、主要新闻/教育产品和实体分发均按实际行为选否。数字商品帮助明确指应用内购买；未将可选浏览器赞赏误报成付费功能。**未点击“预览分级”、未生成/保存评级**，因为页面明确将发布者显示名及联系邮箱共享给 IARC，待用户对此步骤确认；不是沿用 LiteView 证书。草稿答案保留在后台标签页和被忽略的 `test-artifacts/feathermd-iarc-draft-20261018.txt`。
- 只读检查公开爱发电页面：展示支持创作的赞赏方案，未看到 FeatherMD 功能解锁说明。未登录、未进入实际付款、未核验支付安全资质，因此不称第三方付款链路已审计通过。证据 `test-artifacts/feathermd-support-public-20261018.jpg`。
- 本轮全部网页操作在后台；没有本机原生窗口、安装或证书测试。证据包括 `test-artifacts/feathermd-store-overview-20261018.jpg`、`feathermd-global-market-draft-20261018.jpg`、`feathermd-iarc-consent-pending-20261018.jpg`。下一步：获得 IARC 数据共享/生成评级确认，处理实际评级结果并复核定价状态、地区条件和支持支付边界；最终提交认证、发布仍需当时确认。真实 Windows 10、完整辅助功能、Store 签名交付等既有验证限制保留。

## 2026-10-17 — Cloud checks complete; production Store candidate frozen (not submitted)

- 用户确认的名称预留、Git 提交/推送、云端验证均已完成。产品 **FeatherMD / 9MW2CTPBSGX7** 仍为 Partner Center 草稿；没有点击“开始提交”、上传包、填写评级或提交审核。LiteView 和现有 GitHub Release 均未修改；本机未运行原生窗口、安装或证书信任测试。
- 已推送 `c4abc2f`（Store 适配）、`47a0ec0`（WACK）、`0851b6b`（正确 Shell 调用测试）、`fc67956`（正式身份 WACK）。最终普通 CI **39045802061** 在 Windows / Ubuntu 全部成功，Windows 真实焦点和原 19 项原生回归通过；本机 107/109 项应用测试、17 项 Python 打包/报告测试记录仍有效。工作流 Node 20 迁移警告是后续维护项，不影响本轮结果。
- 生命周期对照 **39045655691** 成功：包清单不变，改用 `IAssocHandler::Invoke` 后，精确匹配测试包 AUMID，`.md` / `.markdown` / `.mdown` / `.mkd` 四种中文带空格文件均正确打开并保留包身份；安装后的四语 × 浅深 GUI 通过，升级保留 INI，卸载和临时证书清理通过。这确定了先前失败来自测试激活路径，而不是用改清单、直接启动 EXE 或抢默认关联掩盖问题。原失败 **39044834008** 保留。
- 正式身份 WACK **39045802702**，源代码 `fc67956e58d54b76d386eb8212417d2c5b651567`：确认包名 `ice.FeatherMD`、`test_identity=false`；原包和签名测试副本的 EXE/manifest 完整性通过。WACK 10.0.26100.8249，退出 0，完整总体 PASS，**13 必选通过、10 可选通过、1 可选 Blocked executables 失败**，未跳过，无清理错误。可选信息仍为 ShellExecuteW/ExW、REG 文本，解释见上一节；不称为无告警或微软已认证。
- 冻结交付路径：`dist/store-1.0.10.0-candidate/FeatherMD-1.0.10.0-x64.msix`，**551,084 字节**，SHA-256 **`ba17aec5ba13880c794e767640104f8ae41771da0a60b751f9f013c7538cb80f`**。包内 EXE SHA-256 **`c41056040d245f4e2a2afde62ff954514d48c01b92803684c65d22f0ba594c10`**。这是从正式身份 WACK run 下载后逐项核验并复制的原始未签名包，包含报告和哈希；不含测试证书、私钥、INI 或签名测试副本。后续优先上传此文件，勿用不同字节的本地早期候选代替。
- 隐私 URL `https://github.com/Lianzy-Baimiao/FeatherMD/blob/main/docs/PRIVACY.md` 已验证 HTTP 200，内容 blob SHA `7c912fb145e31c214024a1c79b7b4d4cdf769a0b` 与提交一致。完整证据在 `test-artifacts/cloud-ci-39045802061/`、`cloud-store-39045655691/`、`cloud-wack-39045802702/` 及相应 GitHub Actions artifacts；预留/下一步入口截图 `test-artifacts/feathermd-store-draft-ready.jpg`。
- 下一步需用户确认创建商店提交草稿、上传上述正式候选和公开资料，再完成 FeatherMD 自身的 IARC、市场、第三方自愿赞赏申报和受限能力说明。正式提交审核需要当时确认。隔离 Server 检查不等于真实 Windows 10 验证、完整辅助功能验收或 Store 签名/交付；这些限制必须如实保留，不能把产品预留称为已经上架。

## 2026-10-17 — FeatherMD name reserved; cloud validation authorized

- 用户确认预留名称、提交推送本次适配并启动云端验证。Partner Center 已成功创建独立 **FeatherMD** 产品，Store ID `9MW2CTPBSGX7`，状态为草稿；没有开始商店提交或修改 LiteView。
- 实际产品标识页确认包名 `ice.FeatherMD`、PFN `ice.FeatherMD_rz8m2sa64m4em`；完整发布者字段已存入被 Git 忽略的 `packaging/store-identity.local.json`。这是逐项核对 FeatherMD 页面得到的值，不是从 LiteView 复制。预留成功截图：`test-artifacts/feathermd-name-reserved.jpg`。
- 本轮将推送适配和测试保护，执行普通 Windows/Ubuntu CI 与手动 Store 生命周期验证；执行结果另记，不将成功派发当作通过。WACK 尚待补齐。所有真实安装/信任/窗口测试在隔离 runner 上，本机不再运行。
- 适配已提交为 `c4abc2f`；直接连接 GitHub 的推送连续两次超时，尚未据此声称推送成功。另补独立手动 WACK workflow：仅隔离 Windows 2025 runner、测试身份、临时非导出签名、固定哈希且 Microsoft 签署的 SDK、原包完整性和报告保守判定。新增报告测试后 Python 共 **17 项通过**，PowerShell 语法检查通过；本机未执行 WACK。云端结果另记。
- 使用本机已经配置的系统代理进行单次 Git 推送（未修改全局 Git 或系统设置）后，`c4abc2f` 和 WACK 提交 `47a0ec0` 已成功推送。普通 CI **39044833108** 在 Windows / Ubuntu 全部通过，含真实焦点原生回归。隐私页面 HTTP 200，公开内容 blob SHA 与本地提交一致。
- Store run **39044834008**：构建、两版测试 MSIX、四语 Preview GUI 和安装成功，但 `IApplicationActivationManager::ActivateForFile` 返回不支持契约，生命周期未通过；清理完成。按 Microsoft 文档，该 API 请求 `Windows.File` 事件，而本程序接收桌面命令行参数。只改测试为真正“打开方式”的 `SHAssocEnumHandlers` + 精确 AUMID/EXE 匹配 + `IAssocHandler::Invoke`，不改包清单或默认关联；本机仅编译 interop 和解析脚本，等待云端对照。研究见 `docs/STORE-ACTIVATION-RESEARCH.md`。
- WACK run **39044836701**：工具 10.0.26100.8249，退出 0、完整报告总体 PASS，**13 必选通过、10 可选通过、1 可选失败**；没有跳过/未知测试，清理无错误。可选 Blocked executables 指向 ShellExecuteW / ShellExecuteExW 和字符串 REG：源码中的 Shell 调用用于用户确认后的外链/编辑器/系统入口，`reg` 是 HTML `&reg;` 实体映射，未发现 reg.exe 启动代码。保留原始报告，不称为“所有测试通过”。这是测试身份的 Windows Server 补充检查，不是正式包已认证。证据 `test-artifacts/cloud-wack-39044836701/`。
- 本地正式身份候选 `dist/store-production-1/FeatherMD-1.0.10.0-x64.msix` 已通过 MakeAppx/内容校验，SHA-256 `76a6f399076cec8d09b9a0331e2f5af5daab092fb52b5d30e5c3f83a574000c1`，仍未签名/安装/上传。候选不是上述 WACK 原包，不混称为逐字节验证过的同一包。

## 2026-10-17 — Secondary-monitor regression complete; Store reservation pending

- 用户明确允许必要窗口测试在副屏进行，主屏继续打游戏。新增 `-SecondaryMonitor` 路径，不使用本机 `-AllowDesktopUi` 绕过保护：EXE 必须包含测试标记，UI 线程 CBT hook / 窗口子类在显示前限制窗口位置、阻止激活；副屏发现或保护初始化失败直接退出，不另弹错误框。不注入全局键鼠，不改剪贴板，不打开系统设置或外部浏览器。
- 最终同一轮副屏套件 **20/20 PASS**，证据 `test-artifacts/secondary-verified/results.json`。包括原 19 项回归和 Store 新套件（四语 × 浅深 8 个场景）；20 份 `test-host.txt` 共 69 个完成进程报告，边界/前台违规均为 0。DPI-aware 副屏工作区为 `3840,0–6400,1560`、120 DPI，主屏未作为测试目标。导航 headings 416/494、wrapped 251/280；i18n 105 个视图、8 次实时切换无截断；Store 设置隔离、禁用 GitHub 更新、无绿色版下载控件、离线隐私、About 和中文/空格路径通过。
- 副屏模式是 **passive-secondary**，不能验证真实键盘焦点；About 的 Tab/Enter 部分明确记录 `keyboard focus deferred` 和 `tab_and_enter_open_confirmation=false`，不是已经通过。真正激活窗口的焦点测试保留在普通 GitHub Windows CI。安装、信任证书、Shell 激活、升级卸载仍只允许隔离 runner。
- 保留失败证据而不覆盖：首轮 `secondary-final` 为 15/20（导航两项、About 两项、editor 未通过）；修正测试适配后 `secondary-recheck` 为 6/6，再跑上述完整 20/20。合成消息测试不再读取游戏期间物理 Ctrl/Shift；About 不用禁止激活的模式冒充焦点覆盖；editor 补齐必需快照环境；Store 脚本修正 PowerShell 空标题筛选。旧 footer-tooltip 本轮通过，未据此声称已找出历史间歇问题根因。
- 最终静默验证：MSVC fmt；绿色版/Store strict all-targets Clippy；**107 / 109 项测试**；两种静态 CRT release；**5 项 Python 打包测试**；全部 PowerShell 语法检查；生命周期 COM/native C# 绑定仅编译、未执行；`git diff --check` 通过。测试后无 target 测试进程或临时 INI 遗留，用户 dist 程序未关闭。
- 新未签名测试包 `dist/store-test-2/FeatherMD-1.0.10.0-x64-TEST.msix`，552,470 字节，SHA-256 `28de1034946b5c288d940a9c76313e37a24d8580a381670f9104179a2dd188e2`；MakeAppx 10.0.22000 schema 与逐字节包内容校验通过。Store EXE 1,082,880 字节，SHA-256 `a247d316af15900e027f45ddf31a8e7e1b2960733f03060cb5139e9aa22c8cc3`；绿色版候选 1,118,208 字节，SHA-256 `193b87a8403bd0b6a5f7c10e84ab27aea0a5668ff192b4d3777bdde77187f9ab`。已发布 v0.1.10 EXE 仍为 `c594dceb6bd76b66c6060b0acff1afdb5b024172d82123ed17b46d8bdb86b86b`，未覆盖。
- 已在后台内置浏览器中使用现有 Partner Center 会话检查名称，**FeatherMD 名称可用，但未点击“保留产品名称”**；截图 `test-artifacts/feathermd-name-available.jpg`。LiteView 未修改。待用户确认后创建 FeatherMD 独立产品、读取实际包身份，并提交/推送代码启动隔离验证。
- 仍未执行：GitHub 上本轮完整 CI、已编写的安装/实际 Shell 四扩展名激活/升级/卸载 workflow、WACK、真实 Store URI 打开、Windows 10 和完整辅助功能验证。当前 Store workflow **不含 WACK**。未签名、安装、提交 Git、推送、预留产品或提交商店；隐私 URL 尚未发布核验。不能将此记录当作提交就绪或保证过审。

## 2026-10-17 — Store adaptation, local test candidate (not published)

- 用户同意开始 FeatherMD 商店适配。未预留名称、登录 Partner Center、提交审核、提交 Git、推送或修改旧 Release。Cargo/UI 版本仍为 0.1.10；单独增加 Store feature 和 MSIX 版本。旧 dist 发布包保持原样。
- `distribution` 使用 Windows 实际包身份，异常时不退回绿色版注册/共享设置。商店包设置使用该包 LocalState，未安装 Store 构建使用独立 StorePreview；绿色版保留原 INI 规则。商店更新入口转到 Microsoft Store，不调用 GitHub 检查；登记/删除关联入口被 UI 和底层双重限制，默认应用仍由系统和用户决定。四种语言均添加离线隐私说明，并修正原更新设置中“关闭检查就完全不联网”的错误说法。
- 增加 `packaging/msix.py`、清单、图标构建、严格身份/版本/PE 检查、白名单内容核对；Test 身份不能作为正式包。双语隐私政策、Store listing/review/build 草稿、独立构建入口及手动隔离 Windows workflow 已编写。生命周期脚本未执行；不能当成安装/商店认证证据。
- 本轮已通过：MSVC fmt、两种配置 strict all-targets Clippy；绿色版 **106 项测试**，Store **108 项测试**；两种静态 CRT release 及编辑/更新测试替身；Python **5 项打包测试**。MakeAppx 10.0.22000 正常 schema 校验通过，13 个 payload 文件逐字节核对（另有 2 个 MSIX 元数据文件）。测试包 `dist/store-test-1/FeatherMD-1.0.10.0-x64-TEST.msix`，未签名、未安装，不是提交包；哈希及 imports 在同目录 package-report.json / SHA256SUMS.txt。
- **用户反馈窗口测试影响打游戏，已立即停止**本机测试父进程、子脚本及 target/release 测试程序；没有关闭用户的 dist 程序。现场检查无遗留测试进程。此前隐藏启动仍会使模态窗口短暂显示，因此新增全部 smoke 的默认本机保护，后续本机仅做静默校验，GUI 必须隔离 runner 或取得当次明确许可。
- 被中断的 19 项 GUI 套件：12 PASS（ui、navigation 两种、scrollbar、typography 两种、about 两种、settings 两种、editor、update 浅色）；window FAIL（footer tooltip 不出现）；update 深色中断；table/image 浅深及 i18n 共 5 项未运行。证据 `test-artifacts/store-adaptation-regression/`，不能宣称全量通过。没有为了复现 tooltip 再打扰用户桌面。新 Store GUI smoke 尚未运行。
- 未验证：安装后真实包设置路径、Shell 含中文/空格文件关联激活、共存/升级/卸载、Store URI 实际打开、WACK、Windows 10、完整辅助功能、第三方赞赏流程。正式名称/Identity/Publisher、公开隐私 URL、最终截图和提交仍待下一阶段确认。此记录不是“可以直接上架/一定过审”的结论。

## v0.1.10 publication record — 2026-10-16

- 用户明确要求“提交、发布”。已发布并设为 Latest：https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.10 （北京时间 2026-10-16 21:19:48）。不是草稿或预发布。
- 功能提交 e22c69e；英文 About 小工作区修正提交 **120c0d7ecf056dec3d06d8d48dc9778d72c3fc6a**，为 annotated tag v0.1.10 的源代码。首次 CI 的截断缺陷见下一节；修正后的 main CI run **38904974739** 和 tag CI run **38907003137** 均在 Windows / Ubuntu 通过，含完整原生回归和四语检查：https://github.com/Lianzy-Baimiao/FeatherMD/actions/runs/38904974739 。标签推送曾两次遇到 GitHub 443 连接超时，重试成功，没有改标签目标或强推。
- 最终 MSVC 静态 CRT 构建通过 fmt、strict Clippy、**104 项测试**（含新增的小工作区文字宽度测试）；交付 EXE 上完整 **19 项原生回归一次全部通过**，证据 `test-artifacts/release-0.1.10-final/`。CI 也通过 96 DPI 的英文 About 截断检查。版本号 0.1.10；仅依赖系统 DLL。
- 公开附件：FeatherMD.exe **1,098,752** 字节，SHA-256 **c594dceb6bd76b66c6060b0acff1afdb5b024172d82123ed17b46d8bdb86b86b**；FeatherMD-0.1.10-windows-x64.zip **2,564,032** 字节，SHA-256 **29d8ef74b29ffe97511654fa4a75859f975e19e84329a6d956e39d1c8a03ca2e**；SHA256SUMS.txt **181** 字节，SHA-256 **c196f4ebb74b0faa43f2a3e1a246ecb2aea9a2cec134520819ef8df206bb587a**。
- 三个附件均已从 Release 重新下载，逐字节哈希与本地交付一致，且与 GitHub asset digest、校验清单相符。ZIP 共 47 个文件：EXE 等同 target/release；46 个文档/图片均核对标签源码（仅四份 README 的 Windows CRLF 与 Git LF 不同，其余字节相同）。不含设置、测试夹具或开发环境文件。
- 新预览图 `v0.1.10/docs/reading-web-images.png` 和 `reading-web-images-loaded.png` 的公开 URL 均返回 HTTP 200；releases/latest 返回 v0.1.10；v0.1.9 三个发布附件的 digest 保持不变。
- 本地最终交付：**dist/release-0.1.10/**。最初未修正英文说明的包保存在 `dist/release-0.1.10-initial-candidate/`，未上传。正在运行的旧版 `dist/FeatherMD.exe` 未关闭或覆盖；旧发布 ZIP 均保留。测试使用各自的便携 INI，验证后清理。

## 2026-10-16 — v0.1.10 CI correction before tagging

- 首次 main 推送 e22c69e 的 CI（run 38901690861）：Ubuntu 全通过；Windows 编译/测试和前 18 项原生回归（含新图片功能浅/深，各 4 次回环请求）通过，仅最后 i18n 报英文 About 图片说明截断。runner 为 96 DPI / 1024×720；展开 About 会缩到约 90 DPI，当前长句在此大小放不下。这是确定的界面缺陷，不是直接重跑可忽略的间歇失败；尚未创建标签或发布。
- 英文由“click each placeholder and confirm”缩短为“click and confirm”。新增 GDI 单元测试，用 Microsoft YaHei UI 和 Segoe UI 测量 90 / 92 / 96 / 120 / 144 DPI 下 428 逻辑像素宽的该说明，防止回归。fmt / strict Clippy / 104 项测试 / MSVC release 通过；本机 120 DPI 的四語 105 个视图全部通过（GDI 31→31，USER 24→24），英文 About 截图已重新生成。等待修正提交的 CI 后再发布。

## 2026-10-16 — v0.1.10 release candidate: manual web images

- 用户明确要求“提交、发布”。版本升级至 0.1.10（Cargo/Cargo.lock、Windows FileVersion/ProductVersion、package 默认版本）；四份 README 去掉未发布标记并更新体积、新增两张手动加载截图。About 的版本由 Cargo 自动更新，重新生成浅/深及三种翻译版共 5 张截图和阅读设置截图（全部 120 DPI）。没有修改自动下载默认行为。
- MSVC static-CRT release：fmt --check、strict all-targets Clippy、103 项测试、release 与两个测试替身通过。19 项原生回归本轮**一次全部 PASS**，证据 `test-artifacts/release-0.1.10/`：navigation headings 416/494、wrapped 251/280；image 浅/深各 4 次回环请求，原本机图片 GDI 27→27；i18n 105 个视图无截断、8 次实时切换，GDI 30→30、USER 30→29。前一轮 0.1.9 开发构建的间歇失败记录保留在下一节，不据此宣称已修复。dumpbin 仅 Windows 系统 DLL。
- 打包时现有 `dist/FeatherMD.exe` 被运行中的程序占用；没有关闭程序或覆盖旧 EXE。package.ps1 增加独立 Output 目录及清理路径/重解析点检查，新包放在 `dist/release-0.1.10/`。EXE 1,098,752 字节，SHA-256 267929c899a9e67bfa5f2ee946bc594b5610227a1921df9e71d625b2cf069730；ZIP 2,564,325 字节，SHA-256 3f67c164eacc4fa49045196d3825b85df4c863bf5576658605dc488d88c5e3ab。ZIP 的 47 个文件逐字节核对当前工作区（EXE 核对 target/release），仅包含许可、四语 README、选定文档/截图和 EXE，不含 INI、测试夹具或机器专用配置。旧的发布 ZIP 保留。
- 发布顺序：提交/推送 main，等待 GitHub Windows 与 Ubuntu CI；通过后创建 v0.1.10 标签及 GitHub Release，上传 EXE / ZIP / SHA256SUMS.txt，并重新下载验证。此处是发布前记录，实际发布结果另记。

## 2026-10-16 — Manual HTTP(S) image display (unreleased)

- 用户需求：远程图片不自动下载，但缺少手动入口，增加手动触发展示。按明确需求为原 AGENTS.md 的禁止远程图片规则增加第二个、逐张确认的网络例外；没有增加自动下载开关，没有提交、推送、发布或改版本号。`dist/FeatherMD.exe` 和现有发布包保持原样。
- 预览中的 HTTP(S) 图片占位文字显示“点击加载网络图片”，使用链接色和手形指针；点击确认框显示完整地址及 IP 披露说明（默认选取消），确认后原位置显示。同地址共享内存缓存，但不会加载其他地址；失败/超时/格式不支持有重试入口。自动本机图片关闭时也可手动加载；保存无关设置不清空已手动加载的图片。外层超链接在图片加载后仍可点击；源码模式和表格单元格没有图片加载入口。
- 实现：`image::WebImage` 仅接受明确 HTTP(S)，保留 query、去掉 fragment，拒绝凭据、控制字符、反斜杠和无效地址；`src/ui/pictures/web.rs` 用系统 WinHTTP 匿名 GET，禁 Cookie、认证、Referer、重定向，不放宽证书验证，不落盘。32 MiB、光栅签名、原有像素/缓存上限；网络解码线程独立于本机图片线程，单线程 + 4 个等待项；UI 15 s 截止。F5/换文档取消旧任务，单调任务编号防止同 URL 的旧回复覆盖新请求；两个解码线程都退出后才关闭 GDI+。四语界面、Settings/About、四份 README、ARCHITECTURE、CHANGELOG 与待办同步。
- 构建验证：MSVC static CRT；`cargo fmt --check`、`cargo clippy --all-targets --locked -- -D warnings`、`cargo test --locked`（103 项）、release 和两个测试替身均通过。新测试覆盖 URL/query/凭据策略，回环 HTTP 200/302/404/401、Content-Length 与无长度的 32 MiB 上限、非图片内容、匿名请求头、取消/过期、同地址旧任务回复。`target/release/feathermd.exe` 1,098,752 字节，SHA-256 bfaec689fbe1bae99d364c23f678add099f435aaf6b3c914c32b382ea12cf76d，版本仍为 0.1.9。
- 原生验证（本机 120 DPI，隐藏窗口 + 合成消息，全部使用临时便携 INI）：image-smoke 浅/深原有本机图片断言全部通过（GDI 27→27），新增取消零请求、只加载所选 URL、重复 URL 一次 GET、链接图片、失败后手动重试、源码/预览切换不重下载、无关设置保存保留图片、下载中 F5 丢弃结果、重启无自动加载（每轮 4 次回环请求）。最后连续四轮浅/深交替全部 PASS：`test-artifacts/manual-image-repeat/`。多语言 105 个视图无截断、8 次实时切换，GDI 30→30、USER 24→24。
- 全量 19 项回归首轮 14 PASS / 5 FAIL：wrapped navigation（Backward paging stopped）、window（tooltip）、typography-light（GDI 35→38）、table-light（拖选未滚动）、image-light（未等到包含 URL 的确认框）。window/typography/table/image 单独重跑通过；wrapped 首次重跑又报 EOF not visible，之后与未改动的 dist v0.1.9 交替各两次全部通过，两版同为 251/280 页。因而每项均有通过记录，但**不宣称首轮全量通过，也未确定间歇失败原因**。证据：`test-artifacts/manual-image-regression/`、`manual-image-recheck/`、`manual-image-baseline/`。
- 限制：只验证本机回环 HTTP，未访问真实外网图片；真实 HTTPS/代理/证书失败未实测。系统同步 WinHTTP 调用可能超过 UI 截止，取消在调用间检查；不会为此继续创建线程，队列满时需稍后重试。仅支持直接返回 PNG/JPEG/GIF/BMP/TIFF/ICO 的地址，不跟随 3xx，不支持 SVG/WebP、隐式 `//host` 地址、带认证的地址或表格内图片。新增翻译未经母语者审校，未验证 Windows 10、多显示器或真实指针输入。

## v0.1.9 publication record — 2026-10-14

- 作者/最后修改人：lianzy。用户要求“上传github，发Releases,更新About”。Public latest release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.9
- Tag/source: 3e5e048b458dc7015b3a115e7ec0c3c9750d2d3e (annotated tag v0.1.9, title “FeatherMD 羽读 v0.1.9 · 排版、图片与 20 款皮肤”). GitHub Actions run 38500655155 (main, the first CI run of 0.1.9) passed on Windows and Ubuntu: all 19 native smokes at 96 DPI with a 1024×720 work area and an en-US locale (default_window "1024x720 in 1024x720"; navigation headings 499/607, wrapped 314/355; table reading width 712, wide table 1343, GDI 24→24; image light/dark 5 images, GDI 27→27; about GDI 25→25; i18n 105 views with no clipped text, "System" resolved to en, 8 live switches GDI 31→31, USER 22→22). The About text measured beforehand at 90–144 DPI also fit on the runner. Tag run 38501192394 also passed.
- Assets: FeatherMD.exe 1087488 bytes, FeatherMD-0.1.9-windows-x64.zip 2563922 bytes, SHA256SUMS.txt 180 bytes (SHA-256 c1e58a36650022570707bb26eac75dedad60b21f6608900a44b2ee8a0a70154f). All three were downloaded again; they are byte-identical to the local distribution and match the GitHub asset digests. All 44 non-EXE ZIP entries are byte-identical to the blobs at the tag, and the EXE is newer than every compile input. EXE/ZIP hashes are in the candidate entry below.
- Release-note preview URLs v0.1.9/docs/reading-typography.png (57,207 bytes), reading-skins.png (578,315 bytes) and reading-images.png (29,514 bytes) return HTTP 200, byte-identical to the tagged blobs. releases/latest reports v0.1.9. v0.1.8 tag (78e0e5e) and its asset digests are unchanged.
- 实网检查（下载的 EXE、各自的临时便携 INI、真实 GitHub API、系统代理设置）：已发布的 v0.1.8 显示“发现新版本 v0.1.9”，详情“FeatherMD 羽读 v0.1.9 · 排版、图片与 20 款皮肤　发布于 2026-10-14”，显示“前往下载页…”与更多按钮徽标，905 ms；v0.1.9 显示“已是最新版本”，无下载按钮与徽标，390 ms（均为单次本机样本）。两者自动检查关闭时都没有自动检查，INI 记录 update_latest=v0.1.9，update_check 仍为 false；临时 INI 已删除。
- GitHub 仓库 About：简介由“羽读 · 640 KiB native Rust Markdown viewer…”（体积已过时）改为“羽读 · Native Rust Markdown viewer for Windows. One ~1 MiB EXE, bounded-memory paging for huge files, code highlighting, safe HTML, optional local images, 20 skins, 4 UI languages. No browser runtime; offline by default.”；新增 11 个主题：markdown、markdown-viewer、markdown-reader、rust、windows、win32、gdi、native、portable、lightweight、large-files。主页字段保持为空。

## 2026-10-14 — v0.1.9 candidate: local images, HTML subset, code highlighting, reading layout, 20 skins, preview character selection

- 作者/最后修改人：lianzy。用户需求（2026-10-13 夜间，要求持续工作直到待办全部完成；**未要求提交、推送或发布**）：①右上角图标不好看；②设置增加“显示图片”，默认不勾选；③阅读排版：默认字号 16，行距、标题与代码 / 关键字样式；④支持 p、h1 等 HTML 标签；⑤去掉左上角的英文；⑥多做一些皮肤；⑦遗留：正文默认字体随界面语言、预览按字符选择、段落换行时标点不在行首。
- 会话说明：上一会话（e11eff24）在写图片读取时中断（02:33）；其记录 22:36 之后没有保存助手消息，但改动都在工作区。本会话从工作区继续：先修好 1 项失败的图片路径测试（`//?/C:/a.png` 先被切掉 `?` 后变成 `//`，误判为网络地址）和 2 处 clippy，再完成其余部分。
- 标题栏：左上角只留羽毛标识与文档名；打开 / 查找 / 外观 / 更多同尺寸同线宽（`smooth::header_weight`），悬停为文字色的淡圆角底；外观为固定的半圆图标；窗口按钮整列悬停、关闭为红色；预览 / 源码移到状态栏左侧的分段开关。
- 排版（`layout::flow` 重写）：每块文字只测量一次再统一断行（与表格同一 `table::break_range`，列表首行更窄以悬挂缩进，`<br>` 强制换行）；行高 1.6 倍、空行间距、标题上大下小、一二级标题末行下划线、代码块圆角底板与语言标签、行内代码与 kbd 圆角、引用灰线、表格斑马纹、任务复选框、上下标、混排基线对齐。
- HTML（`src/html.rs`）与高亮（`src/highlight.rs`）：见 ARCHITECTURE “Reading layout, highlighting and HTML”。新增 `<center>` 与 `text-align` 样式的对齐（含测试）。
- 皮肤：新增素白、报纸、丁香、暮色、极夜、深海、紫夜、咖啡，共 20 款；先用对比度脚本选色再写入，避开他人主题的原值。新皮肤追加在 `Theme::ALL` 末尾（已保存序号不变），画廊用 `Theme::GALLERY` 浅色在前、5×4。
- 图片（`src/image.rs`、`src/ui/pictures.rs`）：默认关闭；只读本机固定 / 可移动 / 光盘 / 内存盘；逐级 FindFirstFileW 拒绝联接点、符号链接等重解析点（云文件与去重除外），不打开仅在云端的文件；32 MiB / 16384 px / 3200 万像素上限；按文件头识别格式后才交给 GDI+；EXIF 方向；保留最多 2048 px；每页 64 张、共 160 MiB；后台线程，退出时 GDI+ 只在解码线程结束后关闭。排版中图片是一个 U+FFFC 单元，站在基线上。
- 预览按字符选择（`src/ui/select.rs`）：位置为“块起始字节 + 显示单元”，重排后不变；复制时补回换行处去掉的空格，表格整行，图片复制替代文字。设置阅读页、关于窗口说明与菜单文字随之更新。
- 遗留项：正文默认字体为空即跟随界面语言（日文 Meiryo）；标点不在行首由 `table::break_range` 保证，新增全宽度属性测试。
- 首个本地候选的证据（未发布；之后只改了关于窗口与文档，见下方“发布”两条）（本机 125% / 120 DPI，MSVC release，FileVersion 0.1.9）：fmt --check、clippy --all-targets --locked -D warnings、cargo test --locked（99 项）通过；`cargo build --release --locked`。dist/FeatherMD.exe 1,086,976 字节，SHA-256 207ad063a00c53c5dfb9025929e13014a7fd9b7d5f5a1c022d20f875749b2429（与 target/release 相同）；dist/FeatherMD-0.1.9-windows-x64.zip 2,494,165 字节，SHA-256 cc003dfb9ab45e933173ef15bad94e1356b9060ae39b25408e9f35eb4933ba66，45 项，与工作区源文件逐字节一致，不含 INI 或测试数据。交付 EXE 上 19 项原生回归全部 PASS（新增 image-smoke 浅 / 深：5 张图片尺寸、居中、透明、EXIF 6、徽章同行、链接命中、宽图随窗口重排、4 种原因、联接点被拒、GDI 27→27、关闭后释放内存与位图）。docs 下 34 张截图与交付 EXE 的截图逐字节一致。i18n 105 个视图在 120 DPI 无截断；本轮新增文字用同一字体在 96 DPI 实测宽度均放得下（最紧的是皮肤卡片 “Newsprint” 68/84 px），**未在 CI 的 96 DPI 上运行**。dumpbin：新增 Windows 自带的 shlwapi.dll。
- 偶发失败：打包前最后一轮全量回归中 about-dark 一次未在 3 秒内等到外部链接确认框；随后单独重跑 10 次与交付 EXE 全量回归均通过，未复现。疑为隐藏、未激活的对话框上 BM_CLICK 偶尔不生效，脚本未改。
- 性能见 PERFORMANCE §8：普通段落重排 53→36 ms，表格不变，代码块 +1–2 ms，首屏无差别。
- 限制：未在真实的“仅在云端”OneDrive 文件、U 盘或映射网络驱动器上实测（只有规则与联接点测试）；指向网络共享的 subst 盘符未单独处理；GIF/TIFF 只显示第一帧；HTML 按行处理（跨行的单个标签、`<table>` 不按浏览器呈现）；表格单元格内的图片显示替代文字。新增界面文字的繁中、英文、日文翻译由 AI 起草，未经母语者审校。
- 用户设置：本机 %APPDATA%\FeatherMD\settings.ini 在 02:49:24 被一个已带 `images` 键的版本重写（内容仍是用户自己的值，只多了 `images=false`）；本会话的界面测试都用 EXE 旁的临时便携 INI，且最早一次在约 03:00 才开始，来源未查明。
- 发布请求（2026-10-14 上午，用户：“上传github，发Releases,更新About”）：About 同时理解为应用内的关于窗口与 GitHub 仓库的 About。关于窗口新增第 4 条说明“图片默认不显示，可在设置 → 阅读中开启；只读本机文件，不下载网络图片。”（四种语言），展开面板加高 20 逻辑像素（DETAILS_HEIGHT 260，面板 262）。上线前用与应用相同的字体、矩形与 DrawTextW 标志测量了关于窗口全部文字在 90 / 92 / 96 / 120 / 144 DPI 下的宽度（CI 的 1024×720 工作区会把展开后的窗口缩到约 90 DPI）：全部放得下，最紧的是英文新说明 418/428 px（96 DPI）与英文分页说明 421/428 px。四份 README 的关于段落、CHANGELOG 同步；PERFORMANCE §8 的体积改为发布 EXE 的字节数。
- 发布候选的证据（本机此时为 150% / 144 DPI、单显示器 2560×1600，此前为 120 DPI）：fmt --check、clippy --all-targets --locked -D warnings、cargo test --locked（99 项）、`cargo build --release --locked` 与两个测试替身通过。dist/FeatherMD.exe 1,087,488 字节，SHA-256 1dc7c896249854b1d0f67c33083461b36c8876c9a58a50fe813f8a27ab4b0e5c（与 target/release 相同）；dist/FeatherMD-0.1.9-windows-x64.zip 2,563,922 字节，SHA-256 b7579d8b7fe8556b1aa0bac8477100d41f0ea1ce70264051af02c3b4a37f042b，45 项，与工作区源文件逐字节一致。交付 EXE 上 19 项原生回归全部 PASS（default_window 1620×1230 in 2560×1552；navigation headings 454/535、wrapped 251/280；about 浅 / 深 GDI 25→25；table GDI 24→24、复制 29→29；image 5 张、27→27；i18n 105 个视图无截断，8 次实时切换 GDI 30→30、USER 30→29）。关于窗口的 5 张截图（浅 / 深、zh-TW / en / ja）由交付 EXE 在 144 DPI 重新生成，与其截图逐字节一致，所以比其余截图大 1.2 倍；其余截图仍是上一轮 120 DPI 的（阅读、设置等界面代码此后未改）。dumpbin 仍只有 Windows 系统 DLL。

## v0.1.8 publication record — 2026-10-13

- 作者/最后修改人：lianzy。用户在需求中预先要求“改完了自动提交推送发布”。Public latest release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.8
- Tag/source: 78e0e5ed85cbf36f8da02dbf726da7965dbfb1ce (annotated tag v0.1.8, title “FeatherMD 羽读 v0.1.8 · 一键复制与多语言”). GitHub Actions run 38398636465 (main) passed on Windows and Ubuntu: all 17 native smokes at 96 DPI with a 1024×720 work area and an en-US locale (default_window "1024x720 in 1024x720"; navigation headings 384/456, wrapped 313/355; table copy GDI 28→28; i18n 105 views, "System" resolved to en, 8 live switches GDI 32→32, USER 22→22). Tag run 38399567039 also passed. The first push b9e984d (run 38396104689) failed only i18n-smoke on the Japanese picker note, fixed in 78e0e5e; see the candidate entry below.
- Assets: FeatherMD.exe 935936 bytes, FeatherMD-0.1.8-windows-x64.zip 1646973 bytes, SHA256SUMS.txt 180 bytes (SHA-256 4b5272640fda299d41ee257281cfad726fe445f9af06b71fe531147907855321). All three were downloaded again; they are byte-identical to the local distribution and match the GitHub asset digests. Before tagging, all 38 non-EXE ZIP entries were byte-identical to the committed blobs and the EXE was newer than every compile input. EXE/ZIP hashes are in the candidate entry below.
- Release-note preview URLs v0.1.8/docs/copy-button.png (23,412 bytes) and reading-light.en.png (64,705 bytes) return HTTP 200, same sizes as the tagged blobs. releases/latest reports v0.1.8. v0.1.7 tag (6682e23) and its asset digests are unchanged.
- 实网检查（下载的 EXE、各自的临时便携 INI、真实 GitHub API、系统代理设置）：已发布的 v0.1.7 显示“发现新版本 v0.1.8”，详情“FeatherMD 羽读 v0.1.8 · 一键复制与多语言　发布于 2026-10-13”，显示“前往下载页…”与更多按钮徽标，1354 ms；v0.1.8 显示“已是最新版本”，无下载按钮与徽标，1698 ms（均为单次本机样本）。两者自动检查关闭时都没有自动检查，INI 记录 update_latest=v0.1.8，update_check 仍为 false；临时 INI 已删除。

## 2026-10-13 — v0.1.8 candidate: one-click copy, interface languages, multilingual README, About refresh

- 作者/最后修改人：lianzy。用户需求：①代码块等方块（代码、引用、表格）右上角鼠标移入时显示一键复制按钮；②程序支持多语言；③README 支持多语言；④更新过时的关于窗口；⑤改完自动提交推送发布（本轮已预先授权）。
- 会话说明：上一会话完成了 i18n 核心、界面文字转换、语言页与关于窗口重排，随后因上游接口错误（403 / 无输出）中断；改动保留在工作区。本会话先复核构建与当时的 70 项测试，再完成其余部分。此前尚未在转换后的界面上运行任何原生回归。
- 一键复制：`markdown::boxes`（纯逻辑）把预览分页的块分组为围栏代码、缩进代码、引用和表格；围栏块交替开闭，所以一个框延伸到下一个围栏块或分页末尾，空围栏没有可复制内容；触及分页边缘的框标为部分。`text::source` 按字节范围复制已载入行（物理行之间 CRLF，超长行片段之间不加换行，可去掉一级缩进）。重排时按起始字节映射到布局行；绘制时用已有的行命中框找到指针所在的框，按钮在框可见部分的右上角（长块不必滚回开头），比正文窄的表格放在表格右侧，按实测文字定宽，以“复制 / 已复制”中较宽者决定位置。复制后对勾 1.5 s；源码模式没有按钮。测试快照路径存在时，按钮与 Ctrl+C 都写 copy.txt，不碰剪贴板。
- 多语言：简体中文为查找键，`src/i18n/strings.rs` 每行 [zh-CN, zh-TW, en, ja]；缺失时显示原文。设置 `language=system|zh-CN|zh-TW|en|ja`，system 按 GetUserDefaultUILanguage（台港澳 → 繁中，其他中文 → 简中，日语 → 日文，其余 → 英文）。保存后替换界面字体缓存（搜索框先换字体再释放旧缓存）、重建菜单、重绘，无需重启。各语言界面字体：Microsoft YaHei UI / Microsoft JhengHei UI / Yu Gothic UI；阅读字体不变。
- 截断检查：测试快照路径存在时，`snapshot::clip` 在阅读器、关于与设置每次绘制文字前用 DT_CALCRECT 测量并记录会被截断的文字（省略号文字除外），`snapshot::report` 再加上放不下的 STATIC 子控件，写入 labels.txt。首轮实测发现英文欢迎页按钮“Open document”被截成“Open docume”、外观弹层英文提示被截断：按钮改为按实测文字加宽（中文仍为 208 逻辑像素），提示缩短。
- 翻译复核：古籍 Ancient → Antique；日文“前往下载页…”原译“ダウンロード…”读起来像会直接下载，改为“配布ページへ…”；繁中引用统一为“引文”。繁中、英文、日文翻译由 AI 助手起草，未经母语者审校。
- 关于窗口：展开面板改为两列快捷键（新增 Ctrl+E、Ctrl+,、Ctrl+T、Esc）与四条说明（一键复制、表格排版、按字符选择、分页预览）；“返回阅读”加宽并居中，版本号在胶囊内居中。展开后逻辑高度 720，小工作区沿用原有的按比例缩小。
- README：新增 README.zh-TW.md / README.en.md / README.ja.md 与语言切换行，各自使用本语言的阅读界面、关于窗口与语言设置截图（其余截图为简体中文界面，文中说明）；翻译版示例文档 docs/demo.*.md；ZIP 含四份 README。about.rs 单元测试检查四份 README 都含项目与赞赏链接。
- 测试：新增 tests/copy.rs（3 项）与 tests/i18n.rs（5 项：翻译表占位符/助记符/快捷键/换行一致、繁中与日文不含简体字、源码扫描所有中文界面字面量都经翻译表、LANGID 映射、设置往返）；table-smoke 增加复制按钮段；新增 i18n-smoke（四种语言 × 欢迎页、9 个标题栏提示、外观弹层、菜单、5 个设置页、关于收起/展开、文档与复制按钮、查找、跳转、示例文档；system；8 次实时切换）。所有冒烟 INI 固定 language=zh-CN（CI 为英文系统）。修正 window-smoke：欢迎页前的 INI 重置与 `StopReader` 写在同一行，PowerShell 把它当成被忽略的参数，从未执行。
- CI 发现：首次推送（b9e984d，run 38396104689）Ubuntu 与 Windows 上其余 16 项冒烟均通过，i18n-smoke 在 windows-latest（96 DPI、en-US，system 正确解析为 en）报告日文外观弹层提示“システムに合わせると明暗が自動で切り替わります”被截断（本机 120 DPI 时刚好放下）。改为“システムは明暗を自動で切り替えます”，重新构建、打包并在交付 EXE 上重跑全部 17 项。
- 证据（125% / 120 DPI，MSVC release，FileVersion 0.1.8，--locked）：fmt --check、clippy -D warnings、cargo test --locked（73 项）通过；构建、交付与 ZIP 内 EXE 哈希一致；交付 EXE 上 17 项原生回归全部 PASS：ui（新 EXE 首次启动 142.3 ms，随后三次 81.3 / 81.3 / 73.7 ms，均为单次样本；GDI 35→35）、navigation headings 333/411、wrapped 242/280（与 v0.1.7 记录相同）、scrollbar、window（1350×1025 in 3840×2120，51 项命中）、typography 浅/深（35→35）、about 浅/深（22→22）、settings 浅/深（35→35）、editor、update 浅/深（各 14 次回环请求，33→33）、table 浅/深（正文宽 960，放得下的表右缘 960，宽表 1633；复制按钮重复 4 轮 GDI 28→28）、i18n（105 个视图无截断，8 次实时切换 GDI 31→31、USER 30→29，本机显示语言 0x0804 → zh-CN）。docs 截图与交付 EXE 的截图逐字节一致。
- 性能（与已发布 v0.1.7 交替，见 PERFORMANCE §7）：满页表格重排 86.3/88.2 vs 86.6/86.1 ms，普通段落 63.7/61.6 vs 61.9/59.3 ms，约 1100 个代码块 25.6/24.2 vs 27.5/24.7 ms；首屏 12 次中位数 84.6 vs 84.9 ms。无可测出的差别。dumpbin：仍只有 Windows 系统 DLL。
- Delivery: dist/FeatherMD.exe 935,936 字节（914 KiB），SHA-256 02b3417e1e932f428303ab703904c9ea89aa90e8a1100306e1d2d5a19199021b；dist/FeatherMD-0.1.8-windows-x64.zip 1,646,973 字节，SHA-256 c884e3af93d2d77d0e1d0956f87a8a4835161d9034c3f6d63ddee23b4b7244d5（39 项，含四份 README 与各自截图）。CI 修正前的候选（96aa2b44…）未发布。已发布的 v0.1.7 EXE 另存于 target/tmp/rel017 供对比。用户 %APPDATA% 设置文件未被测试改动（修改时间仍为 05:01）。
- 限制：只在本机 125% 下实测；隐藏窗口 + 合成消息，未覆盖真实指针与系统剪贴板。截断检查只覆盖 FeatherMD 自己绘制的文字与 STATIC 控件，不含系统消息框、文件对话框与弹出菜单。system 语言只验证了本机（简体中文）。日文用户的正文默认字体仍是 Microsoft YaHei UI，需要在“阅读”页自选日文字体。预览仍按行选择；列表里的代码块按缩进代码处理（原有行为）。

## v0.1.7 publication record — 2026-10-13

- 作者/最后修改人：lianzy。用户明确要求“提交推送发布”。Public latest release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.7
- Tag/source: 6682e23ba3ba16ea62461126c607c9f67f43ba17 (annotated tag v0.1.7). GitHub Actions run 38350463869 (main) passed on Windows and Ubuntu, including table-smoke light/dark. The Windows runner is 96 DPI with a 1024×720 work area (default_window "1024x720 in 1024x720"), so the default frame was capped by the work area; table-smoke scrolled the wide table into view before comparing horizontal positions (reading width 712, fitting table right edge 712, wide table 1343; 7 lines default, 16 narrow; auto-scroll 21/20 rows, GDI 24→24). navigation headings 384/456 on CI. Tag run 38351047039 also passed: https://github.com/Lianzy-Baimiao/FeatherMD/actions/runs/38350463869
- Assets: FeatherMD.exe 855040 bytes, FeatherMD-0.1.7-windows-x64.zip 1059359 bytes, SHA256SUMS.txt 180 bytes (SHA-256 0358f79fa690857000b8450a9575d9d367e2b23d0d5ced0b159443bed8a3e3fb). All three were downloaded again; they are byte-identical to the local distribution and match the GitHub asset digests. Before committing, all 24 non-EXE ZIP entries were byte-identical to the committed blobs. EXE/ZIP hashes are recorded in the v0.1.7 candidate entry below.
- Release-note preview URLs v0.1.7/docs/reading-table.png (101,200 bytes) and source-selection.png (20,047 bytes) return HTTP 200, same sizes as the tagged blobs. releases/latest reports v0.1.7. v0.1.6 tag (646b79b) and its asset digests are unchanged.
- 实网检查（下载的 EXE、各自的临时便携 INI、真实 GitHub API、系统代理设置）：已发布的 v0.1.6 显示“发现新版本 v0.1.7”，详情“FeatherMD 羽读 v0.1.7 · 表格与按字符选择　发布于 2026-10-13”，显示“前往下载页…”与更多按钮徽标，874 ms；v0.1.7 显示“已是最新版本”，无下载按钮与徽标，625 ms（均为单次本机样本）。两者自动检查关闭时都没有自动检查，INI 记录 update_latest=v0.1.7，update_check 仍为 false；临时 INI 已删除。

## 2026-10-13 — v0.1.7 local candidate: tables, source character selection, DPI-scaled default window

- 作者/最后修改人：lianzy。用户需求：①默认窗口跟着缩放调整；②解析表格，按窗口大小自动调整，避免窗口太小时显示混乱；③源码模式按字符选择复制，而不是按行。
- 为什么之前不解析表格：markdown::blocks 逐个物理行独立分类，表格需要表头 + 分隔行 + 后续行的跨行上下文，原实现只把以 `|` 开头的行标成等宽文本（README 也列为已知限制）；鼠标选择同样以布局行为单位。
- 表格（src/table.rs、markdown.rs、layout.rs、ui.rs）：表头行 + 同列数的分隔行才成表（分隔行并入表头的字节范围），后续非空且不开始其他块的行为表体；无分隔行的 `|` 行改为普通段落（与 GFM 一致，此前是等宽文本）。单元格文字转义，不会变成标题/列表/引用/分隔线/围栏。分页从表格中间开始时，开头的 `|` 行作为无表头表格。列宽：自然宽度 = 最宽单元格；最小宽度 = 约 6 em（内容更窄时取内容）或最长单词（上限 12 em）；放得下用自然宽度，放不下先把最宽的列收窄到共同宽度（初版按比例收窄会让“重新载入”“参数八”这类短列折行，实图检查后改掉），最小宽度也放不下时表格超出正文宽度并单独横向滚动。单元格换行：空格后、中文字符间，不在闭标点前或开标点后断行，不拆代理对。每个表格可视行是一个布局行，翻页、滚动仍按行。
- 横向滚动：自动换行时只有超出正文宽度的表格随横向滚动，正文与放得下的表格不动（初版整页表格一起移动，实图检查后修正）；关闭换行时表格保持自然宽度，与正文一起滚动。预览复制：表格行按制表符分隔，一个表格行只写一次。
- 源码按字符选择：`text::Caret` = 已载入行的起始文件字节 + 解码文本内字节下标，重排、缩放、分页补读后选区仍有效；命中测试取最近字符边界（制表符按 4 格显示、不拆代理对）；双击选词，Shift+单击扩展，拖出正文上下边缘时在 UI 定时器上自动滚动；Ctrl+A 选当前分页，Esc 取消，右键菜单显示“复制选中文本”。复制原文：制表符保留，行间 CRLF，超长行拆开的片段之间不加换行；选区超出已载入分页时只复制已载入部分并在状态栏说明。选区底色为强调色混入代码底色（原选中色在代码行上只有 1.09:1），12 款皮肤实测 1.43–1.89:1，文字 ≥ 6.0:1，有单元测试。
- 默认窗口原因：进程为 Per-Monitor V2 感知，CreateWindowEx 的 1080×820 是物理像素，自首个版本起不随缩放变化；150% 下只有 720×547 逻辑像素，目录用窄宽度。
- 默认窗口实现（src/ui.rs）：创建后、首次显示前，`initial_frame` 按窗口 DPI 把 1080×820 逻辑像素换算为物理像素，限制在显示器工作区内（不小于 640×480 逻辑像素），系统层叠位置越界时移回工作区；与原有 SWP_FRAMECHANGED 合并为一次 SetWindowPos。GetMonitorInfoW 失败时保持原行为。默认与最小尺寸改为 DEFAULT_SIZE / MIN_SIZE 常量，WM_GETMINMAXINFO 共用。之后切换显示器或缩放仍由已有的 WM_DPICHANGED 按系统建议矩形处理，未改。
- 默认窗口测试：新增单元测试（100/125/150/200%、DPI 查询失败按 96、层叠位置越界、小工作区封顶、最小尺寸优先、负坐标副显示器）。window-smoke 去掉 v0.1.6 的加宽，改为断言默认窗口 = min(1080×820 逻辑像素, 工作区)、不小于最小尺寸且位于工作区内，并在 JSON 记录 default_window。
- 默认窗口单独验证时（EXE 828bfefb…）：14 项原生回归全部 PASS；首屏与 v0.1.6 交替各测 5 次，中位 76.2 vs 74.8 ms，无可见差异；可见窗口实测 DPI 144，窗口 152,152–1772,1382（1620×1230），位于工作区 0,0–2560,1552 内。
- 表格与选择测试：新增 tests/table.rs（10 项：切分、分隔行与对齐、列宽分配、单元格断行含中文/标点/代理对、表格识别与边界、分页从表格中间开始、显示位置与源字节映射、原文复制、选词）与选区对比度单元测试；新增 scripts/table-smoke.ps1（浅/深）：两张表的列数与行数、放得下的表不超出正文、宽表超出并可横向滚动、只有宽表随滚动移动、窄窗口内单元格换行且不超出、关闭换行时自然宽度并与正文一起滚动、预览复制为制表符分隔且换行行只写一次；源码模式拖选精确到字符（跨三行）、双击选词、Shift+单击跨中文与 emoji、全选与 Esc、无选区复制可见原文、拖到正文下方自动滚动 20 行并复制 47 行；GDI 先预热再重复 4 轮不增长。复制经 WM_APP+42 查询 62 写入测试目录的 copy.txt 读取，不改动用户剪贴板。GDI 首次选择后 21→24 并保持不变，用 6 轮重复确认是一次性分配而非泄漏。
- 证据（150% / 144 DPI，MSVC release，FileVersion 0.1.7，--locked 构建）：fmt --check、clippy -D warnings、cargo test --locked（65 项）通过；构建、交付与 ZIP 内 EXE 哈希一致；交付 EXE 上 16 项原生回归全部 PASS：ui（首屏 83.04 ms 单次样本，GDI 35→35）、navigation headings 333/411、wrapped 242/280（与本轮默认窗口候选相同，说明非表格排版未变）、scrollbar、window（1620×1230 in 2560×1552，51 项命中）、typography 浅/深、about 浅/深（25→25）、settings 浅/深（35→35）、editor（真实右键菜单）、update 浅/深（各 14 次回环请求，GDI 36→36）、table 浅/深（正文宽 1152，放得下的表右缘 1152，宽表 1991；默认 6 个可视行，窄窗口 16 个）。docs/reading-table.png 与 source-selection.png（裁取顶部 300 px）与交付 EXE 的截图逐像素一致。
- 表格重排耗时（同一窗口 1620×1230、144 DPI，约 128 KiB 满页 1511 行 6 列表格；12 次放大/缩小命令往返的中位数，含 INI 保存，不是绘制时间；两轮）：v0.1.6 等宽文本 149.6 / 150.2 ms，v0.1.7 表格 35.9 / 35.6 ms；同内容去掉 `|` 的普通段落 v0.1.6 29.8 / 30.0 ms，v0.1.7 28.2 / 28.6 ms。普通文本无回退；v0.1.6 处理这类等宽长行为何更慢未查明。
- Delivery: dist/FeatherMD.exe 855,040 字节（835 KiB），SHA-256 fb6c428bbc119ddc76c497f80d20dc5e0c4f0d10cb05d08fc188be24a2fbfa81；dist/FeatherMD-0.1.7-windows-x64.zip 1,059,359 字节，SHA-256 c8ade0eb97446cc26ada1d3948adc38bf8c3b6fdb418e1949a77bc28a15e1e7c（新增 reading-table.png、source-selection.png）。仅含默认窗口改动的候选包改名为 0.1.7-window-candidate；已发布的 0.1.6 ZIP 保留不变。用户的 %APPDATA% 设置文件未被测试改动（修改时间仍为 05:01）。
- 限制：只在本机 150% 下实测。隐藏窗口 + 合成鼠标消息，未覆盖真实指针和系统剪贴板。按字符选择只在源码模式；预览仍按行。段落（非表格）换行仍可能让逗号、句号出现在行首（原有行为，本轮未改）。列表/引用里的表格不按 GFM 嵌套处理；分页从表格中间开始且表头在 32 KiB 回看范围外时显示为无表头表格。CI 运行器的工作区大小未记录，推送后看 window.json 的 default_window。table-smoke 在小屏幕上会先把宽表滚进视口再比较横向位置（本机缩小到 1024×728 物理像素的变体滚动 11 行后浅/深均通过）；比较原文改为区分大小写。用户确认后推送 GitHub，CI 通过后发布；发布记录见上一条。

## v0.1.6 publication record — 2026-10-13

- 作者/最后修改人：lianzy。用户明确要求“提交推送发布”。Public latest release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.6
- Tag/source: 646b79b8c6178bc0a921e6c82d7a7ba96b2b8961 (annotated tag v0.1.6). GitHub Actions run 38333886061 (main) passed on Windows and Ubuntu, including the new update smokes; the Windows runner (96 DPI) showed the real More menu (more_menu_label_checked=true), window-smoke did not need to widen (widened_default_window=false), navigation headings 357/419, wrapped 274/299. Tag run 38334232952 also passed: https://github.com/Lianzy-Baimiao/FeatherMD/actions/runs/38333886061
- Assets: FeatherMD.exe 821248 bytes, FeatherMD-0.1.6-windows-x64.zip 935663 bytes, SHA256SUMS.txt 180 bytes (SHA-256 bdf90cfe94613be37772e214a5c28b9b40646752876a9d83637929d060e0a45c). All three were downloaded again; they are byte-identical to the local distribution and match the GitHub asset digests. EXE/ZIP hashes are recorded in the v0.1.6 candidate entry below.
- Release-note preview URL v0.1.6/docs/settings-update.png returns HTTP 200 (39,640 bytes, same as the tagged blob). releases/latest reports v0.1.6. v0.1.5 tag (ffbdaac) and its asset digests are unchanged.
- 实网检查（下载的发布 EXE、便携 INI、真实 GitHub API、系统代理设置）：更多 → 检查更新显示“已是最新版本”，详情“FeatherMD 羽读 v0.1.6 · 检查更新　发布于 2026-10-13”，不显示“前往下载页…”与徽标；从命令到结果 788 ms（单次本机样本）；自动检查关闭时没有自动检查；INI 记录 update_checked 与 update_latest=v0.1.6，update_check 仍为 false。

## 2026-10-13 — v0.1.6 release candidate: update checks via GitHub Releases

- 作者/最后修改人：lianzy。用户需求：接入打开检查更新，默认关闭，支持手动检查更新；更新源为 GitHub Releases。
- 这是项目首个网络访问，与 AGENTS.md 原“无网络获取”约束冲突；按用户明确要求实现为唯一例外并更新 AGENTS.md、README、关于窗口措辞（“无自动联网”→“默认不联网”）。范围：仅 HTTPS GET api.github.com/repos/Lianzy-Baimiao/FeatherMD/releases/latest（系统 WinHTTP、系统代理、默认证书校验），只读公开版本信息；不下载、不安装、不上传。
- 打开时自动检查（update_check，默认 false）：首屏约 3 秒后后台检查，每 24 小时最多一次；仅成功检查记录 update_checked / update_latest，失败静默。手动：更多 → 检查更新…打开设置新增的“更新”页并立即检查；结果显示已是最新 / 发现新版本（版本名、发布日期）/ 失败原因；“前往下载页…”经确认后打开固定仓库的 Release 页面。发现新版本时“更多”按钮有圆点、菜单项为“发现新版本 vX.Y.Z…”，跨会话保留至升级。
- 实测问题：WinHTTP 的 WinHttpSetTimeouts 与 WINHTTP_OPTION_RECEIVE_RESPONSE_TIMEOUT 在回环测试中不可靠（1 s 超时 2.5 s 后才触发；0.4 s、2 s 未触发）。改为在 UI 轮询时强制 15 s 截止（updater::Pending），超时后放弃该线程，WinHTTP 句柄在其返回时关闭；有单元测试覆盖。原因未进一步查明。
- 测试：纯逻辑（版本比较含 SemVer 预发布、只取顶层字段、BOM/转义/代理对、截断与 4000 次变异不崩溃、24 小时判定、设置往返与注入）；WinHTTP 回环（请求行与头、200/404/403/429/500、1 MiB 上限、截止时间）；update-smoke 浅/深（默认关闭零请求、开启后启动一次、重启 24 小时内不重复、徽标像素与“更多”菜单真实文字、手动已是最新/新版本/403/HTML、链接确认、复选框仅保存时生效、检查中关闭、GDI 前后一致）。
- 实网检查一次（本机，经系统代理设置）：v0.1.6 对 GitHub 最新 v0.1.5 显示“当前版本比 GitHub 最新发布更新”，耗时约 0.4–0.65 s，未显示徽标；控制台乱码仅为代码页显示问题，按 UTF-8 校验文字正确。
- 其他：about-smoke 曾单次失败（读取确认框文字早于其填充），三次单独重跑均通过；该脚本与 update-smoke 改为等待文字出现。设置截图改为只绘制 WS_VISIBLE 子控件（WM_PRINT 会画出隐藏的“前往下载页…”）。首屏时间交替测 0.1.5 / 0.1.6 各三次（72/76/81 vs 78/70/71 ms），无可见差异。
- 候选证据（120 DPI，候选 EXE e20e4536…，同一源码）：fmt --check、clippy -D warnings、cargo test --locked（53 项）通过；14 项原生回归全部 PASS：ui（GDI 33→33）、navigation headings 454/535、wrapped 449/504、scrollbar、window（51 项命中）、typography 浅/深、about 浅/深（GDI 22→22）、settings 浅/深、editor、update 浅/深（各 14 次回环请求）。update-smoke 另连续重复 10 次（浅/深各 5）通过。dumpbin：仅 Windows 系统 DLL，新增 winhttp.dll。
- 发布构建：按待提交的树以 --locked 重新构建；fmt、clippy、test --locked（53 项）通过，构建、交付与 ZIP 内 EXE 哈希一致。此时本机显示缩放已变为 150%（应用报告 144 DPI），首轮 window 与 navigation（headings）失败：默认窗口 1080×820 物理像素在 150% 下只有 720×547 逻辑像素，低于宽目录阈值 800；headings 需要 1111 次 PageDown，超过脚本的 1000 次上限。已发布的 v0.1.5 EXE 两项以完全相同的方式失败，不是本版回归。只修测试脚本，产品未改：window-smoke 在逻辑宽度不足 800 时先加宽到 1080×820 逻辑像素再检查宽目录，并在 JSON 记录 widened_default_window；navigation 上限改为 10000，仅作防挂起。修正后 v0.1.5 与 v0.1.6 在 144 DPI 下同为 1111/1280。96/120 DPI 下默认窗口已不小于 800 逻辑像素，两个脚本行为不变。
- 发布证据（144 DPI，交付 EXE）：14 项原生回归全部 PASS：ui（首屏 69.04 ms，单次演示文档样本；GDI 35→35）、navigation headings 1111/1280、wrapped 749/823、scrollbar、window（51 项命中，widened_default_window=true）、typography 浅/深（35→35）、about 浅/深（25→25）、settings 浅/深（35→35）、editor（真实右键菜单）、update 浅/深（各 14 次回环请求，“更多”菜单真实文字已核对，GDI 36→36）。
- 偶发失败记录：一次中途被 `| head -5` 截断的验证运行之后约 10 分钟内，navigation（headings）连续约 8 次报“Forward paging failed to reach EOF”，包括已发布的 v0.1.5 EXE；随后又全部通过。当时推测是被截断的后台验证与之重叠。上述 150% 缩放下会稳定出现同一报错，更可能是当时显示缩放临时变化；当时的失败信息还没有 DPI，无法证实。
- Delivery: dist/FeatherMD.exe 821,248 字节（802 KiB），SHA-256 2dd3cc2a86742c994695cf9b71c76d393b5552657d0da9a6e332e41e9fdfb575；dist/FeatherMD-0.1.6-windows-x64.zip 935,663 字节，SHA-256 1cac87a9e38563c05cb58f51c196849b827bfa2e86cdc2d1fcc04ee1b355f4fd（新增 settings-update.png）。早期 ZIP 均保留，此前的本地候选包改名为 0.1.6-candidate。用户的 %APPDATA% 设置文件未被测试改动（修改时间仍为 05:01）。
- 限制：CI 只用回环替身，不访问 GitHub；真实代理、PAC、证书失败路径未自动化。默认窗口大小不随 DPI 缩放（自首个版本起如此）：150% 下 720×547 逻辑像素，按计算 200% 下会小于 640×480 的最小尺寸（未实测）；本版未改，留待后续。用户确认后推送 GitHub，CI 通过后发布；发布记录见上一条。

## v0.1.5 publication record — 2026-10-13

- 作者/最后修改人：lianzy。用户明确要求“提交推送并发布”。Public latest release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.5
- Tag/source: ffbdaac331b4725548450a92a1c881c8ead4d636 (annotated tag v0.1.5). GitHub Actions run 38309195090 (main) passed on Windows and Ubuntu, including the new settings/editor smokes; the Windows runner showed the real context menu (native_context_menu_label_and_keyboard_launch=true). Tag run 38309635003 also passed: https://github.com/Lianzy-Baimiao/FeatherMD/actions/runs/38309195090
- Assets: FeatherMD.exe 783360 bytes, FeatherMD-0.1.5-windows-x64.zip 874161 bytes, SHA256SUMS.txt 180 bytes (SHA-256 e5383fae1f2a5a9dc1c2fa2367e943f60d3c4b52bc250209db6f3490248ed33b). All three were downloaded again; hashes match the local distribution and GitHub asset digests. EXE/ZIP hashes are recorded in the v0.1.5 candidate entry below.
- Release-note preview URLs under v0.1.5/docs return HTTP 200. v0.1.4 tag (9c4868a) and its asset digests are unchanged.

## 2026-10-13 — v0.1.5 release candidate: right-click edit, skins and opacity, 34 px header

- 作者/最后修改人：lianzy。用户需求：①正文右键增加编辑，设置界面可选默认编辑应用；②标题栏再矮一点，图标同步缩小、圆润无锯齿；③设置界面增加皮肤（书本、古籍、常用颜色）并支持调整透明度。
- 右键菜单仅在正文区域出现（右键 / Shift+F10 / 菜单键）：用默认编辑应用打开（Ctrl+E）、选择编辑应用、复制、全选当前分页、查找、重新载入。文档以单个带引号参数传给编辑器，工作目录为文档所在文件夹；FeatherMD 仍只读。F5 重新载入保留源字节与换行行号。
- 新设置窗口（更多 → 设置…，Ctrl+,）取代独立字体对话框：外观（12 款皮肤卡片、窗口不透明度 30–100%）/ 阅读（原字体字号，控件 id 不变）/ 编辑（系统默认、记事本、检测到的编辑器、浏览 EXE）。草稿保存才生效；皮肤与透明度实时预览，取消还原。设置窗口原生标题栏随草稿皮肤着色。
- 皮肤：跟随系统、浅色、深色、护眼、书本、古籍、豆沙绿、天青、樱粉、雾灰、夜蓝、纯黑。书本/古籍在桌面底色上绘制纸页，古籍带朱砂双线栏。对比度脚本计算：正文 ≥ 9.8:1，次要文字 ≥ 4.3:1，链接 ≥ 4.7:1；护眼次要文字由 4.24 调到 5.06。
- 标题栏 40 → 34 px，按钮 30 → 26 px，图标 0.86 倍。描边宽度取半像素、中心对齐像素网格，窗口按钮等矩形图标坐标取整；96/120/144 DPI 截图放大检查：直线清晰、曲线抗锯齿。
- 修复：INI 以 UTF-8 BOM 保存时首个键被忽略（PowerShell 5 的 Set-Content 会写 BOM，ui-smoke 的“浅色”截图因此曾是深色）。含中文的冒烟脚本加 UTF-8 BOM，Windows PowerShell 5.1 才能正确解析。
- 证据（120 DPI，MSVC release 783,360 B，FileVersion 0.1.5，--locked 构建）：fmt --check、clippy -D warnings、cargo test --locked（44 项，含 12 款皮肤对比度下限测试）全部通过；以下原生回归在交付 EXE 上运行：ui（GDI 35→35）、navigation headings 454/535、wrapped 449/504、scrollbar、window（51 项命中）、typography 浅/深（GDI 35→35）、about 浅/深（GDI 22→22）、settings 浅/深（GDI 35→35）、editor 冒烟全部 PASS。
- wrapped 页数与 v0.1.4 记录的 473/523 不同；用 dist 中的 v0.1.4 EXE 在本机重跑同样得到 449/504，差异来自本机环境而非本次改动，原因未查明。
- 实机可见窗口检查（短暂显示后关闭）：80% 不透明度下标题栏像素与桌面混合（F4F6F3 → C8CAC9），圆角裁切保持；设置窗口标题栏随书本皮肤着色。主窗口 DWMWA_BORDER_COLOR 改为边线色在本机像素逐字节无差别，已恢复为 DWMWA_COLOR_NONE。
- Delivery: dist/FeatherMD.exe 783,360 字节（765 KiB），SHA-256 cc576f353784206352389aaa934ad3452d4b5425af7b466ec05562a332e73578；dist/FeatherMD-0.1.5-windows-x64.zip 874,161 字节，SHA-256 37029a56e5837ccdd7f2f75bf6f22764422f641f122039c86b14db61acc68140（新增 5 张截图）。构建、交付与 ZIP 内 EXE 哈希一致；早期 ZIP 均保留，此前的本地候选包改名为 0.1.5-candidate。全部 12 项原生回归针对交付 EXE 通过；演示文档首屏 80.99 ms 为单次本地小文件样本。
- 限制：隐藏窗口 + 原生消息测试，非物理指针；Ctrl+E 需要真实键盘状态，未自动化；编辑器用 examples/editor_probe 替身，未启动真实编辑器或 Windows edit 动词；CI 上用 -AllowNoPopup 容许服务桌面不显示弹出菜单。未做多显示器 DPI 与 Windows 10 实机验证。用户确认后推送 GitHub，CI 通过后发布；发布记录见上一条。

## 2026-10-12 — v0.1.4 compact header and outline click highlight release candidate

- 作者/最后修改人：lianzy。标题栏由 48 px 收窄为 40 px（逻辑像素）；搜索行、主题弹层、提示与目录面板随 HEADER_HEIGHT 联动。
- 修复：短章节同屏或文末无法把被点标题滚到顶行时，目录高亮停在最上方标题。点击后记录（标题索引，跳转后的顶行），顶行不变时高亮所点标题；滚动、跳转、重排、打开文件后回到按位置判断。跨页加载在页回复后再记录。
- 证据（120 DPI，MSVC release 738,304 B）：fmt --check、clippy -D warnings、cargo test 全通过；ui、navigation（headings/wrapped）、scrollbar、window、typography（light/dark）、about（light/dark）冒烟均 PASS。window-smoke 新增 8 个短章节夹具，依次点击第 3/6/1 个标题并用 WM_APP+42/46 断言高亮索引，Home 后回到 0。已目视检查截图并更新 docs 预览图。
- 限制：仍为隐藏窗口合成消息测试，未覆盖真实指针输入。
- Version bump: Cargo/Cargo.lock (feathermd only), app.rc FileVersion/ProductVersion 0.1.4, package.ps1 default 0.1.4. Rebuilt and reran fmt, strict Clippy, cargo test --locked (40 tests) and all 9 native regressions against the 0.1.4 build: PASS. About light/dark show v0.1.4 (GDI 22→22); previews regenerated.
- Delivery: dist/FeatherMD.exe 738304 bytes (721 KiB), SHA-256 33882b654634390d9b649aa8b66f2af167ff7ff9d1f06238ea3764a7ea2a3c87. dist/FeatherMD-0.1.4-windows-x64.zip 659473 bytes, SHA-256 081132a4ff717fd217bb7053e01356af165066fb2e3bfc7815ac4723763ddd6e. Build, delivery and ZIP EXE hashes match; earlier ZIPs preserved.

## v0.1.4 publication record — 2026-10-12

- 作者/最后修改人：lianzy。Public latest release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.4
- Tag/source: 9c4868a2ac0ee712cc5f6944b12730b4072bc0ce (annotated tag v0.1.4). GitHub Actions run 38269151582 (main) passed on Windows and Ubuntu, including the native Windows smoke steps; tag run 38269160330 also passed: https://github.com/Lianzy-Baimiao/FeatherMD/actions/runs/38269151582
- Assets: FeatherMD.exe 738304 bytes, FeatherMD-0.1.4-windows-x64.zip 659473 bytes, SHA256SUMS.txt 180 bytes (SHA-256 1971e812bb68c9e961fa1070efae231d918951689804d2a4fee6a487077f632e). All three were downloaded again; hashes match the local distribution and GitHub asset digests. EXE/ZIP hashes are recorded in the v0.1.4 entry above.
- Versioned release-note preview URLs under v0.1.4/docs return HTTP 200. Earlier tags and release assets are unchanged.
## 2026-10-12 — v0.1.3 anti-aliased icons, reading font settings and release candidate

- Header, welcome and About icons now use GDI+ anti-aliased vector paths with round caps, drawn at the window DPI. Before/after icon captures show 6 vs 216 edge colors. No icon font, downloaded assets or third-party runtime were added.
- More → 字体与默认字号: installed-font selector (212 faces on this host), 12–36 px default size, live preview, draft-only reset, Save/Cancel/Escape and restart persistence. Body text and headings use the chosen face; UI chrome, source view and code stay on their own fonts. The dialog follows the reader theme and handles WM_DPICHANGED.
- Release candidate build: fmt --check, strict all-targets Clippy, all 40 tests (2+4+19+8+3+4) and the static-CRT MSVC release build pass.
- Native regressions against the 738304-byte candidate (second full batch, 9/9 PASS): UI smoke GDI 35→35; navigation headings 454/535 and wrapped 473/523 pages, 13 font sizes each; scrollbar both axes, vertical middle byte 1834737 of 3675600, wrapped row 25 preserved; window 51 hit tests, resize anchor 941400 preserved; typography light/dark (4 invalid sizes rejected, GDI 31→31); About light/dark (GDI 22→22, browser not launched).
- Intermittent failure: in the first full batch, scrollbar smoke failed once with "Cached paragraph thumb did not move through wrapped rows" (scripts/scrollbar-smoke.ps1). It passed three isolated reruns and the complete second batch. The cause was not identified; treat it as an unresolved possible flake, not a fix.
- README previews regenerated from this build: reading light/dark/outline-hidden, welcome light/dark, About light/dark and new typography light/dark. File-association menu capture unchanged.
- Delivery: dist/FeatherMD.exe 738304 bytes (721 KiB), SHA-256 d199bcde3309e786e7217ee6f8fb2b6a7345a665e576dce0d9776319869be16f. Build, delivery and ZIP EXE hashes match. dist/FeatherMD-0.1.3-windows-x64.zip 659871 bytes, SHA-256 1189ad4b2abbae5586dd57476a4cd41555100b454eb131576aa0c2ad6cc04a01. The earlier local 0.1.3 archive was preserved under an -early suffix. FileVersion 0.1.3, CompanyName lianzy. Delivery EXE UI smoke PASS, GDI 35→35; demo first layout 90.43 ms is a single local sample.
- Boundaries: hidden windows and native messages, not physical pointer input; no multi-monitor DPI, Windows 10, Snap or full accessibility validation. Missing glyphs rely on Windows font fallback. No production association changes were performed.


## v0.1.3 publication record — 2026-10-12

- Public latest release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.3
- Tag/source: 4daaaab282e60df04732bff3a9f9d470e2bd22a1 (annotated tag v0.1.3). GitHub Actions run 38255735339 passed on Windows and Ubuntu, including the native Windows smoke steps: https://github.com/Lianzy-Baimiao/FeatherMD/actions/runs/38255735339
- Assets: FeatherMD.exe 738304 bytes, FeatherMD-0.1.3-windows-x64.zip 659871 bytes, SHA256SUMS.txt 180 bytes (SHA-256 6ef32e3da1bc36a02817fd508c60630209def768d07919fa4cfc51f9bbb84359). All three were downloaded again; hashes match the local distribution and GitHub asset digests. EXE/ZIP hashes are recorded in the v0.1.3 entry above.
- Versioned README preview URLs under v0.1.3/docs return HTTP 200. Earlier tags and release assets are unchanged.
- Follow-up: investigate the single unreproduced cached-paragraph scrollbar smoke failure; update deprecated Node 20 Actions (checkout/upload-artifact v4) and note the ubuntu-latest migration to Ubuntu 26 starting 2026-10-19.

## 2026-10-12 — main header and welcome redesign (local 0.1.3 revision)

- Corrected the scope after the user's clarification: rebuilt the main reader/home header, not About. Preserved the existing uncommitted work and all previous packages. No repository reset, commit, push, tag or GitHub publication.
- Replaced the wide text controls with locally drawn feather/line icons, a compact preview/source capsule, folder/search/ellipsis actions and one appearance button showing sun/moon/system state. Controls retain a single 48-logical-pixel header. The application-action area shrank from 400 to 260 logical pixels; window buttons remain separate. Header blank space still drags the native window.
- Added a theme-aware appearance picker (light/dark/system), delayed shortcut tooltips and Ctrl+T. Arrow/Tab navigation, Enter/Space selection, Escape, repeat-button toggle, outside click, focus loss and resize dismissal are implemented. The picker sits below an open native search EDIT row rather than painting over a child HWND. Theme changes still save/repaint without relayout; source/preview semantics and More/About/association actions remain intact.
- Redesigned welcome with a centered feather mark, typographic hierarchy and a single open/drop entry. Suppressed the empty sidebar without changing the saved outline preference; removed redundant welcome footer text. Updated README instructions, light/dark reading/welcome screenshots and package contents.
- Validation: MSVC static-CRT build, fmt --check, strict all-targets Clippy and all 39 tests pass. Final window smoke passed three consecutive runs plus the delivery EXE: 51 frame/control hit tests, 640x480 logical minimum (800x600 physical at 120 DPI), actual picker click/keyboard/dismissal paths, tooltip delay, search-row avoidance and restart persistence. Source byte 941400 is retained on resize. Scrollbar regression preserves wrapped row 25 through theme changes. Navigation: headings 454/535 pages, wrapped 473/523, each exercising 13 native font sizes. About light/dark tests pass with GDI 19→19.
- Test-harness corrections: hidden synthetic mouse moves must not register real desktop leave tracking, and hidden windows need explicit painting before overlay hit-testing. A transient GDI sample saw 30→43 while another run was 30→30; the leak test now samples the minimum across a short quiescence window instead of counting in-flight paint resources, without relaxing its retained-growth threshold. It also exercises ten actual picker paints. Final UI and delivery UI: GDI 31→31. One early synthetic source-click attempt failed once; no cause was proven, and it did not recur in the final three repeated window runs or delivery run. Do not claim real-pointer validation from these results.
- Delivery: dist/FeatherMD.exe, 718848 bytes (702 KiB), SHA-256 8192f9c347dbddebf8bb9213bc73faeb550405de82894b2ef6a3ad4fa99e6789. dist/FeatherMD-0.1.3-header-ui-windows-x64.zip, 579683 bytes, SHA-256 c70d0736274b49826e64b9eadebf7d7530f95b0b5827ee67f9251a7b608a2b6e. Build/delivery/ZIP EXE hashes match. All previous versioned ZIPs are preserved. EXE version remains 0.1.3; this is a local UI revision.
- Boundaries: native messages and in-process snapshots, not physical mouse automation, full accessibility validation, real system-wide theme changes, physical multi-monitor DPI tests or Windows 10 testing. The diagnostic child-EDIT print can show standard white in dark search snapshots because its parent color callback re-enters an active App borrow; do not treat that snapshot artifact as proof of the visible EDIT's theme. No association/registry changes, actual browser launch or new large-file benchmark were performed. Delivery demo first layout 106.71ms is only one local small-file sample.

## 2026-10-12 — About redesign and project/support badges (local)

- Preserved the user's existing uncommitted v0.1.3 work. More now has an explicit “关于 FeatherMD…” entry (F1). Fixed destinations: https://github.com/Lianzy-Baimiao/FeatherMD and https://ifdian.net/a/lianzy; the support URL and flat-square MIT/purple 爱发电 badges were checked against WowAltBoard's README.
- After the user rejected the initial stock Task Dialog appearance, replaced it with a native modal card layout: feather mark, clear title/version hierarchy, GitHub/support cards, optional shortcuts, safe Return to reader action and custom frame. It uses the reader's light/dark/sepia/system preference, with Windows theme notifications and DPI/work-area fitting; no web renderer, remote assets or additional Cargo dependencies.
- Standard STATIC/BUTTON children retain names, focus and keyboard behavior. A native regression exposed Enter on a focused BS_OWNERDRAW link closing About instead of activating the link: the default dialog manager was choosing the safe default action. The button subclass now explicitly requests/handles Enter; Tab → GitHub → Enter reaches the confirmation. Escape, F1, help expansion/collapse and repeated open/close also pass.
- The deferred About launch and nested link confirmations release state borrows before entering modal loops. Both fixed external URLs require confirmation, default Cancel; tests cancel rather than launching the browser. No production association registration/unregistration, UserChoice write or default-app change was performed.
- Hidden cross-process PrintWindow sometimes returned success with a completely black image. Extracted the existing reader's opt-in, in-process GDI snapshot helper for reuse by About, including real child-control painting; added background-pixel checks. Final light/dark/expanded/keyboard-focus captures were inspected, and README/package include the final light/dark previews. The capture seam remains inert without FEATHERMD_TEST_SNAPSHOT.
- Final validation: fmt check, strict all-targets Clippy, 39 tests and MSVC static-CRT release build pass. About passes in light/dark/sepia/system; GDI 19→19 over eight additional open/close cycles per run. Reader UI, window (49 hit tests), scrollbar and navigation shapes pass: headings 454/535 pages, wrapped 473/523 pages, each with 13 native font sizes. Delivery EXE also passes light/dark About and UI smoke (GDI 31→31). Demo first layout 72.30ms is one local small-file run, not a cold-start or large-file benchmark.
- Delivery: dist/FeatherMD.exe, 709120 bytes (~693 KiB), SHA-256 2141282b47398c96383e4e004f8b3ec419fffb3c76ebd1e361f7c1db4fc45619. dist/FeatherMD-0.1.3-about-ui-windows-x64.zip, 568242 bytes, SHA-256 9855dc1876db833245208970cc5f9d9c321576a5e813c5b7477650b636340ac8. Build/delivery/ZIP EXE hashes match; package includes README and its About screenshots. All pre-existing ZIPs are preserved. EXE version remains 0.1.3; this is a local UI revision, not a GitHub release. No commits/pushes/tags/publication in this turn.
- Boundaries: native message tests, not desktop pointer injection or full assistive-technology validation. No actual browser handoff, system-wide theme change, physical multi-monitor DPI transition or Windows 10 run was exercised. Confirmation/error dialogs remain standard Windows dialogs. Large real-file benchmarks were not repeated.

## 2026-10-12 — local header switches and association verification

- Continued the existing uncommitted v0.1.3 work without resetting user edits. Built the always-visible preview/source and light/dark/system segmented controls; removed their dependency on the More menu. New settings default to system, explicit choices persist, and WM_SETTINGCHANGE / WM_THEMECHANGED refresh the resolved palette.
- Added a regression before fixing a related reading-position bug: `scripts/scrollbar-smoke.ps1` reported `Header command 209 moved the reading position: wrapped row 25 -> 0`. Cause: theme commands called the general persistence path, which rebuilt layout from the physical source byte. Theme-only changes now save preferences and repaint without relayout; repeated selection of the current preview/source mode does nothing. All six tested theme/current-mode commands preserve the wrapped row.
- On this host at 120 DPI, the scrollbar test initially missed the thumb because its caller used virtualized coordinates. The script now opts into per-monitor-aware physical coordinates and restores its prior DPI context; it then reproduced the theme regression and passes after the fix.
- Validation: fmt, strict all-targets Clippy, 38 tests, MSVC static-CRT release build, UI smoke, window smoke, scrollbar smoke and both navigation shapes pass. Window smoke: 49 frame hit tests, 800x600 physical minimum at 120 DPI (640x480 logical), top switches including narrow layout and restart persistence, source byte 941400 retained on resize. Headings navigation 454/535 pages; wrapped navigation 473/523 pages; both exercise 13 actual font sizes.
- Delivery EXE also passed UI/window smoke. GDI 27→27. Demo first layout 87.53ms is a single local run, not a cold-start or large-file measurement. Final light/dark/narrow/welcome captures were inspected; README previews now match the local build. No large real-file benchmark was repeated in this turn.
- Association symptom did not reproduce on the current host: read-only SHAssocEnumHandlers includes FeatherMD for .md, and UI Automation inspected FeatherMD in the Windows Default apps application list and its dedicated page. Registration still points to an older executable outside the project. The user clarified that they personally clicked to choose FeatherMD as default during inspection; do not attribute that choice to the agent. No production registry registration/unregistration was performed in this turn; the existing registry tests use isolated subtrees.
- Added `scripts/association-inspect.ps1`, a read-only Shell candidate probe, and documented fixed-location registration, re-registration after moving, Settings refresh and browse-to-EXE fallback. Registration is still opt-in, per-user and never writes UserChoice or extension defaults. Do not claim a reproduced registration root cause or universal Windows compatibility from this host.
- Local delivery: `dist/FeatherMD.exe`, 696832 bytes (680 KiB), SHA-256 `1def124a0a90f8edc9a66d07a4ade24ed7cdb5b589369fa16f1d84259da30fd2`; identical to target/release. `dist/FeatherMD-0.1.3-top-switches-windows-x64.zip`, 524522 bytes, SHA-256 `aa6e597648db6030863fed6b9eea0c4704e5c7ba08dff872cce1a98004591874`. Preserved all pre-existing versioned ZIPs including the earlier 0.1.3 archive. This is a local revision, not a new GitHub publication; no commits, pushes, tags or releases were made.
- Boundaries: native smoke uses hidden windows/messages, not real pointer input or a real system-wide theme change. Theme mapping is covered by pure tests and native refresh/persistence checks; physical monitor transitions and Windows 10 Settings UI remain unverified. dumpbin reports Windows-provided DLLs only.


## 2026-10-12 — v0.1.2 local UI, scroll and outline delivery

Author / company / last editor: lianzy.

- User requested working scrollbars, a non-native border, a cleaner UI, a collapsible heading outline, and removal of the separate toolbar. Implemented a single 48px custom title bar with file title, find, open, more and window controls; remaining actions live in the popup menu. The outline collapses from its own heading and expands from the left content edge; no duplicate toggle remains in the application title bar. Ctrl+B toggles the outline and its preference persists across restarts.
- Reproduced old v0.1.1 twice with actual pointer input: dragging the native thumb to the middle left checkpoint 00000 / 0%. A one-change probe releasing the App borrow before DefWindowProc reached checkpoint 02457 / 49.1%, confirming native tracking reentrancy as the original cause.
- Custom scrollbars share actual viewport state, support both axes and page clicks, use 64-bit geometry, preserve wrapped-paragraph progress and invalidate stale page requests. A regression caught WM_LBUTTONDBLCLK (click followed quickly by drag): old code stayed at byte 0; the fixed branch reaches byte 1,834,737 in the 3,675,600-byte fixture. WM_CANCELMODE releases capture.
- scrollbar-smoke PASS: both axes, drag before release, double-click drag, wheel, track paging, capture loss, three rapid out-and-back seeks, short-document hiding and a 4,060-byte single paragraph with 28 wrapped-row positions.
- window-smoke PASS: 39 frame hit tests, no WS_CAPTION/WS_VSCROLL/WS_HSCROLL or attached native menu, 640x480 client layout at 96 DPI, byte 941,400 preserved across resizing, 1,200 headings, outline click at byte 7,845, independent outline wheel, hide/reopen through actual buttons and preference persistence. Search input remains clear of the collapsed outline button at narrow width. Captured light/dark/search/narrow/empty states.
- Core checks: formatting, warning-free Clippy, 34 tests and static-CRT MSVC release build passed. Navigation regression: heading shape 357 forward / 419 backward pages; wrapped shape 251 / 280 pages, both with 13 actual font settings.
- UI smoke: 29→29 GDI handles. Real 1 GiB Markdown smoke: 107.86ms first layout (one warm-cache sample), 5.42MiB private memory, 19.54MiB working set, GDI 28→29. Not a cold-start guarantee; first-layout timing is measured before waiting for the outline worker.
- Actual pointer checks separately exercised a fresh Release instance reaching 50.7% / checkpoint 02536 by thumb drag. A diagnostic build also verified custom maximize/restore (1080x820 ↔ 2240x1368). Earlier reused-window pointer attempts did not always move content; this led to the explicit double-click regression above. Only successful observations and native-message assertions count as verified evidence. All temporary input tracing was removed.
- Outline limits: separate cancellable worker, 256KiB reads, up to 16MiB scanned, 4096 ATX headings, 160 characters per label, 4096-byte long-line prefix. Code-fence content is excluded. Limits are shown in the sidebar; Setext headings are not indexed. F5 rebuilds the outline. Reading and full-text search continue beyond its range.
- Local delivery: dist/FeatherMD.exe (684,544 bytes) and a new v0.1.2 ZIP (468,999 bytes), including README previews. Build/standalone/ZIP executable hashes match: 23c1b618040df097d043f819eb7aad5e38f31df3cd0689b1607e2f6becff54d5. ZIP hash: a8a4b47657319b73068bb90f088601e7e56eca73ffea53b421ab44a6b012f924. Existing v0.1.0/v0.1.1 packages/tags are preserved. Executable metadata explicitly names lianzy; dumpbin confirms only Windows-provided dependencies. Final packaged EXE passes UI smoke (GDI 29→29). GitHub publication was completed at the user's explicit request; see the record below.
- Remaining platform checks: real multi-monitor DPI transitions, Windows 10 visual verification, snap layouts and isolated default-association integration. Popup menus and file dialogs remain OS-provided; the application frame and scrollbars are custom. Full document/outline accessibility semantics remain future work.

## v0.1.2 publication record — 2026-10-12

- Public latest release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.2
- Tag/source: 8df8bbb5e22a75eb0a2344d223a74bb6e6c38a7f. GitHub Actions run 38198642207 passed on Windows and Ubuntu, including all native smoke tests: https://github.com/Lianzy-Baimiao/FeatherMD/actions/runs/38198642207
- README now contains final light, dark, collapsed-outline and welcome previews. Versioned release preview URLs resolve against v0.1.2.
- Published EXE, ZIP and SHA256SUMS.txt were downloaded again; all SHA-256 hashes match the local distribution and GitHub asset digests. Package sizes and hashes are recorded above. The tag points to the tested source commit; existing releases and tags remain untouched.
- Follow-up work must start from remaining bounded improvements, not repeat the completed scrollbar, frame or outline-control placement work. The current Actions run also warns about deprecated Node 20 actions; update these separately against current upstream releases.

## User intent

Build and keep polishing an ultra-light Rust Windows Markdown viewer, fast first display independent of whole-file loading, settings, opt-in default association, ideally a single EXE; upload working results to Lianzy-Baimiao on GitHub. User requested continued unattended work. Do not disturb existing repositories or default apps.

## 2026-10-12 — initial implementation

- Repository/project: FeatherMD. Native Win32 + GDI, no web runtime, 3 normal direct dependencies.
- Implemented bounded background I/O, UTF-8 segment alignment, source/preview, cancellable original-text search, byte-percent navigation, native menus/open dialog/drop, row selection/copy, 3 themes/zoom/wrap/width, atomic settings, per-user opt-in registration.
- Single EXE uses static MSVC CRT and embeds icon/manifest/version/licenses. Windows 11 x64 local tests passed.
- Tests: 17 core tests including 100 GiB sparse seek; fmt + warning-free Clippy; native hidden-window smoke against 807 B demo and real 1 GiB Markdown; first-layout around 75 ms for the 1 GiB fixture on this machine. See PERFORMANCE.md for exact boundaries.
- Fixed during testing: UTF-8 scalar split at end of read window, wrap-around search missing overlapping origin, UTF-16 BOM handling, Win32 destruction reentrancy, cross-process test input/capture assumptions.
- Local machine defaults to GNU Rust but has MSVC/SDK in a custom location. Use cargo +stable-x86_64-pc-windows-msvc and source ignored target/dev-env.ps1; do not publish that machine-specific file. Do not change the global default toolchain.
- Source published: https://github.com/Lianzy-Baimiao/FeatherMD (main, initial commit 658ea5c).
- Initial GitHub Actions run 38168176475 passed on Windows and Ubuntu, including the native Windows smoke test.
- Final 640 KiB EXE revalidated locally: fmt, Clippy with warnings denied, all 17 core tests, release build, small-document and real 1 GiB native smoke all passed. v0.1.0 packaging/publication completed; see the release record below.
- Hourly continuation remains active after the initial release, using the bounded high-value backlog below; no cosmetic churn.

## v0.1.0 publication record — 2026-10-12

- Public release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.0
- Tag/verified source: c63a265ab03d0c84b42c26325bc83029697188d1. GitHub Actions run 38168408623 passed on Windows and Ubuntu, including native Windows smoke.
- Published assets: FeatherMD.exe (655,360 bytes), FeatherMD-0.1.0-windows-x64.zip (320,434 bytes), SHA256SUMS.txt. GitHub asset SHA-256 digests match local package hashes.
- EXE SHA-256: b8bea5f9d41aa1cf4feeffd54786187a18256a2d42fc99ea6d6ed5d4a09efb8e
- ZIP SHA-256: e91b6a293c3024ccd69fa81e87de4d5c9781b0a2aedb1be9b08b431b0fdc18a0
- Default-app registry mutation was deliberately not exercised on the development user's system. Document this as an outstanding opt-in integration test, not as a verified end-to-end association change.
- Initial delivery is complete. Continue meaningful polish via the hourly heartbeat; preserve published assets and tag, and use a new version for future binaries.

## v0.1.1 navigation work — 2026-10-12

- Reproduced the original End defect with a native keyboard/capture test: only one row was painted despite a long document. The wrapped fixture separately exposed End targeting the first visual row of the final physical line.
- Added pure pixel-height viewport logic and native integration. Fixed EOF placement, heading page skips, fixed-byte backward jumps, wrapped-row refill resets and off-screen copy range.
- Source I/O limits, static CRT, no-network behavior and user-owned default associations are unchanged.
- Added 4 pure tests (21 total), real-window navigation tests and CI coverage. Heading run: 555 forward / 655 backward pages; wrapped run: 393 forward / 447 backward pages. Both exercise 13 actual font settings. Simulated DPI tests are not multi-monitor validation.
- Release candidate v0.1.1 passed local fmt/strict Clippy/21 tests/release build, standard UI smoke, both navigation shapes and real 1 GiB smoke (59.60 ms first layout, single warm-cache sample). EXE: 656,896 bytes. Published after CI and asset verification; see below. v0.1.0 assets and tag remain unchanged.

## v0.1.1 publication record — 2026-10-12

- Public release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.1
- Tag/source: 2f2c002bf0f83f3fcd1e6fd5f0a8b75fbae0a105. GitHub Actions run 38175299658 passed on Windows and Ubuntu; Windows includes both new native navigation shapes.
- Assets verified uploaded: EXE 656,896 bytes, ZIP 330,197 bytes, SHA256SUMS.txt. GitHub SHA-256 digests match local packages.
- EXE SHA-256: 4f2355d7b740821e4627e2971bc22622163303abe3c8f84535f2f905cd5f3eca
- ZIP SHA-256: 27f7afed4fb2cef41d613233c8e95c543531867e1ce69a8cdd74a4a3d0e628f0
- dumpbin confirms only Windows-provided DLL dependencies. Local distribution path remains dist/FeatherMD.exe; older versioned ZIP is preserved.
- Next continuation should pick an unfinished item below rather than repeat the completed EOF/paging work.

## Next high-value polish work (not all required for the initial release)

1. v0.1.1 covers bidirectional heading/wrapped-line cache crossings, EOF placement and 13 native zoom settings. Remaining: selection/copy integration, preserving reading position across resize/zoom, and physical multiple-monitor DPI transitions.
2. Improve Markdown context continuity on sequential page navigation using bounded cached checkpoints; do not scan entire huge files or make arbitrary jumps wait. Add tests that distinguish sequential correctness from explicitly context-limited random jumps.
3. Native accessible reading/text selection and keyboard focus ergonomics. Keep any accessibility projection bounded to the viewport.
4. Assess bounded table layout and local image support only if they do not inflate the binary or block the UI; maintain explicit no-network defaults.
5. Test registration/unregistration in an isolated Windows user/VM; preserve other applications and never touch UserChoice. Update deprecated GitHub Actions runtime versions with verified upstream releases.

## Guardrails

- Do not rewrite version tags or existing release assets silently. New code merits tests and a new version when published.
- Do not claim complete CommonMark rendering, instant arbitrary-size display, or cold-start performance from warm-cache measurements.
- Keep heartbeat notifications quiet unless there is a meaningful verified improvement, publication, failure or required user action. Pick a real bounded improvement per continuation; no cosmetic churn just to appear busy.
