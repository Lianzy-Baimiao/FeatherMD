use feathermd::{
    document::SourceLine,
    highlight::Token,
    markdown::Kind,
    plain::{self, Format, Level, Stamp},
};
use std::path::Path;

fn lines(text: &str) -> Vec<SourceLine> {
    let mut start = 0;
    text.split('\n')
        .map(|line| {
            let out = SourceLine {
                start,
                end: start + line.len() as u64 + 1,
                text: line.to_owned(),
                continued: false,
            };
            start = out.end;
            out
        })
        .collect()
}

#[test]
fn formats_follow_the_name_and_logs_are_recognised_by_their_lines() {
    for (name, format) in [
        ("notes.md", Format::Markdown),
        ("README", Format::Markdown),
        ("a.MARKDOWN", Format::Markdown),
        ("server.log", Format::Log),
        ("server.log.1", Format::Log),
        ("app.log-20261024", Format::Log),
        ("nohup.out", Format::Log),
        ("novel.txt", Format::Text),
        ("data.csv", Format::Text),
        ("LICENSE", Format::Text),
    ] {
        assert_eq!(plain::detect(Path::new(name)), format, "{name}");
    }
    let log = lines("2026-10-24 08:00:01 INFO a\n2026-10-24 08:00:02 INFO b\n\tat x\n[ERROR] c\nd");
    assert_eq!(plain::sniff(Format::Text, &log), Format::Log);
    let prose = lines("第一章\n他说 ERROR 只是一个词。\n今天 2026-10-24 08:00:00 下雨。\n天晴了。");
    assert_eq!(plain::sniff(Format::Text, &prose), Format::Text);
    // A Markdown name always stays Markdown.
    assert_eq!(plain::sniff(Format::Markdown, &log), Format::Markdown);
}

fn stamp(text: &str) -> plain::Parsed {
    plain::stamp(text).unwrap_or_else(|| panic!("no timestamp in {text:?}"))
}

#[test]
fn timestamps_of_common_log_formats_are_found_with_their_bytes() {
    let iso = stamp("2026-10-24T08:15:30.123+08:00 INFO x");
    assert_eq!(
        iso.stamp,
        Stamp {
            year: Some(2026),
            date: Some((10, 24)),
            time: (8, 15, 30)
        }
    );
    assert_eq!(iso.range, (0, 29));
    assert_eq!(
        stamp("[2026-10-24 08:15:30,123] [main] ERROR x").range,
        (1, 24)
    );
    assert_eq!(stamp("2026/10/24 08:15:30 x").stamp.date, Some((10, 24)));
    assert_eq!(stamp("24/10/24 08:15:30 INFO x").stamp.year, Some(2024));
    let logcat = stamp("10-24 08:15:30.123  1234  5678 E Tag: boom");
    assert_eq!(
        (logcat.stamp.year, logcat.stamp.date),
        (None, Some((10, 24)))
    );
    assert_eq!(logcat.letter, Some((Level::Error, (31, 32))));
    let syslog = stamp("Oct  4 08:15:30 host sshd[42]: x");
    assert_eq!(syslog.stamp.date, Some((10, 4)));
    let glog = stamp("E1024 08:15:30.123456 4242 main.cc:12] boom");
    assert_eq!(glog.letter, Some((Level::Error, (0, 1))));
    assert_eq!(glog.range.0, 1);
    assert_eq!(stamp("08:15:30.5 tick").stamp.time, (8, 15, 30));
    let access = stamp("127.0.0.1 - - [24/Oct/2026:08:15:30 +0000] \"GET / HTTP/1.1\" 200");
    assert_eq!(
        (access.range, access.stamp.date),
        ((15, 35), Some((10, 24)))
    );
    assert_eq!(
        stamp("time=2026-10-24T08:15:30Z level=info msg=x").range,
        (5, 25)
    );
    for text in [
        "version 1.2.3",
        "12:30 meeting",
        "2026-13-01 08:00:00 x",
        "abc",
    ] {
        assert!(plain::stamp(text).is_none(), "{text}");
    }
    // Days count from 1970-01-01; ordering follows the clock.
    let day = |y, m, d| {
        Stamp {
            year: Some(y),
            date: Some((m, d)),
            time: (0, 0, 0),
        }
        .day()
    };
    assert_eq!(day(1970, 1, 1), Some(0));
    assert_eq!(day(2000, 3, 1), Some(11_017));
    assert_eq!(
        day(2026, 10, 24).zip(day(2026, 10, 23)).map(|(a, b)| a - b),
        Some(1)
    );
}

#[test]
fn level_words_count_only_where_they_cannot_be_prose() {
    let found = |text: &str| plain::level(text, 0).map(|f| (f.level, f.strong));
    assert_eq!(found("2026 ERROR boom"), Some((Level::Error, true)));
    assert_eq!(found("x [warn] y"), Some((Level::Warn, true)));
    assert_eq!(found("ts=1 level=error msg=x"), Some((Level::Error, true)));
    assert_eq!(
        found("{\"level\":\"debug\",\"msg\":\"x\"}"),
        Some((Level::Debug, true))
    );
    assert_eq!(
        found("src/main.rs:3:5: error[E0308]: x"),
        Some((Level::Error, true))
    );
    assert_eq!(found("Error: boom"), Some((Level::Error, true)));
    assert_eq!(found("Info CBS loaded"), Some((Level::Info, false)));
    for prose in [
        "the error was fixed",
        "Connection error, retrying",
        "errors happen",
    ] {
        assert_eq!(found(prose), None, "{prose}");
    }
    assert!(plain::problem("\u{1b}[31mERROR\u{1b}[0m failed"));
    assert!(plain::problem("thread 'main' panicked at src/main.rs:2:5:"));
    assert!(!plain::problem("WARN slow"));
    assert_eq!(plain::strip_ansi("\u{1b}[1;31mred\u{1b}[0m"), "red");
    assert_eq!(
        plain::strip_ansi("\u{1b}]8;;https://x\u{7}link\u{1b}]8;;\u{7}"),
        "link"
    );
}

#[test]
fn chapter_lines_of_plain_text_and_what_is_only_prose() {
    for (line, level) in [
        ("第一卷 风起", 1),
        ("第十二章 少年", 2),
        ("第一百二十三章：重逢", 2),
        ("第3章", 2),
        ("第三章风起云涌", 2),
        ("第二节 方法", 3),
        ("第12話 雨", 2),
        ("第3巻 旅立ち", 1),
        ("　　第十二章　少年", 2),
        ("序章", 2),
        ("番外三 重逢", 2),
        ("Chapter 12: The Storm", 2),
        ("PART ONE", 1),
        ("Book II", 1),
        ("Prologue", 2),
    ] {
        assert_eq!(plain::chapter(line), Some(level), "{line}");
    }
    for line in [
        "第一章说的是他如何离开了家乡，又如何回来的。",
        "第一个人走了",
        "Part of the plan is to wait.",
        "Section 2 describes the method in detail.",
        "序列号见包装",
        "",
    ] {
        assert_eq!(plain::chapter(line), None, "{line}");
    }
    assert_eq!(plain::numbered("一、总则"), Some(1));
    assert_eq!(plain::numbered("（一）适用范围"), Some(2));
    assert_eq!(plain::numbered("(三) 附则"), Some(2));
    assert_eq!(plain::numbered("一、我们走吧，天黑了。"), None);
    assert_eq!(plain::numbered("1、苹果"), None);
    assert_eq!(plain::setext("====="), Some(1));
    assert_eq!(plain::setext("---"), Some(2));
    assert_eq!(plain::setext("--"), None);
    assert!(plain::setext_title("Report"));
    assert!(!plain::setext_title("This ends like a sentence."));
}

#[test]
fn plain_text_and_log_pages_become_blocks_as_written() {
    let page = lines("第一章 少年\n　　他走了。\n  缩进的行\n\nReport\n======");
    let blocks = plain::blocks(&page, Format::Text, false);
    let kinds: Vec<Kind> = blocks.iter().map(|b| b.kind).collect();
    assert_eq!(
        kinds,
        [
            Kind::Heading(3),
            Kind::Text,
            Kind::Text,
            Kind::Text,
            Kind::Heading(3),
            Kind::Rule
        ]
    );
    // Leading spaces stay; a chapter heading loses its indent; a blank line is empty.
    assert_eq!(blocks[2].spans[0].text, "  缩进的行");
    assert_eq!(blocks[0].spans[0].text, "第一章 少年");
    assert!(blocks[3].spans.is_empty());

    let log = lines("2026-10-24 08:15:30,123 ERROR failed\n\tat a.b(C.java:1)");
    let blocks = plain::blocks(&log, Format::Log, false);
    assert!(blocks.iter().all(|b| b.kind == Kind::Log));
    let spans: Vec<(&str, Option<Token>)> = blocks[0]
        .spans
        .iter()
        .map(|s| (s.text.as_str(), s.token))
        .collect();
    assert_eq!(
        spans,
        [
            ("2026-10-24 08:15:30,123", Some(Token::Comment)),
            (" ", None),
            ("ERROR", Some(Token::Deleted)),
            (" failed", None),
        ]
    );
    assert_eq!(blocks[1].spans[0].text, "\tat a.b(C.java:1)");
    // Source mode shows every line as code, whatever the format.
    assert!(
        plain::blocks(&log, Format::Log, true)
            .iter()
            .all(|b| b.kind == Kind::Code)
    );
}
