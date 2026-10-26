//! 必应搜索的 URL 与网页解析属于插件业务；网络来源、DNS 与资源额度由宿主再次校验。
use super::{RemoteImage, plain};
use regex::Regex;
use serde_json::Value;
use std::{collections::BTreeSet, sync::OnceLock};

pub(crate) const SEARCH_LIMIT: usize = 2 * 1024 * 1024;
const MAX_SEARCH_RESULTS: usize = 12;
const MAX_URL_BYTES: usize = 4096;

pub(crate) fn search_url(query: &str) -> String {
    let mut url = url::Url::parse("https://cn.bing.com/images/search").expect("fixed Bing URL");
    url.query_pairs_mut().extend_pairs([
        ("q", query),
        ("first", "1"),
        ("count", "12"),
        ("adlt", "strict"),
        ("mkt", "zh-CN"),
        ("cc", "CN"),
        ("setlang", "zh-Hans"),
    ]);
    url.into()
}

pub(crate) fn approved_public_url(source: &str) -> Result<url::Url, String> {
    if source.len() > MAX_URL_BYTES || source.chars().any(char::is_control) {
        return Err(String::from("Invalid wallpaper source URL"));
    }
    let url = url::Url::parse(source).map_err(|_| String::from("Invalid wallpaper source URL"))?;
    if url.scheme() != "https"
        || url.port().is_some_and(|port| port != 443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(String::from(
            "Wallpaper sources require credential-free HTTPS on port 443",
        ));
    }
    // 插件仅保留 HTTPS 公网域名候选；实际 DNS 与 SSRF 防护由宿主负责。
    let Some(url::Host::Domain(host)) = url.host() else {
        return Err(String::from("Wallpaper sources require a public DNS name"));
    };
    if !public_hostname(host) {
        return Err(String::from("Wallpaper sources require a public DNS name"));
    }
    Ok(url)
}

fn public_hostname(host: &str) -> bool {
    let labels = host.split('.').collect::<Vec<_>>();
    if host.len() > 253
        || labels.len() < 2
        || labels.iter().any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
        || labels
            .last()
            .is_some_and(|label| label.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    ![
        "localhost",
        "local",
        "internal",
        "intranet",
        "lan",
        "home",
        "corp",
        "onion",
        "test",
        "invalid",
        "example",
        "arpa",
        "alt",
        "localdomain",
    ]
    .iter()
    .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
        && !["example.com", "example.net", "example.org"]
            .iter()
            .any(|name| host == *name || host.ends_with(&format!(".{name}")))
}

pub(crate) fn approved_thumbnail_url(source: &str) -> Result<url::Url, String> {
    let url = approved_public_url(source)?;
    let host = url.host_str().unwrap_or("");
    let allowed = ["mm.bing.net", "explicit.bing.net"].iter().any(|suffix| {
        ["ts", "tse"]
            .iter()
            .any(|prefix| (1..=4).any(|number| host == format!("{prefix}{number}.{suffix}")))
    });
    if !allowed
        || url.path() != "/th"
        || !url
            .query_pairs()
            .any(|(key, value)| key == "id" && !value.is_empty())
    {
        return Err(String::from(
            "Wallpaper previews must use approved Bing thumbnail hosts",
        ));
    }
    Ok(url)
}

fn html_entities(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find('&') {
        result.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest.chars().take(12).position(|character| character == ';');
        let decoded = end.and_then(|end| {
            // 实体名称为 ASCII，输入仍可能包含其他字符；拒绝无效 UTF-8 切片。
            let entity = rest.get(1..end)?;
            match entity {
                "quot" => Some('"'),
                "apos" | "#39" => Some('\''),
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "nbsp" => Some(' '),
                _ => entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|value| u32::from_str_radix(value, 16).ok())
                    .or_else(|| {
                        entity
                            .strip_prefix('#')
                            .and_then(|value| value.parse().ok())
                    })
                    .and_then(char::from_u32),
            }
        });
        if let (Some(end), Some(character)) = (end, decoded) {
            result.push(character);
            rest = &rest[end + 1..];
        } else {
            result.push('&');
            rest = &rest[1..];
        }
    }
    result.push_str(rest);
    result
}

fn anchor_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r#"(?is)<a\b((?:[^\"'<>]|\"[^\"]*\"|'[^']*')*)>"#).expect("fixed anchor pattern")
    })
}
fn attribute_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r#"(?i)([a-z_:][a-z0-9_:.-]*)\s*=\s*(?:\"([^\"]*)\"|'([^']*)'|([^\s\"'=<>`]+))"#)
            .expect("fixed attribute pattern")
    })
}

fn attribute<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    attribute_pattern()
        .captures_iter(attrs)
        .take(64)
        .find_map(|attribute| {
            if attribute[1].eq_ignore_ascii_case(name) {
                attribute
                    .get(2)
                    .or_else(|| attribute.get(3))
                    .or_else(|| attribute.get(4))
                    .map(|value| value.as_str())
            } else {
                None
            }
        })
}
fn validate_query(html: &str, query: &str) -> Result<(), String> {
    static INPUT: OnceLock<Regex> = OnceLock::new();
    let input = INPUT.get_or_init(|| {
        Regex::new(r#"(?is)<input\b((?:[^\"'<>]|\"[^\"]*\"|'[^']*')*)>"#)
            .expect("fixed input pattern")
    });
    for input in input.captures_iter(html).take(100) {
        let attrs = &input[1];
        if attribute(attrs, "id") == Some("sb_form_q") {
            if attribute(attrs, "value").is_some_and(|value| html_entities(value) == query) {
                return Ok(());
            }
            break;
        }
    }
    Err(String::from(
        "Bing returned a different search query or an unreadable response; retry",
    ))
}
fn dimensions(href: Option<&str>, query: &str) -> Option<(u64, u64)> {
    let href = href?;
    let base = url::Url::parse("https://cn.bing.com/").expect("fixed Bing URL");
    let Ok(url) = base.join(&html_entities(href)) else {
        return None;
    };
    if url.scheme() != "https"
        || !matches!(url.host_str(), Some("cn.bing.com" | "www.bing.com"))
        || url.path() != "/images/search"
    {
        return None;
    }
    let mut width = 0;
    let mut height = 0;
    let mut same_query = false;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "expw" => width = value.parse().unwrap_or(0),
            "exph" => height = value.parse().unwrap_or(0),
            "q" => same_query = value == query,
            _ => {}
        }
    }
    if !same_query {
        None
    } else if width > 30_000 || height > 30_000 {
        Some((0, 0))
    } else {
        Some((width, height))
    }
}

fn tag_end(tag: &str) -> Option<usize> {
    let mut quote = None;
    tag.bytes().position(|byte| {
        if quote == Some(byte) {
            quote = None;
        } else if quote.is_none() {
            if matches!(byte, b'\'' | b'"') {
                quote = Some(byte);
            } else if byte == b'>' {
                return true;
            }
        }
        false
    })
}

fn after_raw_text<'a>(html: &'a str, name: &str) -> Option<&'a str> {
    let closing = format!("</{name}");
    let mut rest = html;
    while let Some(start) = rest
        .as_bytes()
        .windows(closing.len())
        .position(|part| part.eq_ignore_ascii_case(closing.as_bytes()))
    {
        rest = &rest[start..];
        if rest
            .as_bytes()
            .get(closing.len())
            .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(*byte, b'/' | b'>'))
        {
            let content = &html[..html.len() - rest.len()];
            if name == "script"
                && content.contains("<!--")
                && content.to_ascii_lowercase().contains("<script")
            {
                // 双重转义脚本中的第一个闭合文本不一定结束元素。
                return None;
            }
            return Some(&rest[tag_end(rest)? + 1..]);
        }
        rest = &rest[1..];
    }
    None
}

fn has_no_results(html: &str) -> bool {
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            let Some(end) = comment.find("-->") else {
                return false;
            };
            rest = &comment[end + 3..];
            continue;
        }
        let Some(first) = rest.as_bytes().get(1).copied() else {
            return false;
        };
        if !first.is_ascii_alphabetic() && !matches!(first, b'!' | b'?' | b'/') {
            rest = &rest[1..];
            continue;
        }
        let Some(end) = tag_end(rest) else {
            return false;
        };
        let tag = &rest[1..end];
        rest = &rest[end + 1..];
        if !first.is_ascii_alphabetic() {
            continue;
        }
        let name_end = tag
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b':'))
            .count();
        let name = tag[..name_end].to_ascii_lowercase();
        let attrs = &tag[name_end..];
        if attrs
            .as_bytes()
            .first()
            .is_some_and(|byte| !byte.is_ascii_whitespace() && *byte != b'/')
        {
            continue;
        }
        // 注释、属性值和原始文本元素中的标签不构成无结果标记。
        if name == "plaintext" {
            return false;
        }
        if matches!(
            name.as_str(),
            "script"
                | "style"
                | "textarea"
                | "title"
                | "xmp"
                | "iframe"
                | "noembed"
                | "noframes"
                | "noscript"
        ) {
            let Some(after) = after_raw_text(rest, &name) else {
                return false;
            };
            rest = after;
            continue;
        }
        if attribute(attrs, "id").is_some_and(|value| html_entities(value) == "nr_indicator")
            || attribute(attrs, "class").is_some_and(|value| {
                html_entities(value)
                    .split_ascii_whitespace()
                    .any(|class| class == "b_no")
            })
        {
            return true;
        }
    }
    false
}

pub(crate) fn parse_images(bytes: &[u8], query: &str) -> Result<Vec<RemoteImage>, String> {
    if bytes.len() > SEARCH_LIMIT {
        return Err(String::from("Bing search response exceeds its limit"));
    }
    let html =
        std::str::from_utf8(bytes).map_err(|_| String::from("Invalid Bing search response"))?;
    validate_query(html, query)?;
    let mut images = Vec::new();
    let mut seen = BTreeSet::new();
    let mut result_anchors = 0;
    for anchor in anchor_pattern().captures_iter(html).take(1000) {
        let attrs = &anchor[1];
        if attrs.len() > 32 * 1024 {
            continue;
        }
        let mut class = None;
        let mut metadata = None;
        let mut href = None;
        for attribute in attribute_pattern().captures_iter(attrs).take(64) {
            let Some(value) = attribute
                .get(2)
                .or_else(|| attribute.get(3))
                .or_else(|| attribute.get(4))
            else {
                continue;
            };
            match attribute[1].to_ascii_lowercase().as_str() {
                "class" => class = Some(value.as_str()),
                "m" => metadata = Some(value.as_str()),
                "href" => href = Some(value.as_str()),
                _ => {}
            }
        }
        if !class.is_some_and(|class| class.split_ascii_whitespace().any(|part| part == "iusc")) {
            continue;
        }
        result_anchors += 1;
        let Some(metadata) = metadata else { continue };
        let Ok(metadata) = serde_json::from_str::<Value>(&html_entities(metadata)) else {
            continue;
        };
        let Some(original_url) = metadata.get("murl").and_then(Value::as_str) else {
            continue;
        };
        let Some(preview_url) = metadata.get("turl").and_then(Value::as_str) else {
            continue;
        };
        let Some(source_url) = metadata.get("purl").and_then(Value::as_str) else {
            continue;
        };
        let (Ok(original), Ok(preview), Ok(source)) = (
            approved_public_url(original_url),
            approved_thumbnail_url(preview_url),
            approved_public_url(source_url),
        ) else {
            continue;
        };
        let title = plain(
            &html_entities(metadata.get("t").and_then(Value::as_str).unwrap_or("")),
            120,
        );
        let Some((width, height)) = dimensions(href, query) else {
            continue;
        };
        if !seen.insert(original.as_str().to_owned()) {
            continue;
        }
        images.push(RemoteImage {
            result_id: String::new(),
            title: if title.is_empty() {
                source.host_str().unwrap_or("Bing image").into()
            } else {
                title
            },
            source_url: source.into(),
            width,
            height,
            original_url: original.into(),
            preview_url: preview.into(),
        });
        if images.len() == MAX_SEARCH_RESULTS {
            break;
        }
    }
    if images.is_empty() {
        if result_anchors > 0 {
            return Err(String::from(
                "Bing returned no supported HTTPS image results; try another keyword",
            ));
        }
        // 验证码、变动后的标记或上游错误不能冒充空结果成功。
        if !has_no_results(html) {
            return Err(String::from(
                "Bing search response could not be read; retry or open Bing in your browser",
            ));
        }
    }
    Ok(images)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn result_markup(title: &str, original: &str, source: &str) -> String {
        let metadata = serde_json::json!({"murl":original,"purl":source,"turl":"https://ts2.mm.bing.net/th?id=OIP.test&pid=15.1","t":title});
        let escaped = serde_json::to_string(&metadata)
            .unwrap()
            .replace('&', "&amp;")
            .replace('"', "&quot;");
        format!(
            "<input value='test-query' name='q' id='sb_form_q'><a href='/images/search?expw=1920&amp;exph=1080&amp;q=test-query' m=\"{escaped}\" data-other=\"x\" class='extra iusc'>image</a>"
        )
    }
    #[test]
    fn bing_parser_preserves_rank_titles_and_real_source_without_license_filtering() {
        let markup = format!(
            "{}{}{}",
            result_markup(
                "海洋 & sky",
                "https://images.wikipedia.org/one.jpg",
                "https://www.wikipedia.org/one"
            ),
            result_markup(
                "Second",
                "https://images.wikipedia.org/two.png",
                "https://www.wikipedia.org/two"
            ),
            result_markup(
                "Duplicate",
                "https://images.wikipedia.org/one.jpg",
                "https://www.wikipedia.org/one"
            )
        );
        let images = parse_images(markup.as_bytes(), "test-query").unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].title, "海洋 & sky");
        assert_eq!(images[1].title, "Second");
        assert_eq!((images[0].width, images[0].height), (1920, 1080));
        assert_eq!(images[0].source_url, "https://www.wikipedia.org/one");
        let url = url::Url::parse(&search_url("海洋 & sky")).unwrap();
        assert!(
            url.query_pairs()
                .any(|(key, value)| key == "q" && value == "海洋 & sky")
        );
        assert!(
            url.query_pairs()
                .any(|(key, value)| key == "mkt" && value == "zh-CN")
        );
    }
    #[test]
    fn unrelated_results_do_not_hide_matching_duplicates() {
        let unrelated = result_markup(
            "Unrelated result",
            "https://images.wikipedia.org/shared.jpg",
            "https://www.wikipedia.org/shared",
        )
        .replace("&amp;q=test-query", "&amp;q=unrelated");
        let matching = result_markup(
            "Matching result",
            "https://images.wikipedia.org/shared.jpg",
            "https://www.wikipedia.org/shared",
        );
        let markup = format!("{unrelated}{matching}{matching}");
        let images = parse_images(markup.as_bytes(), "test-query").unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].title, "Matching result");
    }
    #[test]
    fn no_result_markers_in_query_text_comments_attributes_and_raw_text_are_not_success() {
        assert!(
            parse_images(
                br#"<input id="sb_form_q" value="id='nr_indicator'"><div>Verify you are human</div>"#,
                "id='nr_indicator'",
            )
            .is_err()
        );
        for fragment in [
            "id='nr_indicator' class=\"b_no\"",
            "&lt;div id='nr_indicator'&gt;No results&lt;/div&gt;",
            "<!-- <div id='nr_indicator'>No results</div> -->",
            "<div data-note=\"<span id='nr_indicator'>\">Challenge</div>",
            "<div data-note='class=\"b_no\"'>Challenge</div>",
            "<div class='b_no_extra'>Challenge</div>",
            "<script>const markup = '<div id=\"nr_indicator\">';</script>",
            "<ScRiPt/>const markup = '<div class=\"b_no\">';</sCrIpT>",
            "<script></scriptlet><div class='b_no'>Still script text</div></script>",
            "<script><!--<script></script><div id='nr_indicator'></div>--></script>",
            "<style><div class='b_no'>Still style text</div></style>",
            "<textarea><div id='nr_indicator'>Still text</div></textarea>",
            "<title><div class='b_no'>Still text</div></title>",
            "<plaintext><div class='b_no'>Still text</div>",
            "<!-- <div class='b_no'>Unclosed comment",
            "<script><div class='b_no'>Unclosed script",
            "<div data-note=\"<span id='nr_indicator'>Unclosed attribute",
        ] {
            let markup = format!("<input id='sb_form_q' value='test-query'>{fragment}");
            assert!(
                parse_images(markup.as_bytes(), "test-query").is_err(),
                "accepted a textual marker as no results: {fragment}",
            );
        }
    }
    #[test]
    fn actual_no_result_elements_accept_ids_and_class_lists() {
        for fragment in [
            "<div id='nr_indicator'>No results</div>",
            "<div id=nr_indicator>No results</div>",
            "<DIV ID = \"nr_indicator\">No results</DIV>",
            "<div class='notice b_no extra'>No results</div>",
            "<div class='notice\tb_no\nextra'>No results</div>",
            "<div class='notice b&#95;no extra'>No results</div>",
            "<!-- class='b_no' --><div id='nr_indicator'>No results</div>",
            "<script>const text = '<div class=\"b_no\">';</script><div class='notice b_no'>No results</div>",
        ] {
            let markup = format!("<input id='sb_form_q' value='test-query'>{fragment}");
            assert!(
                parse_images(markup.as_bytes(), "test-query")
                    .unwrap()
                    .is_empty(),
                "did not recognize a real no-results element: {fragment}",
            );
        }
    }
    #[test]
    fn unsafe_sources_and_changed_or_challenged_html_fail_explicitly() {
        for source in [
            "http://images.wikipedia.org/one.jpg",
            "https://127.0.0.1/one.jpg",
            "https://images.local/one.jpg",
        ] {
            assert!(
                parse_images(
                    result_markup("Unsafe", source, "https://www.wikipedia.org/one").as_bytes(),
                    "test-query"
                )
                .is_err()
            );
        }
        for markup in [
            "<html>Verify you are human</html>",
            "<a class='iusc' m='invalid'>broken</a>",
        ] {
            assert!(parse_images(markup.as_bytes(), "test-query").is_err());
        }
        assert!(
            parse_images(b"<html><input id='sb_form_q' value='test-query'><div id='nr_indicator'>No results</div></html>", "test-query")
                .unwrap()
                .is_empty()
        );
        assert!(parse_images(&vec![b' '; SEARCH_LIMIT + 1], "test-query").is_err());
    }
    #[test]
    fn returned_form_and_each_result_must_match_the_requested_query() {
        let markup = result_markup(
            "Other language is allowed",
            "https://images.wikipedia.org/one.jpg",
            "https://www.wikipedia.org/one",
        );
        assert!(parse_images(markup.as_bytes(), "different-query").is_err());
        let wrong_result = markup.replace("&amp;q=test-query", "&amp;q=unrelated");
        assert!(parse_images(wrong_result.as_bytes(), "test-query").is_err());
        let unknown_form = markup.replace("id='sb_form_q'", "id='unrecognized'");
        assert!(parse_images(unknown_form.as_bytes(), "test-query").is_err());
        let encoded = markup
            .replace("value='test-query'", "value='海洋 &amp; sky'")
            .replace("&amp;q=test-query", "&amp;q=%E6%B5%B7%E6%B4%8B+%26+sky");
        assert!(parse_images(encoded.as_bytes(), "海洋 & sky").is_ok());
    }
    #[test]
    fn all_remote_boundaries_reject_private_literals_reserved_dns_and_credentials() {
        assert!(approved_public_url("https://images.wikipedia.org/a.jpg?size=2").is_ok());
        for source in [
            "http://images.wikipedia.org/a.jpg",
            "https://user:secret@images.wikipedia.org/a.jpg",
            "https://images.wikipedia.org:444/a.jpg",
            "https://images.wikipedia.org/a.jpg#fragment",
            "https://127.0.0.1/a.jpg",
            "https://2130706433/a.jpg",
            "https://[::1]/a.jpg",
            "https://8.8.8.8/a.jpg",
            "https://localhost/a.jpg",
            "https://metadata.google.internal/a.jpg",
            "https://router.home.arpa/a.jpg",
            "https://images.local/a.jpg",
            "https://images.example/a.jpg",
            "https://images.wikipedia.org./a.jpg",
        ] {
            assert!(approved_public_url(source).is_err(), "{source}");
        }
        for source in [
            "https://ts2.mm.bing.net/th?id=OIP.test",
            "https://tse4.mm.bing.net/th?id=OIP.test",
            "https://ts2.explicit.bing.net/th?id=OIP.test",
        ] {
            assert!(approved_thumbnail_url(source).is_ok());
        }
        for source in [
            "https://ts2.mm.bing.net.evil.com/th?id=x",
            "https://arbitrary.bing.net/th?id=x",
            "https://ts2.mm.bing.net/private?id=x",
            "https://ts2.mm.bing.net/th",
        ] {
            assert!(approved_thumbnail_url(source).is_err());
        }
    }
}
