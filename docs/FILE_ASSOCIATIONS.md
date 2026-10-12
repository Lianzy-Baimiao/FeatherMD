# Windows 文件关联

作者：lianzy
最后修改人：lianzy
日期：2026-10-12

## 产品行为

“更多 → 文件关联”提供三个独立操作：

- 加入 Markdown 右键打开方式：注册当前 EXE 路径，成功后在状态栏提示；不弹出系统设置。
- 设为 .md 默认应用（系统确认）：先完成注册，再请求打开 FeatherMD 的 Windows 默认应用页面。用户在系统界面选择 .md 并确认。
- 移除打开方式注册：清理 FeatherMD 自有条目，不改其他应用的注册和用户默认选择。

注册针对当前用户，无需管理员权限。移动 EXE 后需重新注册。关闭系统设置不意味着已设为默认；程序不会根据 URI 启动成功显示“已设为默认”。

## 清单找不到应用时

便携 EXE 不会因下载或启动就自动出现在系统候选里，需要用户先执行注册。建议先放到固定目录，再从该副本的“更多 → 文件关联”重新注册；旧副本的注册路径不会自动迁移。注册会刷新 FeatherMD 自有的 ProgID、显示名称、图标、扩展名候选和默认应用能力，不改其他软件或用户选择。

1. 执行“设为 .md 默认应用”会先注册**正在运行的 EXE**，再打开 Windows 设置；无需先点另一遍加入打开方式。
2. 系统的“默认应用”首页可以按应用搜索 FeatherMD，也可以按文件类型搜索 `.md`。专属应用页里的 .md 行可能仍显示当前默认程序，这是正常的，不代表 FeatherMD 没有注册。
3. 如果旧设置页没有刷新，关闭后重新打开；仍找不到时，右键文件 → 打开方式 → 选择其他应用 → 浏览到固定目录的 EXE，由用户确认。
4. 开发者可运行 `scripts/association-inspect.ps1 -RequireFeatherMD` 只读检查 Windows Shell 的候选，其他扩展名通过 `-Extension .markdown` 等指定。此检查不注册、不变更默认项，也不证明所有 Windows 版本的设置 UI 都显示一致。

## Windows 支持边界

微软只为 Windows 8 及之后的 IApplicationAssociationRegistration 保留 QueryCurrentDefault 支持；旧 SetAppAsDefault 不再受支持。Windows 10 开始，SHOpenWithDialog 也不能用于更改默认程序，注册相关标志会被忽略。

本项目使用受支持的 `ms-settings:defaultapps?registeredAppUser=FeatherMD`。它对应 `HKCU\Software\RegisteredApplications` 中注册的名称。该精确跳转要求 Windows 11 21H2/22H2 的 2023 年 4 月累积更新，或 Windows 11 23H2 及后续版本。旧版可能仅打开默认应用首页；URI 返回成功不能验证页面位置或默认关联结果。若启动失败，程序尝试 `ms-settings:defaultapps`，再次失败则给出右键“始终使用”的操作指引。

普通桌面程序没有受支持的静默默认切换接口。企业设备可以通过受管理的默认关联策略部署，这是管理员部署流程，不适合便携阅读器的普通用户按钮。本项目不写受保护的 UserChoice、不计算其哈希，也不替换扩展名的默认值。

## 注册与测试

注册 `FeatherMD.Document` 的启动命令、图标和应用名称，添加 `.md`、`.markdown`、`.mdown`、`.mkd` 的 `OpenWithProgids`，并维护 `RegisteredApplications` 与 Capabilities。命令中的 EXE 和 `%1` 都带引号，支持中文和空格路径。

Windows 集成测试使用 HKCU 下专用唯一测试子树，调用相同的内部注册逻辑；不覆盖预定义根键、不发送关联更改通知、不写真实 Classes 或 Explorer 默认配置。测试不能代替各版本 Explorer 的实际列表显示和系统设置中的最终人工确认。

## 官方依据

- [IApplicationAssociationRegistration](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-iapplicationassociationregistration)
- [SHOpenWithDialog](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shopenwithdialog)
- [Launch the Default Apps settings page](https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-default-apps-settings)
- [How to Include an Application in the Open With Dialog Box](https://learn.microsoft.com/en-us/windows/win32/shell/how-to-include-an-application-on-the-open-with-dialog-box)
