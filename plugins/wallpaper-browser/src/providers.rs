//! 网站列表与固定图片身份归插件核验；实际 DNS、下载与解码边界由宿主再次检查。
use super::{RemoteImage, plain};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) const IMAGE_COUNT: usize = 24;
const RESPONSE_LIMIT: usize = 2 * 1024 * 1024;

pub(crate) fn valid_query(query: &str) -> bool {
    query.len() <= 400 && query.chars().count() <= 100 && !query.chars().any(char::is_control)
}

pub(crate) fn category_options(provider: Provider, english: bool) -> Vec<Value> {
    let choices: &[(&str, &str, &str)] = match provider {
        Provider::Bizhihui => &[
            ("all", "全部分类", "All categories"),
            ("dongman", "动漫", "Anime"),
            ("renwu", "人物", "People"),
            ("fengjing", "风景", "Landscape"),
            ("youxi", "游戏", "Games"),
            ("mengchong", "动物", "Animals"),
            ("yuzhou", "宇宙", "Space"),
            ("jianyue", "简约", "Minimalism"),
        ],
        Provider::Wallcoo => &[
            ("all", "全部分类", "All categories"),
            ("landscape", "风景", "Landscape"),
            ("painting", "插画", "Painting"),
            ("anime", "动漫", "Anime"),
            ("animal", "动物", "Animals"),
            ("game", "游戏", "Games"),
            ("digital-art", "数位艺术", "Digital art"),
        ],
        Provider::Wallpaperscraft => &[
            ("all", "全部分类", "All categories"),
            ("nature", "自然", "Nature"),
            ("animals", "动物", "Animals"),
            ("anime", "动漫", "Anime"),
            ("cars", "汽车", "Cars"),
            ("city", "城市", "City"),
            ("space", "宇宙", "Space"),
            ("minimalism", "简约", "Minimalism"),
            ("abstract", "抽象", "Abstract"),
        ],
        Provider::Goodfon => &[
            ("all", "全部分类", "All categories"),
            ("landscapes", "风景", "Landscape"),
            ("nature", "自然", "Nature"),
            ("animals", "动物", "Animals"),
            ("city", "城市", "City"),
            ("space", "宇宙", "Space"),
            ("games", "游戏", "Games"),
            ("abstraction", "抽象", "Abstract"),
            ("minimalism", "简约", "Minimalism"),
        ],
    };
    choices
        .iter()
        .map(|(value, zh, en)| serde_json::json!({"value":value,"label":if english {en} else {zh}}))
        .collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Provider {
    #[default]
    Bizhihui,
    Wallcoo,
    Wallpaperscraft,
    Goodfon,
}
impl Provider {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Bizhihui => "bizhihui",
            Self::Wallcoo => "wallcoo",
            Self::Wallpaperscraft => "wallpaperscraft",
            Self::Goodfon => "goodfon",
        }
    }
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Bizhihui => "壁纸汇",
            Self::Wallcoo => "Wallcoo · 猫猫壁纸酷",
            Self::Wallpaperscraft => "WallpapersCraft",
            Self::Goodfon => "GoodFon",
        }
    }
    pub(crate) fn from_id(value: &str) -> Option<Self> {
        match value {
            "bizhihui" => Some(Self::Bizhihui),
            "wallcoo" => Some(Self::Wallcoo),
            "wallpaperscraft" => Some(Self::Wallpaperscraft),
            "goodfon" => Some(Self::Goodfon),
            _ => None,
        }
    }
    pub(crate) fn default_category(self) -> &'static str {
        "all"
    }
    pub(crate) fn valid_category(self, category: &str) -> bool {
        category_options(self, true)
            .iter()
            .any(|option| option["value"] == category)
    }
    pub(crate) fn urls(
        self,
        _sequence: u32,
        _round: u8,
        query: &str,
        category: &str,
        page: u32,
    ) -> Vec<String> {
        if !valid_query(query) || !self.valid_category(category) || page == 0 {
            return Vec::new();
        }
        if self == Self::Wallcoo {
            let mut url = url::Url::parse("https://www.wallcoo.com/wp-json/bc/v1/wallpapers")
                .expect("fixed website API URL");
            url.query_pairs_mut()
                .append_pair("page", &page.to_string())
                .append_pair("per_page", "24");
            if !query.is_empty() {
                url.query_pairs_mut().append_pair("search", query);
            } else if let Some(id) = match category {
                "landscape" => Some("3"),
                "painting" => Some("4"),
                "anime" => Some("5"),
                "animal" => Some("7"),
                "game" => Some("11"),
                "digital-art" => Some("5745"),
                _ => None,
            } {
                url.query_pairs_mut().append_pair("category_id", id);
            }
            return vec![url.into()];
        }
        let mut url = url::Url::parse(match self {
            Self::Bizhihui => "https://www.bizhihui.com/",
            Self::Wallcoo => unreachable!("Wallcoo uses its public JSON catalog"),
            Self::Wallpaperscraft => "https://wallpaperscraft.com/",
            Self::Goodfon => "https://www.goodfon.com/",
        })
        .expect("fixed website URL");
        if !query.is_empty() {
            match self {
                Self::Bizhihui => {
                    url.set_path("/search.php");
                    url.query_pairs_mut()
                        .append_pair("q", query)
                        .append_pair("page", &page.to_string());
                }
                Self::Wallcoo => unreachable!("Wallcoo uses its public JSON catalog"),
                Self::Wallpaperscraft => {
                    url.set_path("/search/");
                    url.query_pairs_mut()
                        .append_pair("query", query)
                        .append_pair("size", "1920x1080")
                        .append_pair("page", &page.to_string());
                }
                Self::Goodfon => {
                    url.set_path("/search/");
                    url.query_pairs_mut()
                        .append_pair("q", query)
                        .append_pair("page", &page.to_string());
                }
            }
        } else {
            match self {
                Self::Bizhihui => {
                    url.set_path(&if category == "all" {
                        if page == 1 {
                            "/".into()
                        } else {
                            format!("/page/{page}/")
                        }
                    } else if page == 1 {
                        format!("/{category}/")
                    } else {
                        format!("/{category}/{page}/")
                    });
                }
                Self::Wallcoo => unreachable!("Wallcoo uses its public JSON catalog"),
                Self::Wallpaperscraft => {
                    let section = if category == "all" {
                        "all".into()
                    } else {
                        format!("catalog/{category}")
                    };
                    url.set_path(&format!(
                        "/{section}/1920x1080{}",
                        if page == 1 {
                            String::new()
                        } else {
                            format!("/page{page}")
                        }
                    ));
                }
                Self::Goodfon => {
                    if page > 1 {
                        let section = if category == "all" {
                            String::new()
                        } else {
                            format!("{category}/")
                        };
                        url.set_path(&format!("/{section}index-{page}.html"));
                    } else if category != "all" {
                        url.set_path(&format!("/{category}/"));
                    }
                }
            }
        }
        vec![url.into()]
    }
    pub(crate) fn effect_id(self, sequence: u32, index: usize) -> String {
        format!("wallpaper-{}-{sequence}-{index}", self.id())
    }
}

pub(crate) fn public_url(source: &str) -> Result<url::Url, String> {
    if source.len() > 4096 || source.chars().any(char::is_control) {
        return Err("Invalid wallpaper source URL".into());
    }
    let url = url::Url::parse(source).map_err(|_| "Invalid wallpaper source URL")?;
    if url.scheme() != "https"
        || url.port().is_some_and(|port| port != 443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("Wallpaper sources require credential-free HTTPS on port 443".into());
    }
    let Some(url::Host::Domain(host)) = url.host() else {
        return Err("Wallpaper sources require a public DNS name".into());
    };
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
        || [
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
            "example.com",
            "example.net",
            "example.org",
        ]
        .iter()
        .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
    {
        return Err("Wallpaper sources require a public DNS name".into());
    }
    Ok(url)
}

fn digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|c| c.is_ascii_digit())
}
fn slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 240
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}
fn image_dimensions(width: u64, height: u64) -> bool {
    width > 0 && height > 0 && width <= 30_000 && height <= 30_000
}

pub(crate) fn valid_image(provider: Provider, category: &str, image: &RemoteImage) -> bool {
    if !provider.valid_category(category)
        || if provider == Provider::Wallcoo {
            (image.width, image.height) != (0, 0)
        } else {
            !image_dimensions(image.width, image.height)
        }
    {
        return false;
    }
    let (Ok(original), Ok(preview), Ok(display), Ok(source)) = (
        public_url(&image.original_url),
        public_url(&image.preview_url),
        public_url(&image.display_url),
        public_url(&image.source_url),
    ) else {
        return false;
    };
    if [&original, &preview, &display, &source]
        .iter()
        .any(|url| url.query().is_some())
    {
        return false;
    }
    match provider {
        Provider::Bizhihui => {
            let source_id = source
                .path()
                .strip_prefix("/p/")
                .and_then(|s| s.strip_suffix(".html"));
            let parts = original
                .path()
                .strip_prefix("/zb_users/upload/")
                .map(|s| s.split('/').collect::<Vec<_>>())
                .unwrap_or_default();
            source.host_str() == Some("www.bizhihui.com")
                && source_id.is_some_and(digits)
                && original.host_str() == Some("s.panlai.com")
                && preview.host_str() == Some("s.panlai.com")
                && parts.len() == 3
                && parts[0].len() == 4
                && digits(parts[0])
                && parts[1].len() == 2
                && digits(parts[1])
                && parts[2].rsplit_once('.').is_some_and(|(name, ext)| {
                    name.strip_prefix("bizhihui_com_").is_some_and(digits)
                        && matches!(ext, "jpg" | "jpeg" | "png" | "webp")
                })
                && preview.path() == format!("{}-pcthumbs", original.path())
                && display.host_str() == Some("s.panlai.com")
                && display.path() == format!("{}-arthumbs", original.path())
        }
        Provider::Wallcoo => {
            let parts = preview
                .path()
                .strip_prefix("/wp-content/uploads/wallcoo/")
                .map(|path| path.split('/').collect::<Vec<_>>())
                .unwrap_or_default();
            if parts.len() != 4 {
                return false;
            }
            let Some(name) = parts[3]
                .strip_prefix("thumb_")
                .and_then(|file| file.strip_suffix(".jpg"))
            else {
                return false;
            };
            let source_parts = source
                .path()
                .trim_matches('/')
                .split('/')
                .collect::<Vec<_>>();
            [&original, &preview, &display, &source]
                .iter()
                .all(|url| url.host_str() == Some("www.wallcoo.com"))
                && parts[0].len() == 4
                && digits(parts[0])
                && parts[1].len() == 2
                && digits(parts[1])
                && parts[2].strip_prefix('w').is_some_and(digits)
                && slug(name)
                && source_parts.len() == 2
                && slug(source_parts[0])
                && slug(source_parts[1])
                && !matches!(source_parts[0], "wp-json" | "wp-admin" | "wp-content")
                && original.path()
                    == format!(
                        "/wp-content/uploads/wallcoo/{}/{}/{}/preview_{name}.webp",
                        parts[0], parts[1], parts[2]
                    )
                && image.display_url == image.original_url
        }
        Provider::Wallpaperscraft => {
            let Some(name) = preview
                .path()
                .strip_prefix("/image/single/")
                .and_then(|s| s.strip_suffix("_300x168.jpg"))
            else {
                return false;
            };
            source.host_str() == Some("wallpaperscraft.com")
                && original.host_str() == Some("images.wallpaperscraft.com")
                && preview.host_str() == Some("images.wallpaperscraft.com")
                && slug(name)
                && name.rsplit_once('_').is_some_and(|(_, id)| digits(id))
                && (source.path() == format!("/download/{name}/1920x1080")
                    || source.path() == format!("/wallpaper/{name}"))
                && original.path() == format!("/image/single/{name}_1920x1080.jpg")
                && image.display_url == image.original_url
                && (image.width, image.height) == (1920, 1080)
        }
        Provider::Goodfon => {
            let parts = preview
                .path()
                .strip_prefix("/wallpaper/big/")
                .map(|s| s.split('/').collect::<Vec<_>>())
                .unwrap_or_default();
            if parts.len() != 3 {
                return false;
            }
            let Some(name) = parts[2].strip_suffix(".webp") else {
                return false;
            };
            let source_parts = source
                .path()
                .trim_start_matches('/')
                .split('/')
                .collect::<Vec<_>>();
            source.host_str() == Some("www.goodfon.com")
                && original.host_str() == Some("img.goodfon.com")
                && preview.host_str() == Some("img.goodfon.com")
                && parts[0].len() == 1
                && parts[1].len() == 2
                && parts[..2]
                    .iter()
                    .all(|part| part.bytes().all(|c| c.is_ascii_hexdigit()))
                && slug(name)
                && source_parts.len() == 2
                && slug(source_parts[0])
                && source_parts[1] == format!("wallpaper-{name}.html")
                && original.path()
                    == format!(
                        "/original/{}x{}/{}/{}/{name}.jpg",
                        image.width, image.height, parts[0], parts[1]
                    )
                && display.host_str() == Some("img.goodfon.com")
                && display.path()
                    == format!("/wallpaper/nbig/{}/{}/{name}.webp", parts[0], parts[1])
        }
    }
}

// 只读四个已核验的列表容器；小型 tokenizer 处理引号中的 >、无引号属性、
// 注释及脚本，避免把广告、推荐栏或脚本字符串当作壁纸。此处不执行网页代码。
struct Tag<'a> {
    name: &'a str,
    attributes: &'a str,
    start: usize,
    end: usize,
    closing: bool,
    single: bool,
}
struct Tags<'a> {
    html: &'a str,
    offset: usize,
}
impl<'a> Tags<'a> {
    fn new(html: &'a str) -> Self {
        Self { html, offset: 0 }
    }
}
impl<'a> Iterator for Tags<'a> {
    type Item = Tag<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let start = self.offset + self.html[self.offset..].find('<')?;
        if self.html[start..].starts_with("<!--") {
            let end = start + self.html[start..].find("-->")? + 3;
            self.offset = end;
            return Some(Tag {
                name: "",
                attributes: "",
                start,
                end,
                closing: false,
                single: true,
            });
        }
        let mut quote = None;
        let mut value_start = false;
        let mut end = None;
        for (index, byte) in self.html.as_bytes().iter().enumerate().skip(start + 1) {
            match (quote, *byte) {
                (Some(expected), found) if expected == found => quote = None,
                (Some(_), _) => {}
                (None, b'=') => value_start = true,
                (None, b'\'' | b'"') if value_start => {
                    quote = Some(*byte);
                    value_start = false;
                }
                (None, b'>') => {
                    end = Some(index + 1);
                    break;
                }
                (None, found) if !found.is_ascii_whitespace() => value_start = false,
                _ => {}
            }
        }
        let header_end = end?;
        let mut inner = self.html[start + 1..header_end - 1].trim();
        let closing = inner.starts_with('/');
        if closing {
            inner = inner[1..].trim_start();
        }
        let name_end = inner
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
            .unwrap_or(inner.len());
        let name = &inner[..name_end];
        let single = inner.ends_with('/')
            || matches!(
                name,
                "img" | "source" | "br" | "input" | "meta" | "link" | "hr"
            );
        let mut end = header_end;
        if !closing && matches!(name, "script" | "style") {
            let tail = &self.html[header_end..];
            if let Some(close) = tail.find(&format!("</{name}")) {
                end = header_end + close + tail[close..].find('>')? + 1;
            } else {
                end = self.html.len();
            }
        }
        self.offset = end;
        Some(Tag {
            name,
            attributes: &inner[name_end..],
            start,
            end,
            closing,
            single,
        })
    }
}
impl Tag<'_> {
    fn attr(&self, wanted: &str) -> Option<String> {
        let mut rest = self.attributes;
        while !rest.is_empty() {
            rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '/');
            let length = rest
                .find(|c: char| c.is_ascii_whitespace() || c == '=')
                .unwrap_or(rest.len());
            if length == 0 {
                break;
            }
            let key = &rest[..length];
            rest = rest[length..].trim_start();
            if !rest.starts_with('=') {
                continue;
            }
            rest = rest[1..].trim_start();
            let (value, remaining) = if rest.starts_with(['\'', '"']) {
                let quote = rest.as_bytes()[0] as char;
                let end = rest[1..].find(quote)? + 1;
                (&rest[1..end], &rest[end + 1..])
            } else {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                (&rest[..end], &rest[end..])
            };
            if key.eq_ignore_ascii_case(wanted) {
                return Some(decode_entities(value));
            }
            rest = remaining;
        }
        None
    }
    fn class(&self, wanted: &str) -> bool {
        self.attr("class")
            .is_some_and(|value| value.split_ascii_whitespace().any(|class| class == wanted))
    }
}
fn decode_entities(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('&') {
        result.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find(';').filter(|end| *end <= 12) else {
            result.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| {
                    entity
                        .strip_prefix('#')
                        .and_then(|number| number.parse().ok())
                })
                .and_then(char::from_u32),
        };
        if let Some(decoded) = decoded {
            result.push(decoded);
        } else {
            result.push_str(&rest[..=end]);
        }
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    result
}
fn text_content(html: &str) -> String {
    let mut result = String::new();
    let mut offset = 0;
    for tag in Tags::new(html) {
        result.push_str(&html[offset..tag.start]);
        if matches!(tag.name, "br" | "p" | "div") {
            result.push(' ');
        }
        offset = tag.end;
    }
    result.push_str(&html[offset..]);
    decode_entities(&result)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn element_inner<'a>(html: &'a str, name: &str, attribute: &str, value: &str) -> Option<&'a str> {
    let mut depth = 0;
    let mut start = 0;
    for tag in Tags::new(html) {
        if tag.name != name {
            continue;
        }
        if depth == 0 {
            let matches = if attribute == "class" {
                tag.class(value)
            } else {
                tag.attr(attribute).as_deref() == Some(value)
            };
            if !tag.closing && matches {
                start = tag.end;
                depth = 1;
            }
        } else if tag.closing {
            depth -= 1;
            if depth == 0 {
                return Some(&html[start..tag.start]);
            }
        } else if !tag.single {
            depth += 1;
        }
    }
    None
}
fn dimension_pair(value: &str) -> Option<(u64, u64)> {
    let compact = text_content(value).split_whitespace().collect::<String>();
    let (width, height) = compact
        .split_once('x')
        .or_else(|| compact.split_once('×'))?;
    let dimensions = (width.parse().ok()?, height.parse().ok()?);
    image_dimensions(dimensions.0, dimensions.1).then_some(dimensions)
}
fn join_url(provider: Provider, value: &str) -> Option<String> {
    let base = provider.urls(0, 0, "", "all", 1).pop()?;
    let url = url::Url::parse(&base).ok()?.join(value).ok()?;
    public_url(url.as_str()).ok()?;
    Some(url.into())
}
fn card_image(provider: Provider, body: &str) -> Option<RemoteImage> {
    let (link, img) = match provider {
        Provider::Bizhihui => (
            Tags::new(body).find(|tag| tag.name == "a" && tag.class("item-img"))?,
            Tags::new(body).find(|tag| tag.name == "img")?,
        ),
        Provider::Wallcoo => return None,
        Provider::Wallpaperscraft => (
            Tags::new(body).find(|tag| tag.name == "a" && tag.class("wallpapers__link"))?,
            Tags::new(body).find(|tag| tag.name == "img" && tag.class("wallpapers__image"))?,
        ),
        Provider::Goodfon => (
            Tags::new(body)
                .find(|tag| tag.name == "a" && tag.attr("itemprop").as_deref() == Some("url"))?,
            Tags::new(body).find(|tag| tag.name == "img" && tag.class("wallpapers__item__img"))?,
        ),
    };
    let source_url = join_url(provider, &link.attr("href")?)?;
    let preview_url = join_url(provider, &img.attr("src")?)?;
    let title = link.attr("title").or_else(|| img.attr("alt"))?;
    let (width, height, original_url) = match provider {
        Provider::Bizhihui => {
            let (width, height) =
                dimension_pair(element_inner(body, "span", "class", "float-lt")?)?;
            (
                width,
                height,
                preview_url.strip_suffix("-pcthumbs")?.to_owned(),
            )
        }
        Provider::Wallcoo => return None,
        Provider::Wallpaperscraft => (
            1920,
            1080,
            preview_url.strip_suffix("_300x168.jpg")?.to_owned() + "_1920x1080.jpg",
        ),
        Provider::Goodfon => {
            let start = Tags::new(body)
                .find(|tag| tag.name == "small" && !tag.closing)?
                .end;
            let end = body[start..].find("</small>")? + start;
            let (width, height) = dimension_pair(&body[start..end])?;
            let path = preview_url
                .strip_prefix("https://img.goodfon.com/wallpaper/big/")?
                .strip_suffix(".webp")?;
            (
                width,
                height,
                format!("https://img.goodfon.com/original/{width}x{height}/{path}.jpg"),
            )
        }
    };
    let display_url = match provider {
        Provider::Bizhihui => format!("{original_url}-arthumbs"),
        Provider::Goodfon => preview_url.replace("/wallpaper/big/", "/wallpaper/nbig/"),
        Provider::Wallcoo | Provider::Wallpaperscraft => original_url.clone(),
    };
    Some(RemoteImage {
        result_id: String::new(),
        title: plain(&text_content(&title), 120),
        source_url,
        width,
        height,
        original_url,
        preview_url,
        display_url,
    })
}

#[derive(Deserialize)]
struct WallcooListing {
    items: Vec<WallcooItem>,
    pagination: WallcooPagination,
}
#[derive(Deserialize)]
struct WallcooPagination {
    current_page: u64,
    per_page: u64,
    total_pages: u64,
    total_items: u64,
    has_more: bool,
}
#[derive(Deserialize)]
struct WallcooItem {
    id: u64,
    title: String,
    link: String,
    url: String,
    #[serde(rename = "largeUrl")]
    large_url: String,
}
fn parse_wallcoo(category: &str, body: &str) -> Result<Vec<RemoteImage>, String> {
    let listing: WallcooListing =
        serde_json::from_str(body).map_err(|_| "Invalid Wallcoo catalog JSON")?;
    let pagination = &listing.pagination;
    if pagination.current_page == 0
        || pagination.per_page != 24
        || listing.items.len() > 100
        || listing.items.len() as u64 > pagination.per_page
        || pagination.total_items < listing.items.len() as u64
        || pagination.has_more != (pagination.current_page < pagination.total_pages)
    {
        return Err("Invalid Wallcoo pagination".into());
    }
    let empty = listing.items.is_empty();
    let mut images = Vec::new();
    let mut seen = BTreeSet::new();
    for item in listing.items {
        if item.id == 0 {
            continue;
        }
        // stats.dimensions 描述下载原图；此处只使用公开适配图，不能冒认原图尺寸。
        let image = RemoteImage {
            result_id: String::new(),
            title: plain(&text_content(&item.title), 120),
            source_url: item.link,
            width: 0,
            height: 0,
            original_url: item.large_url.clone(),
            preview_url: item.url,
            display_url: item.large_url,
        };
        if !image.title.is_empty()
            && valid_image(Provider::Wallcoo, category, &image)
            && seen.insert(image.original_url.clone())
            && images.len() < IMAGE_COUNT
        {
            images.push(image);
        }
    }
    if !empty && images.is_empty() {
        return Err("No supported Wallcoo images in catalog response".into());
    }
    Ok(images)
}

// 仅在 parse 成功之后消费；其余站点的 HTML 没有等价的已核验分页元数据。
pub(crate) fn has_more(provider: Provider, body: &str) -> Option<bool> {
    if provider != Provider::Wallcoo || body.len() > RESPONSE_LIMIT {
        return None;
    }
    serde_json::from_str::<WallcooListing>(body)
        .ok()
        .map(|listing| listing.pagination.has_more)
}

pub(crate) fn page_matches(provider: Provider, page: u32, body: &str) -> bool {
    if provider != Provider::Wallcoo {
        return true;
    }
    if body.len() > RESPONSE_LIMIT {
        return false;
    }
    serde_json::from_str::<WallcooListing>(body).is_ok_and(|listing| {
        listing.pagination.current_page == u64::from(page) && listing.pagination.per_page == 24
    })
}

pub(crate) fn parse(
    provider: Provider,
    category: &str,
    query: &str,
    body: &str,
) -> Result<Vec<RemoteImage>, String> {
    if body.len() > RESPONSE_LIMIT {
        return Err("Wallpaper response exceeds its limit".into());
    }
    if !provider.valid_category(category) || !valid_query(query) {
        return Err("Invalid wallpaper category".into());
    }
    if provider == Provider::Wallcoo {
        return parse_wallcoo(category, body);
    }
    let (name, attribute, value, brand, row_name, row_class) = match provider {
        Provider::Bizhihui => ("ul", "id", "item-lists", "壁纸汇", "li", "item-list"),
        Provider::Wallcoo => unreachable!("Wallcoo uses its public JSON catalog"),
        Provider::Wallpaperscraft => (
            "ul",
            "class",
            "wallpapers__list",
            "wallpaperscraft",
            "li",
            "wallpapers__item",
        ),
        Provider::Goodfon => (
            "div",
            "class",
            "wallpapers",
            "goodfon",
            "div",
            "wallpapers__item",
        ),
    };
    if !body.to_lowercase().contains(&brand.to_lowercase()) {
        return Err("Unrecognized wallpaper website response".into());
    }
    let list = element_inner(body, name, attribute, value)
        .ok_or("Wallpaper website layout changed or request was blocked")?;
    let starts = Tags::new(list)
        .filter(|tag| {
            !tag.closing && tag.name == row_name && (row_class.is_empty() || tag.class(row_class))
        })
        .map(|tag| tag.start)
        .collect::<Vec<_>>();
    if starts.len() > 100 {
        return Err("Invalid wallpaper list size".into());
    }
    if starts.is_empty() {
        if Tags::new(list).any(|tag| matches!(tag.name, "img" | "a"))
            || !text_content(list).is_empty()
        {
            return Err("Wallpaper website layout changed".into());
        }
        return Ok(Vec::new());
    }
    let search =
        (provider == Provider::Bizhihui && !query.is_empty()).then(|| query.trim().to_lowercase());
    let mut images = Vec::new();
    let mut seen = BTreeSet::new();
    let mut recognized = false;
    for (index, start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(list.len());
        let card = &list[*start..end];
        if let Some(image) = card_image(provider, card)
            && valid_image(provider, category, &image)
        {
            recognized = true;
            // 部分站点无匹配仍回传推荐，不把这些卡片显示成关键词命中。
            if let Some(query) = &search {
                // 搜索上游完整标题，展示截断不能丢掉位于长标题末尾的命中。
                let title = Tags::new(card)
                    .find(|tag| tag.name == "a" && tag.class("item-img"))
                    .and_then(|tag| tag.attr("title"))
                    .unwrap_or_else(|| image.title.clone());
                if !text_content(&title).to_lowercase().contains(query) {
                    continue;
                }
            }
            if seen.insert(image.original_url.clone()) && images.len() < IMAGE_COUNT {
                images.push(image);
            }
        }
    }
    if !recognized {
        return Err("No supported wallpaper images in website response".into());
    }
    Ok(images)
}

#[cfg(test)]
pub(crate) mod fixtures {
    // 2026-10-31 真实列表中裁下的卡片；保留上游引号、空白、嵌套和尺寸语义。
    pub(crate) const BIZHIHUI: &str = r#"<title>免费壁纸 - 壁纸汇</title><ul id="item-lists" class="clear">
<li class="grid-sizer"></li><li class="item-list masonry-brick"><div class="item-content item-content-9">
<a class="item-img" href="https://www.bizhihui.com/p/24385.html" title="静物 树叶 书籍 圣杯 水果 棕色 4K壁纸"><img src="https://s.panlai.com/zb_users/upload/2026/10/bizhihui_com_202610291793244133809052.jpg-pcthumbs" alt="静物 树叶 书籍 圣杯 水果 棕色 4K壁纸" width="450" height="285"></a>
<div class="item-info2"><h3><a href="https://www.bizhihui.com/p/24385.html"><p class="clear"><span class="float-lt"><i class="fa fa-photo" aria-hidden="true" /></i> 5120 x 3268</span><span class="float-rt">652</span></p></a></h3></div></div></li></ul>"#;
    pub(crate) const WALLCOO: &str = r#"{"items":[{"id":260612,"title":"英格兰埃夫伯里巨石群晨雾草原UHD壁纸","link":"https://www.wallcoo.com/landscape/avebury-stones-at-dawn-4k-wallpaper/","url":"https://www.wallcoo.com/wp-content/uploads/wallcoo/2026/10/w5/thumb_th-cqewj8.jpg","largeUrl":"https://www.wallcoo.com/wp-content/uploads/wallcoo/2026/10/w5/preview_th-cqewj8.webp","user":{"name":"Mia"},"stats":{"dimensions":"3840 × 2160","copyright":"© George W Johnson/Getty Images"}}],"pagination":{"current_page":1,"per_page":24,"total_pages":2574,"total_items":61772,"has_more":true}}"#;
    pub(crate) const WALLPAPERSCRAFT: &str = r#"<title>Desktop wallpapers - WallpapersCraft</title><ul class="wallpapers__list"><li class="wallpapers__item" style="width: 300px"><a class="wallpapers__link" href="/download/mountains_spruce_art_167976/1920x1080"><span class="wallpapers__canvas"><img class="wallpapers__image" src="https://images.wallpaperscraft.com/image/single/mountains_spruce_art_167976_300x168.jpg" alt="Preview wallpaper mountains, spruce, art"></span><span class="wallpapers__info"><span class="wallpapers__info-rating"><span class="gui-icon gui-icon_rating"></span>&nbsp;6.7</span>1920x1080<span class="wallpapers__info-downloads">9253</span></span><span class="wallpapers__info">mountains, spruce, art</span></a></li></ul>"#;
    pub(crate) const GOODFON: &str = r#"<title>Wallpaper search: mountain</title><a href=https://www.goodfon.com/>GoodFon</a><div class=wallpapers><div class=wallpapers__item><div class=wallpapers__item__wall itemscope itemtype=http://schema.org/ImageObject><a title="Ice, mountain, snow" href=https://www.goodfon.com/landscapes/wallpaper-ice-snow-mountain.html itemprop=url><picture><source srcset="https://img.goodfon.com/wallpaper/big/c/9f/ice-snow-mountain.avif 596w" type=image/avif><img alt="Ice, mountain, snow" class=wallpapers__item__img height=380 itemprop=thumbnail src=https://img.goodfon.com/wallpaper/big/c/9f/ice-snow-mountain.webp width=596></picture></a><div class=wallpapers__item__bottom><a class=wallpapers__item__user_block href=https://www.goodfon.com/user/333arman/><img src=/static/anon.svg></a><div class=wallpapers__item__size><div><span>2K</span></div><div><small>2880x1920</small></div></div></div></div></div></div>"#;
}

#[cfg(test)]
mod provider_tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "manual upstream HTML contract check; requires network access and curl"]
    fn live_website_listings_keep_search_category_and_pagination_contracts() {
        let selected = std::env::var("WALLPAPER_TEST_PROVIDER").ok();
        for (id, category, query) in [
            ("wallpaperscraft", "nature", "mountain"),
            ("bizhihui", "fengjing", "风景"),
            ("wallcoo", "landscape", "山"),
            ("goodfon", "landscapes", "mountain"),
        ] {
            if selected.as_deref().is_some_and(|selected| selected != id) {
                continue;
            }
            let provider = provider(id);
            for (query, category, page) in [
                ("", "all", 1),
                ("", "all", 2),
                (query, "all", 1),
                ("", category, 2),
            ] {
                let url = provider.urls(0, 0, query, category, page).pop().unwrap();
                let response =
                    std::process::Command::new(if cfg!(windows) { "curl.exe" } else { "curl" })
                        .args([
                            "--fail",
                            "--silent",
                            "--show-error",
                            "--compressed",
                            "--max-time",
                            "20",
                            "--max-filesize",
                            "2097152",
                            &url,
                        ])
                        .output()
                        .expect("curl must be installed for the manual live check");
                assert!(
                    response.status.success(),
                    "{url}: {}",
                    String::from_utf8_lossy(&response.stderr)
                );
                let body = String::from_utf8(response.stdout).unwrap_or_else(|error| {
                    panic!(
                        "{url}: invalid UTF-8 at {}",
                        error.utf8_error().valid_up_to()
                    )
                });
                let images = parse(provider, category, query, &body)
                    .unwrap_or_else(|error| panic!("{url}: {error}"));
                assert!(!images.is_empty(), "{url} returned no images");
                eprintln!(
                    "{url}: {} verified images; first HD {}",
                    images.len(),
                    images[0].original_url
                );
            }
        }
    }

    fn provider(id: &str) -> Provider {
        Provider::from_id(id).unwrap_or_else(|| panic!("Missing replacement source {id}"))
    }

    #[test]
    fn wallcoo_public_catalog_preserves_adapted_images_without_original_dimensions() {
        let p = provider("wallcoo");
        let images = parse(p, "all", "", fixtures::WALLCOO).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(
            images[0].original_url,
            "https://www.wallcoo.com/wp-content/uploads/wallcoo/2026/10/w5/preview_th-cqewj8.webp"
        );
        assert_eq!(images[0].display_url, images[0].original_url);
        assert_eq!((images[0].width, images[0].height), (0, 0));
        assert!(valid_image(p, "all", &images[0]));
        let mut wrong_dimensions = images[0].clone();
        wrong_dimensions.width = 3840;
        wrong_dimensions.height = 2160;
        assert!(!valid_image(p, "all", &wrong_dimensions));
        let mut wrong_identity = images[0].clone();
        wrong_identity.preview_url = wrong_identity.preview_url.replace("th-cqewj8", "th-other");
        assert!(!valid_image(p, "all", &wrong_identity));
    }

    #[test]
    fn wallcoo_catalog_validates_typed_pagination_and_requested_page() {
        let p = provider("wallcoo");
        assert!(page_matches(p, 1, fixtures::WALLCOO));
        assert!(!page_matches(p, 2, fixtures::WALLCOO));
        let mut response: Value = serde_json::from_str(fixtures::WALLCOO).unwrap();
        response["pagination"]["per_page"] = 23.into();
        assert!(parse(p, "all", "", &response.to_string()).is_err());
        response["pagination"]["per_page"] = 24.into();
        response["items"] = serde_json::json!([]);
        response["pagination"]["total_pages"] = 0.into();
        response["pagination"]["total_items"] = 0.into();
        response["pagination"]["has_more"] = false.into();
        assert!(
            parse(p, "all", "", &response.to_string())
                .unwrap()
                .is_empty()
        );
        assert_eq!(has_more(p, &response.to_string()), Some(false));
        response["pagination"]["has_more"] = "false".into();
        assert!(parse(p, "all", "", &response.to_string()).is_err());
        assert!(parse(p, "all", "", r#"{"items":[]}"#).is_err());
    }

    #[test]
    fn replacement_source_search_requests_preserve_query_and_real_page() {
        let expected = [
            (
                "bizhihui",
                "https://www.bizhihui.com/search.php?q=mountain+lake&page=2",
            ),
            (
                "wallcoo",
                "https://www.wallcoo.com/wp-json/bc/v1/wallpapers?page=2&per_page=24&search=mountain+lake",
            ),
            (
                "wallpaperscraft",
                "https://wallpaperscraft.com/search/?query=mountain+lake&size=1920x1080&page=2",
            ),
            (
                "goodfon",
                "https://www.goodfon.com/search/?q=mountain+lake&page=2",
            ),
        ];
        for (id, url) in expected {
            assert_eq!(provider(id).urls(1, 0, "mountain lake", "all", 2), [url]);
        }
        assert!(Provider::from_id("upx8").is_none());
        assert!(Provider::from_id("wallhaven").is_none());
        assert!(Provider::from_id("commons").is_none());
        assert!(Provider::from_id("nekosbest").is_none());
    }

    #[test]
    fn real_listing_cards_discover_high_resolution_images_without_detail_requests() {
        for (id, body, width, height, original) in [
            (
                "bizhihui",
                fixtures::BIZHIHUI,
                5120,
                3268,
                "https://s.panlai.com/zb_users/upload/2026/10/bizhihui_com_202610291793244133809052.jpg",
            ),
            (
                "wallcoo",
                fixtures::WALLCOO,
                0,
                0,
                "https://www.wallcoo.com/wp-content/uploads/wallcoo/2026/10/w5/preview_th-cqewj8.webp",
            ),
            (
                "wallpaperscraft",
                fixtures::WALLPAPERSCRAFT,
                1920,
                1080,
                "https://images.wallpaperscraft.com/image/single/mountains_spruce_art_167976_1920x1080.jpg",
            ),
            (
                "goodfon",
                fixtures::GOODFON,
                2880,
                1920,
                "https://img.goodfon.com/original/2880x1920/c/9f/ice-snow-mountain.jpg",
            ),
        ] {
            let p = provider(id);
            let images = parse(p, "all", "", body).expect(id);
            assert_eq!(images.len(), 1, "{id}");
            assert_eq!((images[0].width, images[0].height), (width, height), "{id}");
            assert_eq!(images[0].original_url, original, "{id}");
            assert!(valid_image(p, "all", &images[0]), "{id}");
            let mut mismatch = images[0].clone();
            mismatch.original_url = original
                .replace("mountain", "unrelated")
                .replace("243", "999")
                .replace("20261029", "19990101");
            if mismatch.original_url == original {
                mismatch.preview_url.push_str("?untrusted=1");
            }
            assert!(!valid_image(p, "all", &mismatch), "{id} identity mismatch");
        }
    }

    #[test]
    fn full_previews_use_the_sites_bounded_detail_renditions() {
        for (id, body, display) in [
            (
                "bizhihui",
                fixtures::BIZHIHUI,
                "https://s.panlai.com/zb_users/upload/2026/10/bizhihui_com_202610291793244133809052.jpg-arthumbs",
            ),
            (
                "wallcoo",
                fixtures::WALLCOO,
                "https://www.wallcoo.com/wp-content/uploads/wallcoo/2026/10/w5/preview_th-cqewj8.webp",
            ),
            (
                "wallpaperscraft",
                fixtures::WALLPAPERSCRAFT,
                "https://images.wallpaperscraft.com/image/single/mountains_spruce_art_167976_1920x1080.jpg",
            ),
            (
                "goodfon",
                fixtures::GOODFON,
                "https://img.goodfon.com/wallpaper/nbig/c/9f/ice-snow-mountain.webp",
            ),
        ] {
            let images = parse(provider(id), "all", "", body).unwrap();
            let json = serde_json::to_value(&images[0]).unwrap();
            assert_eq!(json["displayUrl"], display, "{id}");
            let mut forged = json;
            forged["displayUrl"] = "https://attacker.com/unrelated.jpg".into();
            let forged: RemoteImage = serde_json::from_value(forged).unwrap();
            assert!(!valid_image(provider(id), "all", &forged));
        }
    }

    #[test]
    fn category_browsing_uses_verified_site_pagination() {
        for (id, category, url) in [
            (
                "bizhihui",
                "fengjing",
                "https://www.bizhihui.com/fengjing/2/",
            ),
            (
                "wallcoo",
                "landscape",
                "https://www.wallcoo.com/wp-json/bc/v1/wallpapers?page=2&per_page=24&category_id=3",
            ),
            (
                "wallpaperscraft",
                "nature",
                "https://wallpaperscraft.com/catalog/nature/1920x1080/page2",
            ),
            (
                "goodfon",
                "landscapes",
                "https://www.goodfon.com/landscapes/index-2.html",
            ),
        ] {
            let p = provider(id);
            assert!(p.valid_category(category));
            assert_eq!(p.urls(1, 0, "", category, 2), [url]);
            assert!(p.urls(1, 0, "", "../../private", 1).is_empty());
        }
    }

    #[test]
    fn blocks_challenges_layout_changes_and_unrelated_cards() {
        for (id, fixture) in [
            ("bizhihui", fixtures::BIZHIHUI),
            ("wallcoo", fixtures::WALLCOO),
            ("wallpaperscraft", fixtures::WALLPAPERSCRAFT),
            ("goodfon", fixtures::GOODFON),
        ] {
            let p = provider(id);
            assert!(
                parse(
                    p,
                    "all",
                    "",
                    "<html><title>Just a moment...</title>Verify you are human</html>"
                )
                .is_err()
            );
            assert!(parse(p, "all", "", "<html>Unknown site layout</html>").is_err());
            let wrong_host = fixture
                .replace("s.panlai.com", "attacker.com")
                .replace("src=\"/wallpaper", "src=\"https://attacker.com/wallpaper")
                .replace("images.wallpaperscraft.com", "attacker.com")
                .replace("img.goodfon.com", "attacker.com")
                .replace("www.wallcoo.com", "attacker.com");
            assert!(parse(p, "all", "", &wrong_host).is_err(), "{id}");
        }
    }

    #[test]
    fn html_entities_and_search_highlights_are_plain_text() {
        let body = fixtures::BIZHIHUI.replace(
            "静物 树叶 书籍 圣杯 水果 棕色 4K壁纸",
            "湖泊 &amp; <strong>风景</strong> &#x1F30A;",
        );
        let images = parse(provider("bizhihui"), "all", "", &body).unwrap();
        assert_eq!(images[0].title, "湖泊 & 风景 🌊");
    }

    #[test]
    fn empty_search_does_not_turn_recommendations_into_matches() {
        let body = format!(
            "<div class=op-title>搜索&nbsp;&quot;不存在的词&quot; 的结果</div>{}",
            fixtures::BIZHIHUI
        );
        assert!(
            parse(provider("bizhihui"), "all", "不存在的词", &body)
                .unwrap()
                .is_empty()
        );
        assert!(
            parse(
                provider("goodfon"),
                "all",
                "",
                "<title>Wallpaper search</title><a>GoodFon</a><div class=wallpapers></div>"
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn requested_search_query_filters_bizhihui_recommendations_without_server_heading() {
        assert!(
            parse(
                provider("bizhihui"),
                "all",
                "不存在的词",
                fixtures::BIZHIHUI
            )
            .unwrap()
            .is_empty()
        );
        let matched = parse(provider("bizhihui"), "all", "树叶", fixtures::BIZHIHUI).unwrap();
        assert_eq!(matched.len(), 1);
        let long_title = fixtures::BIZHIHUI.replace(
            "静物 树叶 书籍 圣杯 水果 棕色 4K壁纸",
            &format!("{} 树叶", "长".repeat(130)),
        );
        assert_eq!(
            parse(provider("bizhihui"), "all", "树叶", &long_title)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn malformed_upstream_ad_attribute_does_not_swallow_the_wallpaper_list() {
        // WallpapersCraft 当前广告有一个多余引号；浏览器把它当作属性名的一部分。
        let body = format!(
            "<ins class=\"adsbygoogle\" data-ad-slot=\"1784869273\"\" data-ad-format=\"auto\"></ins>{}",
            fixtures::WALLPAPERSCRAFT
        );
        assert_eq!(
            parse(provider("wallpaperscraft"), "all", "", &body)
                .unwrap()
                .len(),
            1
        );
    }
}
