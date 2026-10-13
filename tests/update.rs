use feathermd::update::{
    self, CHECK_INTERVAL, LATEST_API, MAX_RESPONSE, Version, ago, due, parse_latest, release_page,
};

fn v(text: &str) -> Version {
    Version::parse(text).unwrap_or_else(|| panic!("version {text:?}"))
}

#[test]
fn versions_compare_numerically_and_reject_malformed_tags() {
    assert!(v("v0.1.10") > v("v0.1.9"));
    assert!(v("0.2.0") > v("v0.1.99"));
    assert!(v("1.0.0") > v("0.99.99"));
    assert_eq!(v("V0.1.5"), v("0.1.5"));
    assert_eq!(v("0.1.5+build.7"), v("0.1.5"));
    // A pre-release sorts before its release; numeric identifiers numerically.
    assert!(v("1.0.0-rc.1") < v("1.0.0"));
    assert!(v("1.0.0-rc.2") < v("1.0.0-rc.10"));
    assert!(v("1.0.0-alpha") < v("1.0.0-alpha.1"));
    assert!(v("1.0.0-1") < v("1.0.0-alpha"));
    assert_eq!(v("v2.3.4-rc.1").to_string(), "2.3.4-rc.1");
    for bad in [
        "",
        "v",
        "latest",
        "1.2",
        "1.2.3.4",
        "v1..3",
        "1.2.x",
        "-1.2.3",
        "1.2.3-",
        "1.2.3-a..b",
        "1.2.3-é",
        " 1 .2.3",
        "1.2.9999999999",
    ] {
        assert_eq!(Version::parse(bad), None, "{bad:?}");
    }
    assert!(Version::parse(&format!("1.2.3-{}", "a".repeat(80))).is_none());
    assert_eq!(update::current().to_string(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn latest_release_fields_come_only_from_the_top_level() {
    // Shaped like GitHub's response: nested objects repeat field names and the
    // body contains JSON-looking text; only top-level values may be used.
    let json = r#"{"url":"https://api.github.com/repos/x/y/releases/1",
      "author":{"login":"lianzy","tag_name":"v9.9.9","name":"decoy"},
      "assets":[{"name":"FeatherMD.exe","uploader":{"tag_name":"v8.8.8"},"size":783360,
                 "digest":"sha256:abc","nested":[[1,2,{"a":"}]"}],[]]}],
      "body":"Notes \"tag_name\":\"v7.7.7\" {not json} \\ end\n— done",
      "tag_name" : "v0.2.0",
      "name":"FeatherMD 羽读 v0.2.0 \ud83e\udeb6\u0007\u202e",
      "draft":false,"prerelease":false,"immutable":true,
      "created_at":"2026-11-01T08:00:00Z","published_at":"2026-11-02T09:10:11Z",
      "id":411002718,"ratio":-1.5e+3,"missing":null}"#;
    let release = parse_latest(json.as_bytes()).expect("release");
    assert_eq!(release.tag, "v0.2.0");
    assert_eq!(release.version, v("0.2.0"));
    // Escapes decode (including a surrogate pair); control/bidi characters are dropped.
    assert_eq!(release.title, "FeatherMD 羽读 v0.2.0 🪶");
    assert_eq!(release.published, "2026-11-02");
    assert_eq!(
        release.page(),
        "https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.2.0"
    );
    // BOM, whitespace and a missing title/date are accepted.
    let minimal = parse_latest("\u{feff} { \"tag_name\" : \"0.3.1\" } ".as_bytes()).unwrap();
    assert_eq!(
        (minimal.title.as_str(), minimal.published.as_str()),
        ("", "")
    );
    let null_name =
        parse_latest(br#"{"name":null,"tag_name":"v1.0.0","published_at":"soon"}"#).unwrap();
    assert_eq!(
        (null_name.title.as_str(), null_name.published.as_str()),
        ("", "")
    );
    assert!(LATEST_API.starts_with("https://api.github.com/repos/Lianzy-Baimiao/FeatherMD/"));
}

#[test]
fn unusable_responses_are_reported_not_trusted() {
    let error = |json: &str| parse_latest(json.as_bytes()).unwrap_err();
    assert_eq!(
        error(r#"{"tag_name":"v1.0.0","draft":true}"#),
        "最新发布不是正式版本"
    );
    assert_eq!(
        error(r#"{"prerelease":true,"tag_name":"v1.0.0"}"#),
        "最新发布不是正式版本"
    );
    assert_eq!(error(r#"{"name":"no tag"}"#), "最新发布缺少版本号");
    assert_eq!(error(r#"{"tag_name":null}"#), "最新发布缺少版本号");
    assert_eq!(
        error(r#"{"tag_name":"v1.0.0/../x"}"#),
        "最新发布的版本号无效"
    );
    assert_eq!(
        error(r#"{"tag_name":"nightly"}"#),
        "无法识别的版本号：nightly"
    );
    assert_eq!(
        error(r#"{"message":"Not Found","documentation_url":"x"}"#),
        "最新发布缺少版本号"
    );
    for broken in [
        "",
        "[]",
        "<!DOCTYPE html>",
        r#"{"tag_name":"v1.0.0""#,
        r#"{"tag_name":"v1.0.0",}"#,
        r#"{"tag_name":"v1\q.0"}"#,
        r#"{"tag_name":"\ud800"}"#,
        r#"{"a":[1,2}"#,
        "{\"tag_name\":\"v1.0\n.0\"}",
    ] {
        assert_eq!(error(broken), "更新信息格式无效", "{broken:?}");
    }
    let huge = format!(
        "{{\"tag_name\":\"v1.0.0\",\"body\":\"{}\"}}",
        "x".repeat(MAX_RESPONSE)
    );
    assert_eq!(error(&huge), "更新信息过大");
}

#[test]
fn truncated_and_mutated_payloads_never_panic() {
    let json = br#"{"author":{"login":"a","x":[{"y":"z\"}"}]},"tag_name":"v0.1.5","name":"\u7fbd","draft":false,"published_at":"2026-10-12T21:17:09Z"}"#;
    for end in 0..json.len() {
        let _ = parse_latest(&json[..end]);
    }
    let mut seed = 0x5eed_u64;
    for _ in 0..4000 {
        let mut copy = json.to_vec();
        for _ in 0..3 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let index = (seed >> 33) as usize % copy.len();
            copy[index] = (seed >> 25) as u8;
        }
        let _ = parse_latest(&copy);
    }
}

#[test]
fn automatic_checks_run_at_most_daily_and_pages_use_the_fixed_repository() {
    let now = 1_800_000_000;
    assert!(!due(false, 0, now));
    assert!(due(true, 0, now));
    assert!(!due(true, now - 60, now));
    assert!(!due(true, now - CHECK_INTERVAL + 1, now));
    assert!(due(true, now - CHECK_INTERVAL, now));
    assert!(
        due(true, now + 3600, now),
        "a clock moved backwards allows a check"
    );
    assert_eq!(ago(0, now), "尚未检查");
    assert_eq!(ago(now - 5, now), "刚刚");
    assert_eq!(ago(now - 600, now), "10 分钟前");
    assert_eq!(ago(now - 7200, now), "2 小时前");
    assert_eq!(ago(now - 3 * 86400, now), "3 天前");
    assert_eq!(ago(now + 50, now), "刚刚");
    assert_eq!(
        release_page("v0.1.5").as_deref(),
        Some("https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.5")
    );
    for bad in ["", "v1/../../x", "v1 2", "v1?x=1", "v1#x", &"v".repeat(65)] {
        assert_eq!(release_page(bad), None, "{bad:?}");
    }
}
