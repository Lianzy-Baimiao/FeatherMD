# Microsoft Store 上架可行性与政策评估

核对日期：**2026-10-17（北京时间）**。对象：Windows x64 原生 Rust / Win32 / GDI Markdown 阅读器 FeatherMD。

范围：只查阅 Microsoft 一手公开文档，并读取项目 `AGENTS.md`、`docs/PROGRESS.md`。没有登录或操作 Partner Center，没有预留名称、申请证书、安装测试包、改代码、改变发布设置或提交审核。本节是政策与分发方案研究，不是 MSIX 实测结果；项目代码审计可另行附在本文末尾。

## 1. 结论与政策时点

**可以申请上架，不需要为了 Store 重写成 UWP、WinUI 或网页应用。优先考虑在现有原生程序外增加 MSIX 包，继续独立保留 GitHub 便携版。** Microsoft 接受封装桌面应用的 MSIX，也接受符合要求的非游戏 EXE/MSI 安装器；后者不是任意裸 EXE 或 ZIP 的下载链接。[S6]、[S7]、[S11]

- **推荐 MSIX**：Microsoft 托管、分发并签署 Store 包，提供商店更新；不必为这一条 Store 分发路线购买 CA 代码签名证书。Store 不会因此替现有 GitHub 裸 EXE 补签名。[S6]、[S13]、[S14]
- **帐号原则上可复用**：若用于 LiteView 的现有 Windows 开发者帐号仍有效、验证正常，且帐号类型符合实际活动，可在其 Apps & Games 工作区新建 FeatherMD 产品，不需因为“第二个应用”再注册帐号。每个产品仍需自己的名称、标识和提交。未查询帐号实际状态，也未验证 FeatherMD 名称是否可用。[S4]、[S5]
- **不等于已经能直接提交或必过**：仍要做打包、隐私政策、商店资料、打包后的功能/安装测试及认证；旧版发布记录和普通原生冒烟测试不能替代这些工作。[S1]、[S6]、[S16]

特别注意政策生效日：当前主页面展示 **v7.20，2026-09-15 发布、2026-10-22 生效**；本次 10 月 17 日评估的现行基线是 **v7.19，2025-10-14 生效**。已对照新版及变更记录；本文涉及的签名、个人信息、账户、赞赏等关键条款没有因此变成相反结论。实际若在 10 月 22 日或之后提交，应按届时生效政策重新核对，尤其保持年龄评级准确。[S1]、[S2]、[S3]

## 2. 两种分发路线

| 项目 | MSIX 封装现有 Win32（推荐） | EXE/MSI 外部安装器 |
| --- | --- | --- |
| 提交物 | `.msix` / `.msixbundle` 等支持的包，含清单、程序、图标等资源 | 指向 `.exe` 或 `.msi` **安装器**的版本化 HTTPS 直链 |
| Store 签名 | 认证通过后的发布过程中由 Store 重签 MSIX/AppX；无需自备受信 CA 证书 | Store **不重签**；安装器及其中所有 PE 文件必须由 Microsoft Trusted Root Program 信任链中的代码签名证书签署 |
| 托管 | Store 托管与分发包 | 开发者维护可用、可靠的外部托管/CDN；提交后的 URL 对应二进制不得替换 |
| 安装 | 按 MSIX 模型安装、升级、卸载 | 必须离线完整安装器、静默安装；不能运行后再下载主体；允许 UAC，但不允许安装向导 UI |
| 后续版本 | Partner Center 新提交，审核通过后由 Store 分发更新 | 新二进制必须提交新的版本化 URL；不能悄悄覆盖原 URL 或使用会变动的 `latest` 作为版本定位 |
| 对 FeatherMD 的工作 | 增加包标识、资源、文件关联和 Store 渠道行为，验证包内运行 | 除程序外还要维护真正的安装/卸载器、静默参数、签名、外部下载和升级行为 |

表中硬性要求来自包文档及政策 10.2.7、10.2.9；路线取舍是工程建议。[S1]、[S6]、[S11]、[S12]、[S14]

**不要混淆签名场景**：本地部署测试或 Store 外分发 MSIX 仍需满足设备信任/签名条件；Store 免费签名并不等于可以随意安装未受信的本地测试包。Microsoft 的 Azure Artifact Signing 公共信任服务目前也不是中国大陆个人开发者的通用替代方案：其公开资格文档限个人开发者于美国、加拿大。不要把“买个 Azure 套餐即可”作为本帐号已验证的方案。[S13]

## 3. 帐号、名称和提交资料

- **必须满足帐号类型政策**：10.14 规定组织、企业及与自身商贸/职业活动相关的发布需使用 Company 帐号；Individual 通常适用于独立个人的非商业/非职业项目。不是“收费才算企业”，也不是“有可选赞赏就必定要公司帐号”。需按真实发布身份和活动判断；发布过 LiteView 不构成本次豁免。[S1]、[S4]
- **大陆现有帐号与新开户注册是两件事**：本建议复用现有有效帐号，不建议更改地区或重新开户绕过验证。没有用新开户注册资格、Azure 签名服务地域资格来推导现有帐号不能新增应用；最终以帐号实际权限、验证及适用条款为准。[S4]、[S13]
- **每个产品需独立身份**：名称要可用且不侵权；包中 Identity Name、Publisher 等应逐字匹配该产品 Partner Center 提供的值，不能复用 LiteView 的包身份。可只提交 x64，但清单和商店系统需求应反映真实支持范围。[S5]、[S6]、[S7]
- **不要混淆产品版本与包版本**：未发现禁止产品使用 `0.x` 用户可见版本号的条款；但 MSIX 提交文档的 Windows 10/11 包版本规则要求 Identity.Version 为四段、首段非 0、末段保留为 0。可保留 Cargo/界面的 `0.1.10`，另外映射合规 Store 包版本，例如 `1.0.10.0`，并验证后续递增策略；不能直接假定 `0.1.10.0` 可作为 Store 包版本。[S6]
- **提交资料必须真实完整**：合适分类、功能说明/限制、实际截图、支持的语言、目标市场与系统要求，以及 IARC 年龄评级。不能用本机 Windows 11 测试宣称所有 Windows 10/所有辅助技术均已验证。目标市场仍有当地法律与内容要求。[S1]（10.1、10.4、10.7、11.10、11.11）

## 4. MSIX 对原生程序的关键约束

### 原生权限与文件激活

- **必须正确声明运行模型**：普通完整信任桌面进程可用 `packagedClassicApp` + `mediumIL`（较旧系统需相应 EntryPoint 形式）；这种包需声明 `runFullTrust`。它表示中完整性级别桌面运行，**不是管理员权限**，也不是把应用改成 UWP AppContainer。提交时按 Partner Center 要求解释受限能力用途并接受审查，不应预先保证批准。[S7]、[S8]
- **只声明必要能力**：现有 Win32 文件 API 和 WinHTTP 路线不应机械照抄 UWP 能力列表；`mediumIL` 不因普通联网就必须声明 `internetClient`。不要仅因“阅读文件”就增加 `broadFileSystemAccess` 等不必要的受限能力。[S8]
- **若保留文件关联，使用包清单集成**：通过 `windows.fileTypeAssociation` 声明 `.md`、`.markdown`，以及确实支持且测试过的其他扩展名，使应用成为“打开方式”候选。用户仍决定默认应用；政策 10.2.8 要求受支持的方法和用户同意，禁止把“注册候选”做成静默抢默认。[S1]、[S9]
- **建议区分 Store/便携操作入口**：Store 版不要照搬指向版本化 WindowsApps 安装路径的便携自注册/反注册流程；验证文件关联实际激活、含空格/中文路径和多选文件策略，不能仅测试应用内“打开文件”。最终清单和参数传递方式需要打包验证。[S7]、[S9]

### 设置、升级与便携版共存

- **不可把可写配置依赖在包安装目录**：MSIX 包内容受保护；EXE 相邻的便携 INI 不应随包分发并被当作日常写入目标。使用用户可写数据位置，保留正常的首次启动与错误处理。[S10]
- **AppData/注册表兼容是测试项，不是自动保证**：文件和注册表重定向与所选运行/虚拟化模型及 Windows 版本有关，不能假定 `%APPDATA%` 一定共享或一定隔离。应实测首次设置、重启、升级、卸载后清理以及 GitHub 便携版并存；不要为“修复”而写入受保护位置或损坏其他应用关联。[S1]（10.2.7–10.2.8）[S10]
- **推荐 Store 版走 Store 更新**：保留 GitHub 公共版本信息查询并不等于下载/安装程序，不能仅因有 GET 或外链就判定已经违规；但不应把商店用户导向一个会产生另一份便携程序的“升级”流程。建议 Store 渠道隐藏/改向 GitHub 下载按钮，使用 Store 更新入口，便携渠道保留现有行为。政策 10.2.5 涉及 Store 安装/更新路线，正式的 MSIX 更新流程是提交新包给 Store；是否保留纯版本信息查询，应在认证说明中讲清楚。[S1]、[S14]

## 5. 隐私、联网与可选赞赏

### 隐私政策是硬要求，不是因为“默认离线”就可省略

**政策 10.5.1 明确把 Desktop Bridge 和 Win32 列为始终需要隐私政策的产品类型。** 提交时提供有效隐私政策 URL；当前提交界面文档也描述了直接填写政策文本的入口。建议同时发布稳定、公开可访问的 HTTPS 页面，并在 About/设置中给出入口；应用内链接是建议，不应与政策中明确的提交要求混为一谈。[S1]、[S18]

政策必须如实说明所访问/传输的信息、用途、存储/安全、接收方类型和用户控制；建议同时给出隐私联系渠道。对本项目至少应覆盖以下实际设计；最终文案须与代码核对：[S1]（10.5）

- 本地 Markdown、路径、设置及本机图片在何处处理/保存；不把“开发者没有自有上传服务器”写成“任何情况下都不会联网”。
- 更新检查默认关闭，手动检查或用户启用自动检查时访问 GitHub 公共 API；GitHub/网络服务会接收到请求所必需的网络信息。不要在未核对请求头和接收方政策时承诺“零个人数据”“第三方不记录 IP”。
- 每张网络图片需用户确认完整 URL 后才 GET；目标站点会获知请求网络信息，URL 路径/查询字符串也会被发送。图片请求与更新检查是两个独立入口，关闭前者或后者都不能被表述为关闭所有联网。
- 清楚说明不发送文档正文、无 Cookie/认证/Referer、是否跟随重定向、远程图片内存缓存和清除时机等经代码验证的行为；第三方的留存规则不能代替其承诺。

**条件风险**：10.5.4 要求个人信息的采集、存储、传输采用现代加密。允许 HTTP 图片并不意味着所有普通 HTTP 图片必然被 Store 禁止；但若 URL/query 带可识别或敏感信息，明文请求不能靠“用户点过确认”弥补。**建议 Store 版限制为 HTTPS，或在提交前专门处理这一风险**。这是风险收敛建议，不是捏造一条“Store 禁止所有 HTTP”的统一规定。[S1]（10.5.2、10.5.4–10.5.5）

### About 中的“支持作者”外链

**可选第三方赞赏链接不是当然禁止。** 10.8.2 允许通过安全第三方支付接收自愿赞赏，并允许安装完成后转到浏览器完成交易；要求实际交易流程具有提供方识别、身份验证、用户确认及适用的支付安全保障，并在 Partner Center 申报第三方支付使用情况。[S1]（10.8.2）

就目前描述的 `https://ifdian.net/a/lianzy` 入口，建议保持“完全自愿、不影响阅读功能、不解锁数字商品/服务”的清楚表述，确认后才打开浏览器，并在提交说明中讲明性质。**没有审计爱发电的实际付款页、回报设置或合规资质，因此不能据此证明整条付款流程已符合政策。** 若将来增加赞赏后解锁功能/去广告，要重新检查 10.8.1–10.8.2 的数字商品规则，不能继续按无回报赞赏处理；若需要金融信息作为主要功能，也要重新检查 Company 帐号要求。[S1]（10.8、10.14）

## 6. 可访问性、WACK 与验收边界

- **不能虚报“已满足可访问性指南”**：这是商店产品声明。官方要求相关 UI 名称、键盘操作、对比度、Inspect/AccChecker 等检查，以及 Narrator、放大镜、高对比度、高 DPI 下主要场景的端到端验证；只通过 native 控件键盘冒烟或截图检查不够。[S15]
- **缺少完整 UIA 不等于自动判定不能上架**：引用的声明文档没有规定所有 Win32 应用必须勾选该项才可提交。但正文、目录和自绘控件缺少读屏语义是实际使用障碍，建议作为商店版质量工作重点；未验证前不勾选该声明、不在商店宣传读屏支持。其他适用法律义务并不会因为不勾选而自动消失。[S1]（10.1、10.4）[S15]、[S17]
- **WACK 是必要的提交前验证环节，不是通过认证的充分条件**：MSIX 技术认证使用 WACK，但审核还包括安全扫描、内容/政策检查、受限能力用途。应对最终包运行适用测试并保留报告，不能用 `cargo test`、绿色 CI 或裸 EXE 的 UI 冒烟代替，也不能把 WACK 通过称为“微软已批准”。[S6]、[S8]、[S16]
- **建议打包验收矩阵**：干净标准用户环境；宣称支持的 Windows 版本；安装/启动/更新/卸载；实际文件关联激活；设置落点及便携版并存；无网络/代理/证书失败；远程图片逐张同意；屏幕缩放/键盘/Narrator。提供小型本地测试文档和清晰认证说明，不要求审核人员自行构造巨大文件才能确认基础功能。[S1]（10.3–10.4）[S16]、[S17]

建议执行顺序：先确认复用帐号与产品名称可用性，再确定 MSIX 渠道/清单与隐私政策，完成包内适配及上述验证，最后由用户授权后提交。本文没有替用户执行其中任何帐号或发布操作。

## 7. Microsoft 一手来源

以下页面均于 2026-10-17 获取；正文对必须、建议和条件判断分别表述。旧技术文档可能包含较早 Windows 版本的细节，实施时应以实际目标 OS、清单 schema 和认证结果验证。

[S1]: https://learn.microsoft.com/en-us/windows/apps/publish/store-policy-archive/store-policy-7-19
[S2]: https://learn.microsoft.com/en-us/windows/apps/publish/store-policies
[S3]: https://learn.microsoft.com/en-us/windows/apps/publish/store-policies-change-history
[S4]: https://learn.microsoft.com/en-us/windows/apps/publish/partner-center/open-a-developer-account
[S5]: https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/reserve-your-apps-name
[S6]: https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/app-package-requirements
[S7]: https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-manual-conversion
[S8]: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/app-capability-declarations
[S9]: https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/desktop-to-uwp-extensions#associate-your-packaged-application-with-a-set-of-file-types
[S10]: https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-behind-the-scenes
[S11]: https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msi/app-package-requirements
[S12]: https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msi/upload-app-packages
[S13]: https://learn.microsoft.com/en-us/windows/msix/package/signing-package-overview
[S14]: https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/publish-update-to-your-app-on-store
[S15]: https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/product-declarations
[S16]: https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/app-certification-process
[S17]: https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessibility-testing
[S18]: https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/support-info

- [S1：现行 Store Policies 7.19][S1]；[S2：待生效 7.20][S2]；[S3：政策变更历史][S3]。
- [S4：开发者账户与既有账户说明][S4]；[S5：新建产品、名称预留][S5]。
- [S6：MSIX 提交、身份、签名要求][S6]；[S7：手工封装原生桌面应用][S7]；[S8：能力声明与审核][S8]。
- [S9：文件类型集成][S9]；[S10：包内运行、文件系统、注册表、卸载][S10]。
- [S11：EXE/MSI 要求][S11]；[S12：外部安装器 URL、参数、返回码][S12]；[S13：MSIX 签名和公共信任服务地域资格][S13]。
- [S14：MSIX 更新][S14]；[S15：可访问性及第三方支付声明][S15]；[S16：认证流程][S16]；[S17：可访问性测试][S17]；[S18：隐私与支持资料][S18]。

## 8. 本次代码与发行包核查

本节基于本地源码和 GitHub Releases 元数据，不是推测的产品功能。核查前工作区干净，HEAD 为 `2267ca0`；功能发布标签 v0.1.10 对应源码提交 `120c0d7`。本次只增加本文，没有更改程序、用户设置、文件关联或已有发行包。

### 已具备的条件

- `Cargo.toml:1` / `.cargo/config.toml:1`：Rust 2024、Win32 系统 API、静态 MSVC CRT；不依赖 Electron/WebView。`assets/app.manifest:6` 为 `asInvoker`，不要求管理员权限，适合先评估完整信任桌面 MSIX 封装，而非重写 UI。
- `src/main.rs:15`：接受第一个文件路径参数，已有本地文件打开入口。可为包清单设计明确的单文件参数传递；仍必须在实际安装包中验证 Shell 激活。
- `src/association.rs:11`：现有格式为 `.md`、`.markdown`、`.mdown`、`.mkd`。源码明确不写 UserChoice 或扩展名默认值，用户同意边界应保留。
- `README.md:43`、`src/updater.rs:44`、`src/ui/pictures/web.rs:43`：默认不联网，更新查询和逐张确认的网络图片是两个独立入口。没有发现必须依赖后台账户、云服务或管理员服务才能阅读本地 Markdown 的设计。
- `LICENSE`、`THIRD_PARTY_NOTICES.md`：已有项目许可证与第三方声明；商店包应继续正确携带，不包含开发测试替身或本机配置。

GitHub `gh release view --repo Lianzy-Baimiao/FeatherMD` 本次返回：v0.1.10，非草稿、非预发布，2026-10-16 发布。`dist/release-0.1.10/FeatherMD.exe` 实测为 **1,098,752 字节**、ProductVersion **0.1.10**，`Get-AuthenticodeSignature` 为 **NotSigned**；SHA-256 为 `c594dceb6bd76b66c6060b0acff1afdb5b024172d82123ed17b46d8bdb86b86b`，与本次 GitHub asset digest 相符。没有下载或替换现有发行附件。

### 提交前具体工作

| 工作 | 当前证据 | 建议与验收 |
| --- | --- | --- |
| 增加 MSIX 分发链路 | `scripts/package.ps1:1` 仅打包裸 EXE/ZIP；未找到 AppxManifest、MSIX 构建、Store 清单或商店图标资源集 | 增加单独的商店包目标，使用 FeatherMD 自己的产品身份、合规四段包版本及图标资源。现有 `assets/app.manifest` 是 Win32 清单，不是 AppxManifest。保留便携包发布脚本。 |
| 分离文件关联行为 | `src/association.rs:135` 手动写自身 HKCU ProgID/Capabilities；`src/ui.rs:2003` 先注册 EXE 绝对路径再打开默认应用设置 | MSIX 使用清单提供四种文件类型候选；商店环境不执行便携注册/反注册。实测安装、升级后双击文档与默认应用入口，不能留下版本化 WindowsApps 路径或干扰并存绿色版。 |
| 设置落点与共存 | `src/settings.rs:249` 优先读取 EXE 旁已有的 `FeatherMD.ini`，否则使用 `%APPDATA%\FeatherMD\settings.ini`；`:336` 用临时文件和替换保存 | 包内不携带便携 INI；明确是否与绿色版隔离。验证包身份下设置路径、首次写入、重启、升级保留与卸载行为，不能直接假定虚拟化已解决。 |
| 商店更新入口 | `src/update.rs:8` 固定 GitHub latest API；`src/ui/preferences.rs:475` 存在“前往下载页”按钮 | 商店版使用 Store 更新/商店页面入口，绿色版保留当前 GitHub 查询。当前代码没有自动下载或安装更新，不能把它误报为已经违反更新政策。 |
| 隐私政策与说明一致性 | 未找到独立隐私政策文档/URL；`src/ui/preferences.rs:502` 仍显示“关闭自动检查且不点‘立即检查’时，FeatherMD 不会联网。” | 提供完整公开隐私政策并关联到提交资料；修正该句及四语版本，使之明确只描述更新请求。当前手动确认网络图片也会联网，`README.md:122` 已正确披露这一点。需一并评估 HTTP 图片 URL 的条件风险，见第 5 节。 |
| 真正的包内验收 | `.github/workflows/ci.yml:1` 为编译、单元测试与裸 EXE 原生冒烟，没有 MSIX 安装/升级/卸载及 WACK 流程 | 对最终 MSIX 做标准用户环境生命周期、真实文件关联、配置、外部编辑器、离线/联网确认及 WACK；按实际验证范围选择最低 Windows 版本。不能把 README 的 Windows 10 1607+ 直接照搬为已经验证的 MSIX 支持范围。 |
| 无障碍与真实商店资料 | `docs/PROGRESS.md:242`、`:301` 记录正文/目录无障碍仍待完善；已有四语 UI 和截图，但不是完整读屏验收 | 检查键盘、Narrator、自绘控件与正文语义；修复主要可用性问题。使用新版真实截图和准确的“分段 Markdown 阅读器”说明，不宣称完整 CommonMark/GFM 或所有平台已测。 |

`docs/PROGRESS.md` 中的 **104 项测试、19 项原生回归通过**是 2026-10-16 已发布版本的历史证据；本次没有重新构建或运行它们，也没有执行 MSIX/WACK。About 的可选赞赏入口 `src/ui/about.rs:6` 不是单凭外链即可判定的上架障碍，仍需按第 5 节核对真实付款性质与申报。

**实施建议：保留一个产品和两条分发渠道。先完成包身份/关联/配置/更新的最小商店适配、隐私材料和包内回归，再准备提交；不为商店重写核心阅读器，也不把未签名绿色 EXE 冒充已受 Store 签名的版本。**
