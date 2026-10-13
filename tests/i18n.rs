//! Interface languages: table integrity, lookups, settings and source coverage.
use feathermd::{
    i18n::{self, Lang},
    settings::Settings,
    update,
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

const TW: usize = 1;
const EN: usize = 2;
const JA: usize = 3;

fn is_cjk(c: char) -> bool {
    matches!(c, '\u{3000}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}' | '\u{ff00}'..='\u{ffef}')
}

/// Simplified-only forms that are a mistake in Traditional Chinese text.
const NOT_TRADITIONAL: &str = concat!(
    "们这个为时对说过还没进开关问间题页显设认识读写语词话请让记许论计证试详误调资费车转软载辑输",
    "边达运远连选递逻释钮错长闭阅随隐难项顺须预领频风饰驱验齐龙专业东两严丰临举么义乐书买乱亚产",
    "亲仅从价众优会传伤体侧储兴养决况净减击则刚创删别剧务动劳势区华协单卖历压参双发变叠号启员响",
    "团围图圆场坏块坚声处备复头夹夺奖学宁宝实宽寻导将尔层属岁币师带帧帮广库应废异张弹强归当录径",
    "忆态总恶惊惯戏战执扩扫扬扰护报择换据损摄摆数断无旧显晓暂术杂权条极构标栏树样档检楼横欢毕气",
    "汇汉沟泽洁浅测济浏浓润渐温满滤滚灭灯灵灾点炼烁烂烟热爱状独环现电画畅盖盘确码种积称稳穷笔签",
    "简类紧红纤约级纪纯纲纵纸纹线练组细织终经绑结绕绘给络绝统继绩续维综绿缓编缘缩网罗罚习聪肤胜",
    "脑脚脱节范药获营虑虚虽补装见观规视览觉触订讨训议讯讲访评译该诸课谁谈谢谱贝负贡财责败账货质",
    "贫购贴贵赏赖赞跃轨轮轻较辅辞违迟适遗邮针钟钢钥钱铁银链销锁键镜门闪闲队阳阴阶际陆陈险雾静顶",
    "顾颜额飞饭饮鱼鸟麦黄龄后冲尽户",
);

/// Simplified forms whose Japanese shinjitai is a different character.
const NOT_JAPANESE: &str = concat!(
    "们这个为时对说过还进开关问间题页显设认识读语词话请让记许论计证试详误调资费车转软载辑输边达运",
    "远连选递逻释钮错长闭阅隐难项顺须预领频风饰驱验齐龙专业东两严丰临举么义乐书买亚产亲仅从价众优",
    "传伤侧储兴养决况净减击则刚创删别剧务动劳势华协单卖历压发变启员响团围图圆场坏块坚处备复头夹夺",
    "奖宁实宽寻导尔层岁币师带帧帮广库应废异张弹强归录忆态总恶惊惯戏战执扩扫扬扰护报择换损摄摆无显",
    "晓暂术杂权极构标栏树样档检欢毕气汇汉沟泽洁测济浏浓润渐满滤滚灭灵灾炼烁烂烟热爱环现电畅盖盘确",
    "码种积稳穷笔签简类紧红纤约级纪纯纲纵纸纹线练组细织终经绑结绕绘给络绝统继绩续维综绿缓编缘缩网",
    "罗罚习聪肤胜脑节范药获营虑虽补见观规视览觉订讨训议讯讲访评译该诸课谁谈谢谱贝负贡财责败账货质",
    "贫购贴贵赏赖赞跃轨轮轻较辅违迟适遗邮针钟钢钥钱铁银链销锁键镜门闪闲队阳阴阶际陆陈险雾顶顾颜额",
    "飞饭饮鱼鸟麦龄冲户",
);

type Shape<'a> = (
    usize,
    usize,
    usize,
    Option<&'a str>,
    Vec<&'a str>,
    usize,
    bool,
    bool,
);

/// Everything a translation must keep: placeholders, braces, mnemonics, the
/// shortcut after a tab, file-dialog patterns, line breaks and edge spaces.
fn shape(text: &str) -> Shape<'_> {
    (
        text.matches("{}").count(),
        text.matches(['{', '}']).count(),
        text.matches('&').count(),
        text.split_once('\t').map(|(_, shortcut)| shortcut),
        text.split('\0').skip(1).step_by(2).collect(),
        text.matches('\n').count(),
        text.starts_with(char::is_whitespace),
        text.ends_with(char::is_whitespace),
    )
}

#[test]
fn every_row_keeps_placeholders_mnemonics_and_shortcuts() {
    let mut keys = HashSet::new();
    for row in i18n::table() {
        assert!(keys.insert(row[0]), "duplicate key {:?}", row[0]);
        assert!(
            row[0].chars().any(is_cjk),
            "key without Chinese {:?}",
            row[0]
        );
        for text in row {
            assert!(!text.trim().is_empty(), "{row:?}");
            if let Some(at) = text.find('&') {
                assert!(
                    text[at + 1..].starts_with(|c: char| c.is_ascii_alphanumeric()),
                    "mnemonic {row:?}"
                );
            }
            assert_eq!(shape(text), shape(row[0]), "{row:?}");
        }
        assert!(!row[EN].chars().any(is_cjk), "CJK in English {row:?}");
        for (column, banned) in [(TW, NOT_TRADITIONAL), (JA, NOT_JAPANESE)] {
            let found: String = row[column]
                .chars()
                .filter(|c| banned.contains(*c))
                .collect();
            assert!(
                found.is_empty(),
                "simplified {found:?} in {:?}",
                row[column]
            );
        }
    }
}

#[test]
fn lookups_follow_the_language_and_fall_back_to_the_source() {
    assert_eq!(i18n::tr_in(Lang::En, "记事本"), "Notepad");
    assert_eq!(i18n::tr_in(Lang::ZhCn, "记事本"), "记事本");
    assert_eq!(i18n::tr_in(Lang::Ja, "未登记的文字"), "未登记的文字");
    assert_eq!(
        i18n::trf_in(Lang::En, "GitHub 返回 HTTP {}", &[&503]),
        "GitHub returned HTTP 503"
    );
    assert_eq!(
        i18n::trf_in(Lang::ZhTw, "网络错误（{}）", &[&12002]),
        "網路錯誤（12002）"
    );
    // A missing argument leaves its slot empty; extra arguments are ignored.
    assert_eq!(i18n::trf_in(Lang::ZhCn, "{} 和 {}", &[&1]), "1 和 ");
    assert_eq!(i18n::trf_in(Lang::ZhCn, "无参数", &[&1]), "无参数");
    let now = 1_800_000_000;
    assert_eq!(update::ago_in(Lang::En, now - 60, now), "1 minute ago");
    assert_eq!(update::ago_in(Lang::En, now - 7200, now), "2 hours ago");
    assert_eq!(update::ago_in(Lang::Ja, now - 86400, now), "1 日前");
    assert_eq!(update::ago_in(Lang::ZhTw, 0, now), "尚未檢查");
    assert_eq!(update::ago_in(Lang::ZhCn, now - 600, now), "10 分钟前");
}

#[test]
fn windows_languages_map_to_the_nearest_translation() {
    for (id, lang) in [
        (0x0804, Lang::ZhCn),
        (0x1004, Lang::ZhCn),
        (0x0004, Lang::ZhCn),
        (0x0404, Lang::ZhTw),
        (0x0c04, Lang::ZhTw),
        (0x1404, Lang::ZhTw),
        (0x7c04, Lang::ZhTw),
        (0x0411, Lang::Ja),
        (0x0409, Lang::En),
        (0x0809, Lang::En),
        (0x0407, Lang::En),
        (0x0412, Lang::En),
        (0x0000, Lang::En),
    ] {
        assert_eq!(Lang::from_langid(id), lang, "{id:#06x}");
    }
    for lang in Lang::ALL {
        assert_eq!(Lang::parse(lang.code()), Some(lang));
        assert_eq!(Lang::ALL[lang.index()], lang);
        assert_eq!(Lang::resolve(Some(lang)), lang);
    }
    assert_eq!(Lang::parse(" ZH-tw "), Some(Lang::ZhTw));
    assert_eq!(Lang::parse("system"), None);
    assert_eq!(Lang::parse("fr"), None);
    // Interface faces keep each language's regional glyph forms.
    assert_eq!(Lang::ZhCn.ui_font(), "Microsoft YaHei UI");
    assert_eq!(Lang::ZhTw.ui_font(), "Microsoft JhengHei UI");
    assert_eq!(Lang::Ja.ui_font(), "Yu Gothic UI");
}

#[test]
fn language_setting_round_trips_and_defaults_to_the_system() {
    assert_eq!(Settings::default().language, None);
    assert!(
        Settings::default()
            .serialize()
            .contains("\nlanguage=system\n")
    );
    for lang in Lang::ALL {
        let settings = Settings {
            language: Some(lang),
            ..Settings::default()
        };
        let text = settings.serialize();
        assert!(text.contains(&format!("\nlanguage={}\n", lang.code())));
        assert_eq!(Settings::parse(&text), settings);
    }
    for bad in ["language=klingon", "language=", "language=system"] {
        assert_eq!(Settings::parse(bad).language, None, "{bad}");
    }
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Decoded string literals outside comments, with up to 48 bytes of the code
/// before each one.
fn literals(code: &str) -> Vec<(String, String)> {
    let b = code.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'\'' => {
                // A char literal ('x', '\n', '"'); otherwise a lifetime.
                if b.get(i + 1) == Some(&b'\\') {
                    i += 2;
                    while i < b.len() && b[i] != b'\'' {
                        i += 1;
                    }
                    i += 1;
                } else {
                    let len = code[i + 1..].chars().next().map_or(1, char::len_utf8);
                    if b.get(i + 1 + len) == Some(&b'\'') {
                        i += len + 2;
                    } else {
                        i += 1;
                    }
                }
            }
            b'r' if (b.get(i + 1) == Some(&b'"') || b.get(i + 1) == Some(&b'#'))
                && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) =>
            {
                let mut j = i + 1;
                while b.get(j) == Some(&b'#') {
                    j += 1;
                }
                if b.get(j) != Some(&b'"') {
                    i += 1;
                    continue;
                }
                let close = format!("\"{}", "#".repeat(j - i - 1));
                let end = j + 1 + code[j + 1..].find(&close).unwrap();
                out.push((code[j + 1..end].to_string(), before(code, i)));
                i = end + close.len();
            }
            b'"' => {
                let start = i;
                let mut text = String::new();
                i += 1;
                while b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 1;
                        match b[i] {
                            b'n' => text.push('\n'),
                            b't' => text.push('\t'),
                            b'r' => text.push('\r'),
                            b'0' => text.push('\0'),
                            b'u' => {
                                let close = i + code[i..].find('}').unwrap();
                                let hex = &code[i + 2..close];
                                text.push(
                                    char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap(),
                                );
                                i = close;
                            }
                            b'\n' => {
                                while b[i + 1].is_ascii_whitespace() {
                                    i += 1;
                                }
                            }
                            c => text.push(c as char),
                        }
                        i += 1;
                    } else {
                        let c = code[i..].chars().next().unwrap();
                        text.push(c);
                        i += c.len_utf8();
                    }
                }
                out.push((text, before(code, start)));
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

fn before(code: &str, at: usize) -> String {
    let mut from = at.saturating_sub(48);
    while !code.is_char_boundary(from) {
        from += 1;
    }
    code[from..at]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// The literal is the text argument of tr / trf / key! (or of tr_in / trf_in
/// after a language argument).
fn wrapped(prefix: &str) -> bool {
    let call = |name: &str, rest: &str| {
        prefix.ends_with(&format!("{name}{rest}"))
            && prefix[..prefix.len() - name.len() - rest.len()]
                .ends_with(|c: char| !(c.is_alphanumeric() || c == '_'))
    };
    if call("tr", "(") || call("trf", "(") || call("key!", "(") {
        return true;
    }
    ["tr_in(", "trf_in("].iter().any(|name| {
        prefix.rfind(name).is_some_and(|at| {
            let args = &prefix[at + name.len()..];
            args.ends_with(',') && !args[..args.len() - 1].contains([',', '(', ')'])
        })
    })
}

#[test]
fn every_chinese_interface_literal_goes_through_the_table() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    let keys: HashSet<&str> = i18n::table().iter().map(|row| row[0]).collect();
    let mut used = HashSet::new();
    let mut problems = Vec::new();
    for file in files {
        if file.starts_with(root.join("i18n")) || file == root.join("i18n.rs") {
            continue;
        }
        let text = fs::read_to_string(&file).unwrap();
        let code = text.split("#[cfg(test)]").next().unwrap();
        for (literal, prefix) in literals(code) {
            if !literal.chars().any(is_cjk) {
                continue;
            }
            let name = file.strip_prefix(&root).unwrap().display();
            if !wrapped(&prefix) {
                problems.push(format!("{name}: not translated: {literal:?}"));
            } else if !keys.contains(literal.as_str()) {
                problems.push(format!("{name}: missing from the table: {literal:?}"));
            }
            used.insert(literal);
        }
    }
    for key in &keys {
        if !used.contains(*key) {
            problems.push(format!("unused row: {key:?}"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
