//! Code colouring: tokens per language, state across lines and UTF-8 safety.
use feathermd::highlight::{self, State, Token};

/// `(text, token)` pieces of one line.
fn pieces(language: &str, state: &mut State, text: &str) -> Vec<(String, Option<Token>)> {
    let syntax = highlight::find(language).expect(language);
    let out = highlight::line(syntax, state, text);
    // Pieces cover the line exactly, in order.
    let mut at = 0;
    for &(a, b, _) in &out {
        assert_eq!(a, at, "{text:?}");
        assert!(b > a);
        at = b;
    }
    assert_eq!(at, text.len(), "{text:?}");
    out.into_iter()
        .map(|(a, b, t)| (text[a..b].to_owned(), t))
        .collect()
}
fn token_of(p: &[(String, Option<Token>)], word: &str) -> Option<Token> {
    p.iter()
        .find(|(t, _)| t.trim() == word)
        .unwrap_or_else(|| panic!("{word} not a piece of {p:?}"))
        .1
}

#[test]
fn languages_are_found_by_common_fence_names() {
    for name in [
        "rust",
        "RS",
        "c++",
        "C#",
        "python",
        "py",
        "js",
        "TypeScript",
        "tsx",
        "go",
        "sql",
        "bash",
        "sh",
        "PowerShell",
        "bat",
        "json",
        "yaml",
        "toml",
        "html",
        "xml",
        "css",
        "diff",
        "{.python}",
        "rust,ignore",
        "language-java",
        "kotlin",
        "swift",
        "lua",
        "php",
        "ruby",
        "dockerfile",
    ] {
        assert!(highlight::find(name).is_some(), "{name}");
    }
    for name in [
        "",
        "text",
        "plaintext",
        "mermaid",
        "unknown-language",
        &"x".repeat(40),
    ] {
        assert!(highlight::find(name).is_none(), "{name}");
    }
}

#[test]
fn rust_keywords_strings_numbers_comments_and_macros() {
    let mut s = State::default();
    let p = pieces(
        "rust",
        &mut s,
        "pub fn main() -> Result<u8, E> { let x = 0x1f; println!(\"hi {x}\"); } // done",
    );
    assert_eq!(token_of(&p, "pub"), Some(Token::Keyword));
    assert_eq!(token_of(&p, "fn"), Some(Token::Keyword));
    assert_eq!(token_of(&p, "main"), Some(Token::Function));
    assert_eq!(token_of(&p, "Result"), Some(Token::Type));
    assert_eq!(token_of(&p, "u8"), Some(Token::Type));
    assert_eq!(token_of(&p, "0x1f"), Some(Token::Number));
    assert_eq!(token_of(&p, "println"), Some(Token::Function));
    assert_eq!(token_of(&p, "\"hi {x}\""), Some(Token::String));
    assert_eq!(token_of(&p, "// done"), Some(Token::Comment));
    assert_eq!(s, State::Normal);
    // Lifetimes are not characters; characters and attributes are recognised.
    let p = pieces(
        "rust",
        &mut s,
        "#[derive(Debug)] fn f<'a>(c: char) { '\\n'; 'x'; }",
    );
    assert_eq!(token_of(&p, "#[derive(Debug)]"), Some(Token::Meta));
    assert_eq!(token_of(&p, "'a"), Some(Token::Meta));
    assert_eq!(token_of(&p, "'\\n'"), Some(Token::String));
    assert_eq!(token_of(&p, "'x'"), Some(Token::String));
}

#[test]
fn block_comments_and_strings_continue_on_the_next_line() {
    let mut s = State::default();
    let p = pieces("c", &mut s, "int x; /* starts");
    assert_eq!(token_of(&p, "/* starts"), Some(Token::Comment));
    assert_eq!(s, State::Comment);
    let p = pieces("c", &mut s, "still */ return 1;");
    assert_eq!(token_of(&p, "still */"), Some(Token::Comment));
    assert_eq!(token_of(&p, "return"), Some(Token::Keyword));
    assert_eq!(s, State::Normal);

    let mut s = State::default();
    pieces("python", &mut s, "doc = \"\"\"first");
    assert!(matches!(s, State::Text { .. }));
    let p = pieces("python", &mut s, "second\"\"\" if True else None");
    assert_eq!(token_of(&p, "second\"\"\""), Some(Token::String));
    assert_eq!(token_of(&p, "True"), Some(Token::Number));
    assert_eq!(token_of(&p, "None"), Some(Token::Number));

    // JavaScript template literals span lines; ordinary quotes do not.
    let mut s = State::default();
    pieces("js", &mut s, "const t = `a ${b}");
    assert!(matches!(s, State::Text { quote: b'`', .. }));
    pieces("js", &mut s, "c`;");
    assert_eq!(s, State::Normal);
    let mut s = State::default();
    pieces("js", &mut s, "const u = 'unterminated");
    assert_eq!(s, State::Normal);

    // Lua block comments are not line comments.
    let mut s = State::default();
    pieces("lua", &mut s, "--[[ open");
    assert_eq!(s, State::Comment);
}

#[test]
fn other_languages() {
    let mut s = State::default();
    let p = pieces("sql", &mut s, "SELECT name FROM users WHERE id = 42 -- one");
    assert_eq!(token_of(&p, "SELECT"), Some(Token::Keyword));
    assert_eq!(token_of(&p, "42"), Some(Token::Number));
    assert_eq!(token_of(&p, "-- one"), Some(Token::Comment));

    let p = pieces(
        "bash",
        &mut s,
        "if [ -n \"$HOME\" ]; then echo $USER; fi # note",
    );
    assert_eq!(token_of(&p, "if"), Some(Token::Keyword));
    assert_eq!(token_of(&p, "$USER"), Some(Token::Meta));
    assert_eq!(token_of(&p, "# note"), Some(Token::Comment));
    // `$#` and a `#` inside a word are not comments.
    let p = pieces("bash", &mut s, "echo $# a#b");
    assert!(p.iter().all(|(_, t)| *t != Some(Token::Comment)), "{p:?}");

    let p = pieces(
        "json",
        &mut s,
        "{\"name\": \"FeatherMD\", \"ok\": true, \"n\": 1.5e3}",
    );
    assert_eq!(token_of(&p, "\"name\""), Some(Token::Type));
    assert_eq!(token_of(&p, "\"FeatherMD\""), Some(Token::String));
    assert_eq!(token_of(&p, "true"), Some(Token::Number));
    assert_eq!(token_of(&p, "1.5e3"), Some(Token::Number));

    let p = pieces("c", &mut s, "#include <stdio.h>");
    assert_eq!(token_of(&p, "#include"), Some(Token::Meta));

    let p = pieces(
        "css",
        &mut s,
        "a { background: url(https://x.org/a.png) #fff; }",
    );
    assert!(p.iter().all(|(_, t)| *t != Some(Token::Comment)), "{p:?}");

    let p = pieces("toml", &mut s, "[package]");
    assert_eq!(token_of(&p, "[package]"), Some(Token::Keyword));
    let p = pieces("toml", &mut s, "name = \"feathermd\" # comment");
    assert_eq!(token_of(&p, "name"), Some(Token::Type));
    assert_eq!(token_of(&p, "# comment"), Some(Token::Comment));

    let p = pieces("diff", &mut s, "+added line");
    assert_eq!(p[0].1, Some(Token::Inserted));
    let p = pieces("diff", &mut s, "-removed");
    assert_eq!(p[0].1, Some(Token::Deleted));
}

#[test]
fn markup_tags_attributes_comments_and_entities() {
    let mut s = State::default();
    let p = pieces(
        "html",
        &mut s,
        "<a href=\"x\" class=big>&amp; text</a> <!-- note",
    );
    assert_eq!(token_of(&p, "a"), Some(Token::Keyword));
    assert_eq!(token_of(&p, "href"), Some(Token::Type));
    assert_eq!(token_of(&p, "\"x\""), Some(Token::String));
    assert_eq!(token_of(&p, "&amp;"), Some(Token::Number));
    assert_eq!(s, State::Comment);
    pieces("html", &mut s, "end -->");
    assert_eq!(s, State::Normal);
    // A tag split over lines keeps colouring its attributes.
    pieces("xml", &mut s, "<item id=\"1\"");
    assert_eq!(s, State::Tag);
    let p = pieces("xml", &mut s, "  name=\"two\">");
    assert_eq!(token_of(&p, "name"), Some(Token::Type));
    assert_eq!(s, State::Normal);
}

#[test]
fn unicode_text_never_splits_a_character() {
    for language in ["rust", "python", "html", "bash", "sql", "css", "yaml"] {
        let mut s = State::default();
        for text in [
            "let 名字 = \"中文\"; // 注释 😀",
            "日本語のテキスト 'クォート'",
            "<p title=\"标题\">内容&nbsp;</p>",
            "'未闭合",
            "\u{fffd}\u{200d}👩‍💻",
        ] {
            // `pieces` slices every range, which panics off a char boundary.
            pieces(language, &mut s, text);
        }
    }
}
