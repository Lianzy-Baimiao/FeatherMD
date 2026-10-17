# FeatherMD Privacy Policy / 隐私说明

Effective date / 生效日期: 2026-10-17. Developer / 开发者: lianzy.
Applies to the Microsoft Store and GitHub portable editions of FeatherMD.
适用于 FeatherMD 的 Microsoft Store 版和 GitHub 绿色版。

## 中文

### 本机文件与设置

FeatherMD 是只读 Markdown 阅读器。应用在你的电脑上读取所选文档，不向开发者或其他服务器上传文档、设置或文件列表。应用没有账户、广告、分析遥测或自动崩溃上报。编辑命令只把当前文件路径交给你选择的外部编辑器；该编辑器的行为由其自身负责。

设置保存在本机 INI 文件中，可包含语言、外观、阅读字体、你选择的编辑器路径，以及绿色版的上次更新检查时间和版本号。绿色版优先使用 EXE 旁已有的 `FeatherMD.ini`，否则使用 `%APPDATA%\FeatherMD\settings.ini`。商店版使用该包的 `%LOCALAPPDATA%\Packages\<PackageFamilyName>\LocalState\settings.ini`，不共享绿色版设置。退出应用后可删除对应 INI 来重置设置。卸载时是否保留数据取决于 Windows 的卸载/保留选项。

### 图片

本机图片自动显示默认关闭。网络图片永远不自动加载；只有你点击图片占位文字并确认具体地址后，应用才向该地址发出请求。网站及网络服务提供者可获得你的 IP 地址、请求的完整图片路径与查询参数，以及包含 FeatherMD 版本号的 User-Agent。图片地址可能本身包含私人标识，请在确认前检查。HTTPS 使用系统证书验证；**HTTP 不加密**，网络上的其他方可能看到请求和响应。

图片请求不发送 Cookie、账户密码、身份验证信息或 Referer，不跟随重定向。图片只缓存在有界内存中，不由 FeatherMD 保存到磁盘；F5、重新打开文档或退出会清除远程图片缓存和该次授权。操作系统的分页、网络组件以及网站自身的日志不受应用控制。图片主机按其自身隐私政策处理请求数据。

### 更新

- 绿色版：自动检查默认关闭。手动检查或明确启用自动检查后，应用向 GitHub 的公开版本 API 请求版本信息；对方可收到 IP 地址和应用版本，不会收到文档或设置。自动检查最多每天一次。应用不会自动下载或安装更新。
- 商店版：不调用 GitHub 更新检查，也不下载绿色版替换自身。安装和更新由 Microsoft Store 管理；Microsoft 对商店、账户和诊断信息的处理适用其隐私声明。

### 外部网站、赞赏及反馈

应用在打开外部 HTTP(S) 链接前征求确认，再交给默认浏览器。浏览器可能使用自己的 Cookie 和已登录账户。GitHub、图片主机、Microsoft、爱发电及其支付服务均是外部服务，适用各自的政策。赞赏是可选的第三方支付，**不解锁 FeatherMD 功能，也不是使用条件**；FeatherMD 不处理支付凭据。

联系与隐私问题：[项目 Issues](https://github.com/Lianzy-Baimiao/FeatherMD/issues)。Issues 是公开页面，请勿上传私人文档、访问令牌或支付信息。开发者会看到你主动提交的反馈及 GitHub 公开账户信息；可按 GitHub 的功能编辑或删除自己的内容。若反馈涉及敏感信息，请先仅描述问题类别，不公开敏感数据。开发者没有通过应用采集的文档数据库可供访问、出售或删除。

本政策变化将随应用及项目文档更新，并标明日期；引入新的数据处理行为前会更新相应说明和所需同意。

## English

### Local documents and settings

FeatherMD is a read-only Markdown viewer. It reads selected documents on your device and does not upload documents, settings or file lists. It has no accounts, advertising, analytics telemetry or automatic crash reporting. The Edit command passes the current file path to your chosen editor, whose behavior is governed separately.

Local INI settings may contain language, appearance, font and selected editor path, plus the portable edition's last update-check time and version. Portable settings use an existing `FeatherMD.ini` beside the executable, otherwise `%APPDATA%\FeatherMD\settings.ini`. Store settings use the package's `%LOCALAPPDATA%\Packages\<PackageFamilyName>\LocalState\settings.ini`, separate from portable settings. Close the app and delete the corresponding INI to reset it. Data retention on uninstall depends on Windows uninstall/preservation options.

### Images

Automatic local images are off by default. Remote images never load automatically: you must click a placeholder and confirm its specific URL. The image host and network service providers can receive your IP address, full image path and query, and a User-Agent containing the FeatherMD version. URLs may contain private identifiers; inspect them before consenting. HTTPS uses system certificate validation. **HTTP is unencrypted**, and other parties on the network may observe requests and responses.

Image requests send no cookies, credentials, authentication or Referer, and do not follow redirects. Images are kept in bounded memory, not saved to disk by FeatherMD. F5, reopening a document or exiting clears remote-image cache and consent. OS paging, networking components and host logs are outside the app's control. Image hosts apply their own privacy policies.

### Updates

- Portable: automatic checks are off by default. Manual or enabled checks request public version information from GitHub. GitHub can receive your IP and app version, not documents or settings. Automatic checks occur at most daily. No automatic update download or installation.
- Store: no GitHub update checks or portable replacement downloads. Microsoft Store handles installation and updates under Microsoft's privacy statement, including its own account and diagnostic processing.

### External services, support and contact

External HTTP(S) links require confirmation and open in your default browser, which may use its own cookies or signed-in accounts. GitHub, image hosts, Microsoft, Afdian and payment providers operate under their own policies. Financial support is a voluntary third-party payment: it unlocks no FeatherMD features and is not required to use the app. FeatherMD does not process payment credentials.

Contact: [project Issues](https://github.com/Lianzy-Baimiao/FeatherMD/issues). This is public; do not post private documents, access tokens or payment details. The developer can see feedback you voluntarily submit and your public GitHub account information. Use GitHub's controls to edit/delete your content. For sensitive concerns, describe only the issue category first, without sensitive data. The developer holds no app-collected document database to access, sell or delete.

Changes to this policy will be dated and distributed with the app and project documentation. New data-processing behavior will be accompanied by updated notices and any required consent.

## Third-party policies / 第三方政策

- [Microsoft Privacy Statement](https://privacy.microsoft.com/privacystatement)
- [GitHub Privacy Statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement)
- Other image, browser and payment services: check the specific service you choose.
