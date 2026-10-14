//! HTML subset and Markdown structure: what each line becomes. Nothing here runs
//! scripts or loads resources; tags only change the shape of the text.
use feathermd::{
    document::SourceLine,
    html,
    markdown::{self, Block, Kind},
    table::Align,
};

fn lines(texts: &[&str]) -> Vec<SourceLine> {
    let mut at = 0;
    texts
        .iter()
        .map(|text| {
            let end = at + text.len() as u64 + 1;
            let line = SourceLine {
                start: at,
                end,
                text: (*text).into(),
                continued: false,
            };
            at = end;
            line
        })
        .collect()
}
fn parse(texts: &[&str]) -> Vec<Block> {
    markdown::blocks(&lines(texts), false)
}
fn text(block: &Block) -> String {
    block.spans.iter().map(|s| s.text.as_str()).collect()
}

#[test]
fn inline_tags_style_text_and_hide_what_must_not_show() {
    let spans = markdown::inline(
        "a <b>bold</b> <i>it</i> <u>under</u> <kbd>Ctrl</kbd> <mark>hi</mark> x<sup>2</sup> <code>c</code>",
    );
    let find = |t: &str| {
        spans
            .iter()
            .find(|s| s.text == t)
            .unwrap_or_else(|| panic!("{t}: {spans:?}"))
    };
    assert!(find("bold").bold);
    assert!(find("it").italic);
    assert!(find("under").underline);
    assert!(find("Ctrl").kbd);
    assert!(find("hi").mark);
    assert_eq!(find("2").script, 1);
    assert!(find("c").code);
    // Script and style content never shows; comments vanish; entities decode.
    let spans =
        markdown::inline("x<script>alert(1)</script>y<!-- hidden -->z &amp; <style>p{}</style>");
    let shown: String = spans.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(shown, "xyz & ");
    // Unknown tags such as generics stay text.
    let shown: String = markdown::inline("Vec<T> and a < b > c")
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert_eq!(shown, "Vec<T> and a < b > c");
    // Line breaks, links and images.
    let spans = markdown::inline(
        "one<br>two <a href=\"https://example.com\">site</a> <img src=\"a.png\" alt=\"Logo\" width=\"64\">",
    );
    assert!(spans.iter().any(|s| s.line_break));
    assert_eq!(
        spans
            .iter()
            .find(|s| s.text == "site")
            .and_then(|s| s.link.as_deref()),
        Some("https://example.com")
    );
    let image = spans.iter().find(|s| s.image.is_some()).unwrap();
    assert_eq!(image.text, "Logo");
    let picture = image.image.as_ref().unwrap();
    assert_eq!(
        (picture.src.as_str(), picture.width, picture.height),
        ("a.png", Some(64), None)
    );
    // Markdown images keep their alternative text and source.
    let spans = markdown::inline("![Diagram **one**](docs/a.png)");
    let image = spans.iter().find(|s| s.image.is_some()).unwrap();
    assert_eq!(image.text, "Diagram one");
    assert_eq!(image.image.as_ref().unwrap().src, "docs/a.png");
}

#[test]
fn html_headings_paragraphs_and_alignment() {
    let b = parse(&[
        "<h1 align=\"center\">FeatherMD</h1>",
        "<p align=\"center\">",
        "  <img src=\"logo.png\" alt=\"logo\">",
        "</p>",
        "",
        "Plain **markdown** again",
    ]);
    assert_eq!(b[0].kind, Kind::Heading(1));
    assert_eq!(b[0].align, Align::Center);
    assert_eq!(text(&b[0]), "FeatherMD");
    assert_eq!(b[1].kind, Kind::Spacer);
    assert_eq!(b[2].kind, Kind::Paragraph);
    assert_eq!(b[2].align, Align::Center);
    assert!(b[2].spans[0].image.is_some());
    assert_eq!(b[3].kind, Kind::Spacer);
    assert_eq!(b[5].kind, Kind::Paragraph);
    assert_eq!(b[5].align, Align::Left);
    assert!(b[5].spans.iter().any(|s| s.bold && s.text == "markdown"));

    // Markdown inside an open container takes its alignment until it closes.
    let b = parse(&[
        "<div align=\"right\">",
        "",
        "Signed",
        "",
        "</div>",
        "",
        "after",
    ]);
    assert_eq!(b[2].kind, Kind::Paragraph);
    assert_eq!(b[2].align, Align::Right);
    assert_eq!(b[6].align, Align::Left);
    // <center> and inline text-align styles, as many READMEs write them.
    let b = parse(&[
        "<center>Title card</center>",
        "",
        "<p style=\"color: red; text-align: right\">Right</p>",
        "",
        "<div style=\"text-align:center\">",
        "",
        "**Centred** markdown",
        "",
        "</div>",
    ]);
    assert_eq!(
        (text(&b[0]).as_str(), b[0].align),
        ("Title card", Align::Center)
    );
    assert_eq!((text(&b[2]).as_str(), b[2].align), ("Right", Align::Right));
    assert_eq!(b[6].kind, Kind::Paragraph);
    assert_eq!(b[6].align, Align::Center);
    // <hr> and <br>.
    let b = parse(&["<hr>", "", "<p>one<br/>two</p>"]);
    assert_eq!(b[0].kind, Kind::Rule);
    assert!(b[2].spans.iter().any(|s| s.line_break));
}

#[test]
fn html_lists_quotes_details_and_preformatted_text() {
    let b = parse(&[
        "<ol start=\"3\">",
        "  <li>three</li>",
        "  <li>",
        "    four",
        "  </li>",
        "</ol>",
    ]);
    let shown: Vec<String> = b
        .iter()
        .filter(|b| b.kind != Kind::Spacer)
        .map(text)
        .collect();
    assert_eq!(shown, ["3. three", "4. four"]);
    assert!(
        b.iter()
            .filter(|b| b.kind != Kind::Spacer)
            .all(|b| b.marker && b.indent > 0)
    );

    let b = parse(&["<blockquote>", "Quoted words", "</blockquote>"]);
    assert_eq!(b[1].kind, Kind::Quote);

    let b = parse(&[
        "<details>",
        "<summary>More</summary>",
        "Hidden until opened",
        "</details>",
    ]);
    assert_eq!(text(&b[1]), "▾ More");
    assert!(b[1].spans.iter().all(|s| s.bold));
    assert!(b[2].indent > 0);

    // Preformatted text keeps its spaces across blank lines and shows tags' text.
    let b = parse(&[
        "<pre>",
        "  a  b",
        "",
        "<b>c</b> &lt;d&gt;",
        "</pre>",
        "after",
    ]);
    assert_eq!(b[1].kind, Kind::Code);
    assert_eq!(text(&b[1]), "  a  b");
    assert_eq!(text(&b[3]), "c <d>");
    assert_eq!(b[5].kind, Kind::Paragraph);
}

#[test]
fn scripts_comments_and_embeds_stay_hidden_across_lines() {
    let b = parse(&[
        "<script>",
        "document.write('<h1>x</h1>')",
        "",
        "alert(1)",
        "</script>",
        "<!--",
        "a comment",
        "-->",
        "<iframe src=\"https://example.com\"></iframe>",
        "visible",
    ]);
    for block in &b[..9] {
        assert_eq!(block.kind, Kind::Spacer, "{:?}", text(block));
        assert!(block.spans.is_empty());
    }
    assert_eq!(text(&b[9]), "visible");
    // HTML inside fenced code is code.
    let b = parse(&["```html", "<h1>title</h1>", "```"]);
    assert_eq!(b[1].kind, Kind::Code);
    assert_eq!(text(&b[1]), "<h1>title</h1>");
}

#[test]
fn markdown_lists_nest_tasks_check_and_fences_colour() {
    let b = parse(&[
        "- one",
        "  - two",
        "    - three",
        "- [ ] task",
        "- [x] done",
        "1. first",
        "    continued text",
    ]);
    assert_eq!(text(&b[0]), "• one");
    assert_eq!(text(&b[1]), "◦ two");
    assert_eq!(text(&b[2]), "▪ three");
    assert!(b[0].indent < b[1].indent && b[1].indent < b[2].indent);
    assert!(b[..3].iter().all(|b| b.marker));
    assert_eq!(b[3].spans[0].check, Some(false));
    assert_eq!(b[4].spans[0].check, Some(true));
    assert_eq!(b[5].spans[0].text, "1. ");
    // Indented text inside a list is not code.
    assert_eq!(b[6].kind, Kind::Paragraph);

    let b = parse(&["```rust", "fn main() {}", "```"]);
    assert_eq!(b[0].kind, Kind::Fence);
    assert_eq!(text(&b[0]), "rust");
    assert!(b[1].spans.iter().any(|s| s.token.is_some()));
    // A fence inside a list item.
    let b = parse(&["- item", "  ```", "  code", "  ```", "after"]);
    assert_eq!(b[1].kind, Kind::Fence);
    assert_eq!(b[2].kind, Kind::Code);
    assert_eq!(text(&b[2]), "code");
    assert_eq!(b[3].kind, Kind::Fence);
    // Quoted lists and headings.
    let b = parse(&["> - quoted item", "> ## Quoted heading"]);
    assert_eq!(b[0].kind, Kind::Quote);
    assert_eq!(text(&b[0]), "• quoted item");
    assert_eq!(b[1].kind, Kind::Heading(2));
}

#[test]
fn outline_reads_single_line_html_headings() {
    assert_eq!(
        html::heading("<h2 align=\"center\">Hello <b>world</b> &amp; more</h2>"),
        Some((2, "Hello world & more".into()))
    );
    assert_eq!(html::heading("<p>not a heading</p>"), None);
    assert_eq!(html::heading("<h1></h1>"), None);
}

#[test]
fn hostile_markup_stays_bounded() {
    let deep = "<div>".repeat(5000) + "x" + &"</div>".repeat(5000);
    let b = parse(&[&deep]);
    assert_eq!(b.len(), 1);
    let long = format!("<a href=\"{}\">x</a>", "y".repeat(100_000));
    let _ = markdown::inline(&long);
    let tag = format!("<p {}>", "a=1 ".repeat(10_000));
    let _ = parse(&[&tag, "text"]);
    let _ = html::decode(&"&#xFFFFFFFF; &#0; &#55296; &bogus; &".repeat(100));
}
