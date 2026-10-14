<h2 align="center">排版样张 · FeatherMD</h2>

<p align="center">HTML 标签只改变排版：<b>粗体</b>、<mark>高亮</mark>、<kbd>Ctrl</kbd>+<kbd>F</kbd>，脚本不会执行。</p>

## 阅读节奏

正文行高约 1.6 倍，段落之间留一点呼吸；长段落按窗口宽度换行，逗号、句号不会落在行首。行内代码 `cargo build --release` 有圆角底色，[链接](https://github.com/Lianzy-Baimiao/FeatherMD)使用皮肤的强调色。

> 文件可以很大，阅读器应该很轻。

```rust
/// 只读取附近的一页，不必等整份文件。
fn first_page(path: &str) -> std::io::Result<Page> {
    let document = Document::open(path)?;
    document.page(0) // 128 KiB
}
```

| 皮肤 | 气质 | 适合 |
| --- | :---: | ---: |
| 素白 | 干净 | 白天 |
| 报纸 | 沉稳 | 长文 |
| 深海 | 安静 | 夜读 |

- [x] 二十款皮肤与代码高亮
- [ ] 下一页继续读
