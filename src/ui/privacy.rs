//! Offline privacy notice: viewing it does not make a network request.
use super::*;

pub(super) unsafe fn show(owner: HWND) {
    let mut paragraphs = vec![
        tr("FeatherMD 隐私说明 · 2026-10-17"),
        tr("文档在本机读取，不上传文档或设置；没有账户、广告、遥测或崩溃上报。设置仅保存在本机。"),
        tr(
            "网络图片仅在你逐张确认后请求。网站会收到 IP、图片地址（含查询参数）及应用版本；HTTP 地址不加密。",
        ),
        tr(
            "图片不发送 Cookie 或登录信息，仅缓存在内存中。F5、重新打开文档或退出会清除缓存与授权。",
        ),
    ];
    paragraphs.push(if distribution::store() {
        tr("商店版由 Microsoft Store 管理安装与更新，不检查 GitHub。商店的数据处理适用 Microsoft 隐私政策。")
    } else {
        tr("绿色版检查更新默认关闭；手动或启用后访问 GitHub，发送 IP 和应用版本，不自动下载安装。")
    });
    paragraphs.extend([
        tr("外部链接经确认后交给浏览器。赞赏完全自愿，不解锁功能；第三方网站和支付服务适用各自的隐私政策。"),
        tr("反馈请到项目 Issues；这是公开页面，请勿提交私人文档或凭据。完整说明随包提供于 docs/PRIVACY.md。"),
        "https://github.com/Lianzy-Baimiao/FeatherMD/issues",
    ]);
    unsafe {
        MessageBoxW(
            owner,
            w(&paragraphs.join("\n\n")).as_ptr(),
            w(tr("隐私说明…")).as_ptr(),
            MB_OK,
        );
    }
}
