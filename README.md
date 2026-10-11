# FeatherMD · 羽读

**轻一点，读更多。** 一个用 Rust 编写的 Windows 原生 Markdown 查看器。

[下载单文件 EXE](https://github.com/Lianzy-Baimiao/FeatherMD/releases/latest) · [性能与验证](docs/PERFORMANCE.md) · [实现说明](docs/ARCHITECTURE.md)

![FeatherMD 浅色阅读界面](docs/reading-light.png)

## 为什么做这个

打开一份 Markdown，不应该先启动一个浏览器。

FeatherMD 使用 Win32 + GDI，没有 Electron、WebView、JavaScript、遥测或自动联网。v0.1.0 的 Windows x64 EXE 约 **640 KiB**，无需安装 .NET、VC++ 运行库或其他第三方运行时。

**大文件不是先全部载入再显示：** 每次只读取附近 128 KiB，后台读取与搜索，前台只绘制可见行。文件总长度不决定首屏需要分配的内存。

## 使用

1. 从 Releases 下载 **FeatherMD.exe**，放到喜欢的文件夹，双击运行。
2. 拖入 Markdown，或按 **Ctrl+O**。
3. 通过“设置”调整主题、字号、自动换行、阅读宽度或源码模式。

支持 Windows 10（1607+）/ Windows 11 **x64**。当前实测环境为 Windows 11 x64；其他版本尚未逐个实机验证。无安装器，无管理员权限要求。尚未购买代码签名证书，Windows SmartScreen 可能提醒未知发布者；请核对来源和 SHA-256，不要关闭系统防护。

### 设置为默认 Markdown 查看器

- 先将 EXE 放在**固定位置**。
- 点击 **设置 → 注册为 Markdown 查看器**，确认后会打开 Windows“默认应用”。
- 搜索 **.md**，选择 **FeatherMD**；也可以右键文件 → 打开方式 → 选择其他应用 → FeatherMD → 始终使用。
- 支持注册 .md / .markdown / .mdown / .mkd。
- “移除打开方式注册”只移除 FeatherMD 自己的注册项。

**不会绕过 Windows 保护、覆盖 UserChoice 或偷偷改变你现有的默认程序。** 移动 EXE 后需要重新注册。

### 快捷键

| 操作 | 快捷键 |
| --- | --- |
| 打开文件 | Ctrl+O / 拖放 |
| 后台查找（区分大小写的原文搜索） | Ctrl+F |
| 下一个匹配（到末尾会回绕） | Enter / F3 |
| 取消搜索 / 关闭搜索栏 | Esc |
| 按字节百分比跳转 | Ctrl+G |
| 文件开头 / 结尾 | Home / End |
| 滚动 | 滚轮 / ↑↓ / PageUp / PageDown / 空格 |
| 放大 / 缩小 | Ctrl+滚轮 / Ctrl++ / Ctrl+- |
| 源码 / 预览切换 | Ctrl+R |
| 重新载入 | F5 |
| 选择当前缓存分页 | Ctrl+A |
| 复制选中行；未选择时复制可见内容 | Ctrl+C |
| 恢复默认阅读设置 | Ctrl+0 |
| 帮助 | F1 |

鼠标支持**整行拖选**。关闭换行后可使用底部滚动条或左右方向键。

### 设置文件

默认写入 %APPDATA%\FeatherMD\settings.ini，每次修改自动保存。文档本身始终只读。

需要可携带设置时，在 EXE 旁创建一个空的 **FeatherMD.ini**，以后设置随这个文件保存。只想单文件运行则不需要它。最近文件和文档正文不会写入设置。

## 阅读支持与明确边界

- 支持 UTF-8 / UTF-8 BOM、中英文、系统字体可显示的 Unicode。
- 标题、粗体、斜体、删除线、行内代码、代码围栏、引用、列表、任务清单、分隔线、网页链接。
- 表格当前保留为等宽文本；图片显示替代文本，不解码图片、不访问网络。
- 原始 HTML 显示为文字，不执行。仅 http/https 链接可经确认后打开。
- **这是低内存分段预览，不是完整 CommonMark/GFM 浏览器渲染器。** 物理行分别排版；跨行强调、Setext 标题、复杂嵌套、全局引用链接等可能不按完整规范呈现。
- 非起始分页不扫描整份文件来恢复状态，跨页代码围栏/列表上下文可能不完整，状态栏会提示。需要精确原文时切换源码模式。
- 超长单行拆为最多 4096 字节的安全 Unicode 片段，防止一个段落阻塞 UI。
- UTF-16/UTF-32 BOM 文档明确提示先转换成 UTF-8；其他无效 UTF-8 字节显示替代字符并提示。不会自行改写文件。
- 全文搜索仍需顺序读取数据，速度取决于设备；搜索可取消。网络盘、机械硬盘、文件锁或系统负载也会影响速度。
- 自绘正文尚不具备完整屏幕阅读器语义；鼠标选择目前按行而非按字符。

**不承诺“任意大小瞬间完整渲染”。** 目标是有界读取、快速首屏和可响应的交互。

## 实测摘要

本机 Windows 11 x64，Release 构建、暖缓存、1 GiB 实体 UTF-8 Markdown：

- 后台读取 + Markdown 解析首分页，中位数 **1.05 ms**（30 次；不含 GDI 排版）。
- 隐藏真实窗口测试，启动到首分页排版约 **75 ms**（单次，非冷启动保证）。
- 测试结束私有内存约 **7.1 MiB**，工作集约 **26.4 MiB**（含系统 GUI DLL）。
- 100 GiB **稀疏文件**通过首尾/随机读取验证；这不等于扫描了 100 GiB 实际 Markdown 内容。

完整方法、数字和限制见 [PERFORMANCE.md](docs/PERFORMANCE.md)。

## 从源码构建

安装 Rust stable 和 Visual Studio Build Tools 的“使用 C++ 的桌面开发”组件 / Windows SDK，然后在 x64 Developer PowerShell 中：

~~~powershell
cargo +stable-x86_64-pc-windows-msvc test
cargo +stable-x86_64-pc-windows-msvc clippy --all-targets -- -D warnings
cargo +stable-x86_64-pc-windows-msvc build --release
~~~

产物：target/release/feathermd.exe。MSVC 静态 CRT 配置位于 .cargo/config.toml。项目源路径含中文也可构建；不推荐缺少 windres 的 GNU 工具链。

~~~powershell
# 无干扰隐藏窗口测试：主题、真实输入搜索、百分比跳转、GDI 资源、退出
./scripts/ui-smoke.ps1

# 生成 1 MiB / 64 MiB / 1 GiB 实体测试文件（约占 1.1 GiB）
./scripts/make-fixtures.ps1
cargo +stable-x86_64-pc-windows-msvc run --release --example bench -- test-artifacts/fixtures/1073741824.md 30
~~~

核心读取、解析与设置逻辑可在 Linux 上测试，GUI 仅支持 Windows。测试会创建并删除一个 100 GiB 稀疏文件，实际占用很小，需要支持稀疏文件的文件系统。

## License

[MIT](LICENSE)。依赖许可证见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)，也内置在 EXE 中，可通过“帮助 → 导出内置许可证”保存。
