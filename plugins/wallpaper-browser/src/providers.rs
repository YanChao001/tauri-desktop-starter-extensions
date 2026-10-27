//! 供应商 JSON 和固定图片身份归插件核验；实际 DNS、下载与解码边界由宿主再次检查。
use super::{RemoteImage, plain};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) const IMAGE_COUNT: usize = 24;
pub(crate) const MAX_ROUND: u8 = 2;
const RESPONSE_LIMIT: usize = 2 * 1024 * 1024;
const UPX8_CATEGORIES: [&str; 9] = [
    "nature", "anime", "city", "animal", "space", "game", "car", "abstract", "sport",
];

pub(crate) fn valid_query(query: &str) -> bool {
    query.len() <= 400 && query.chars().count() <= 100 && !query.chars().any(char::is_control)
}

pub(crate) fn category_options(provider: Provider, english: bool) -> Vec<Value> {
    let choices: &[(&str, &str, &str)] = match provider {
        Provider::Upx8 => &[
            ("all", "全部随机", "All categories"),
            ("nature", "风景", "Nature"),
            ("anime", "动漫", "Anime"),
            ("game", "游戏", "Games"),
            ("animal", "动物", "Animals"),
            ("city", "城市", "Cities"),
            ("abstract", "抽象", "Abstract"),
            ("space", "宇宙", "Space"),
            ("car", "汽车", "Cars"),
            ("girl", "人物", "Portraits"),
            ("sport", "运动", "Sports"),
        ],
        Provider::Wallhaven => &[
            ("all", "全部分类", "All categories"),
            ("general", "综合", "General"),
            ("anime", "动漫", "Anime"),
            ("people", "人物", "People"),
        ],
        Provider::Commons => &[
            ("all", "全部主题", "All themes"),
            ("landscape", "风景", "Landscape"),
            ("mountain", "山川", "Mountains"),
            ("ocean", "海洋", "Ocean"),
            ("forest", "森林", "Forest"),
            ("city", "城市", "City"),
            ("space", "太空", "Space"),
        ],
        Provider::Nekosbest => &[
            ("neko", "猫耳", "Neko"),
            ("kitsune", "狐耳", "Kitsune"),
            ("waifu", "女性角色", "Waifu"),
            ("husbando", "男性角色", "Husbando"),
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
    Upx8,
    Wallhaven,
    Commons,
    Nekosbest,
}
impl Provider {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Upx8 => "upx8",
            Self::Wallhaven => "wallhaven",
            Self::Commons => "commons",
            Self::Nekosbest => "nekosbest",
        }
    }
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Upx8 => "UPX8",
            Self::Wallhaven => "Wallhaven",
            Self::Commons => "Wikimedia Commons",
            Self::Nekosbest => "Nekos.best",
        }
    }
    pub(crate) fn from_id(value: &str) -> Option<Self> {
        match value {
            "upx8" => Some(Self::Upx8),
            "wallhaven" => Some(Self::Wallhaven),
            "commons" => Some(Self::Commons),
            "nekosbest" => Some(Self::Nekosbest),
            _ => None,
        }
    }
    pub(crate) fn default_category(self) -> &'static str {
        if self == Self::Nekosbest {
            "neko"
        } else {
            "all"
        }
    }
    pub(crate) fn valid_category(self, category: &str) -> bool {
        category_options(self, true)
            .iter()
            .any(|option| option["value"] == category)
    }
    pub(crate) fn urls(
        self,
        sequence: u32,
        round: u8,
        query: &str,
        category: &str,
        page: u32,
    ) -> Vec<String> {
        let index = page.saturating_sub(1) as usize;
        match self {
            Self::Upx8 => (0..3)
                .map(|offset| {
                    let request = usize::from(round) * 3 + offset;
                    let mut url =
                        url::Url::parse("https://wp.upx8.com/api.php").expect("fixed source URL");
                    url.query_pairs_mut()
                        .extend_pairs([("format", "json"), ("count", "10")]);
                    if query.is_empty() {
                        let category = if category == "all" {
                            UPX8_CATEGORIES[(index * 3 + request) % UPX8_CATEGORIES.len()]
                        } else {
                            category
                        };
                        url.query_pairs_mut().append_pair("category", category);
                    } else {
                        url.query_pairs_mut().append_pair("content", query);
                    }
                    url.query_pairs_mut()
                        .append_pair("_", &format!("{sequence}-{request}"));
                    url.into()
                })
                .collect(),
            Self::Wallhaven => {
                let category = match category {
                    "general" => "100",
                    "anime" => "010",
                    "people" => "001",
                    _ => "111",
                };
                let mut url = url::Url::parse("https://wallhaven.cc/api/v1/search")
                    .expect("fixed source URL");
                url.query_pairs_mut().extend_pairs([
                    ("categories", category),
                    ("purity", "100"),
                    (
                        "sorting",
                        if query.is_empty() {
                            "random"
                        } else {
                            "relevance"
                        },
                    ),
                    ("atleast", "1920x1080"),
                ]);
                if !query.is_empty() {
                    url.query_pairs_mut().append_pair("q", query);
                }
                if page > 1 {
                    url.query_pairs_mut().append_pair("page", &page.to_string());
                }
                vec![url.into()]
            }
            Self::Commons => {
                let topic = if category == "all" {
                    if query.is_empty() { "landscape" } else { "" }
                } else {
                    category
                };
                let search = [topic, query, "filetype:bitmap"]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                let mut url = url::Url::parse("https://commons.wikimedia.org/w/api.php")
                    .expect("fixed source URL");
                url.query_pairs_mut().extend_pairs([
                    ("action", "query"),
                    ("generator", "search"),
                    ("gsrsearch", search.as_str()),
                    ("gsrnamespace", "6"),
                    ("gsrlimit", "50"),
                    ("gsroffset", &(index * 50).to_string()),
                    ("prop", "imageinfo"),
                    ("iiprop", "url|size|mime"),
                    ("iiurlwidth", "640"),
                    ("format", "json"),
                    ("formatversion", "2"),
                ]);
                vec![url.into()]
            }
            Self::Nekosbest => (0..2)
                .map(|offset| {
                    let mut url = url::Url::parse(&format!(
                        "https://nekos.best/api/v2/{}",
                        if query.is_empty() { category } else { "search" }
                    ))
                    .expect("fixed source URL");
                    if !query.is_empty() {
                        url.query_pairs_mut().extend_pairs([
                            ("query", query),
                            ("type", "1"),
                            ("category", category),
                        ]);
                    }
                    url.query_pairs_mut().extend_pairs([
                        ("amount", "20"),
                        (
                            "_",
                            &format!("{sequence}-{}", usize::from(round) * 3 + offset),
                        ),
                    ]);
                    url.into()
                })
                .collect(),
        }
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

fn upx8_image_url(source: &str) -> Result<String, String> {
    let mut url = url::Url::parse(source).map_err(|_| "Invalid UPX8 image URL")?;
    if url.scheme() == "http"
        && matches!(
            url.host_str(),
            Some(
                "cdn-hsyq-static.shanhutech.cn"
                    | "cdn-hsyq-static-bak.shanhutech.cn"
                    | "img.birdpaper.com.cn"
            )
        )
    {
        url.set_scheme("https")
            .map_err(|()| "Invalid UPX8 image URL")?;
    }
    public_url(url.as_str())?;
    Ok(url.into())
}

pub(crate) fn valid_image(provider: Provider, category: &str, image: &RemoteImage) -> bool {
    let (Ok(original), Ok(preview), Ok(source)) = (
        public_url(&image.original_url),
        public_url(&image.preview_url),
        public_url(&image.source_url),
    ) else {
        return false;
    };
    match provider {
        Provider::Upx8 => {
            matches!(
                original.host_str(),
                Some(
                    "cdn-hsyq-static.shanhutech.cn"
                        | "cdn-hsyq-static-bak.shanhutech.cn"
                        | "img.birdpaper.com.cn"
                )
            ) && image.original_url == image.preview_url
                && image.source_url == "https://wp.upx8.com/"
        }
        Provider::Wallhaven => {
            let id = source.path().strip_prefix("/w/").unwrap_or("");
            source.host_str() == Some("wallhaven.cc")
                && !id.is_empty()
                && id.bytes().all(|byte| byte.is_ascii_alphanumeric())
                && original.host_str() == Some("w.wallhaven.cc")
                && original.path().starts_with("/full/")
                && original.path().rsplit('/').next().is_some_and(|name| {
                    name.strip_prefix("wallhaven-")
                        .and_then(|name| name.rsplit_once('.'))
                        .is_some_and(|(stem, _)| stem == id)
                })
                && preview.host_str() == Some("th.wallhaven.cc")
                && preview.path().rsplit('/').next() == Some(format!("{id}.jpg").as_str())
        }
        Provider::Commons => {
            let original_file = original.path().strip_prefix("/wikipedia/commons/");
            original.host_str() == Some("upload.wikimedia.org")
                && matches!(
                    preview.host_str(),
                    Some("upload.wikimedia.org" | "thumb.wikimedia.org")
                )
                && source.host_str() == Some("commons.wikimedia.org")
                && source.path().starts_with("/wiki/File:")
                && original_file.is_some_and(|file| {
                    !file.is_empty()
                        && (
                            // MediaWiki 小图不重绘；原图与预览的追踪参数可能不同。
                            (preview.host_str() == original.host_str()
                                && preview.path() == original.path())
                                || preview
                                    .path()
                                    .strip_prefix("/wikipedia/commons/thumb/")
                                    .is_some_and(|thumbnail| {
                                        thumbnail.starts_with(&format!("{file}/"))
                                    })
                        )
                })
        }
        Provider::Nekosbest => {
            original.host_str() == Some("nekos.best")
                && original.path().starts_with(&format!("/api/v2/{category}/"))
                && original.path().ends_with(".png")
                && image.original_url == image.preview_url
        }
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Missing wallpaper {key}"))
}
fn dimension(value: &Value, key: &str) -> u64 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| *value <= 30_000)
        .unwrap_or(0)
}

pub(crate) fn parse(
    provider: Provider,
    category: &str,
    body: &str,
) -> Result<Vec<RemoteImage>, String> {
    if body.len() > RESPONSE_LIMIT {
        return Err("Wallpaper response exceeds its limit".into());
    }
    let value: Value = serde_json::from_str(body).map_err(|_| "Invalid wallpaper JSON")?;
    if provider == Provider::Commons
        && value.get("query").is_none()
        && value.get("batchcomplete").and_then(Value::as_bool) == Some(true)
        && value.get("error").is_none()
    {
        return Ok(Vec::new());
    }
    let rows = match provider {
        Provider::Upx8 if value.get("code").and_then(Value::as_u64) == Some(200) => {
            value.get("data")
        }
        Provider::Wallhaven => value.get("data"),
        Provider::Commons => value.pointer("/query/pages"),
        Provider::Nekosbest => value.get("results"),
        _ => None,
    }
    .and_then(Value::as_array)
    .ok_or("Missing wallpaper list")?;
    if rows.len() > 100 {
        return Err("Invalid wallpaper list size".into());
    }
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let mut images = Vec::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        let candidate = (|| -> Result<RemoteImage, String> {
            let (title, source_url, width, height, original_url, preview_url) = match provider {
                Provider::Upx8 => {
                    // 此供应商的 Bing 每日兜底不是所选图源；拒绝将其混入 birdpaper 列表。
                    if text(row, "source")? != "birdpaper" {
                        return Err("Unexpected UPX8 source".into());
                    }
                    let image = upx8_image_url(text(row, "url")?)?;
                    (
                        row.get("title")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned(),
                        "https://wp.upx8.com/".into(),
                        dimension(row, "width"),
                        dimension(row, "height"),
                        image.clone(),
                        image,
                    )
                }
                Provider::Wallhaven => {
                    if text(row, "purity")? != "sfw" {
                        return Err("Unexpected Wallhaven purity".into());
                    }
                    if category != "all" && text(row, "category")? != category {
                        return Err("Unexpected wallpaper category".into());
                    }
                    (
                        format!("Wallhaven · {}", text(row, "id")?),
                        text(row, "url")?.into(),
                        dimension(row, "dimension_x"),
                        dimension(row, "dimension_y"),
                        text(row, "path")?.into(),
                        row.pointer("/thumbs/large")
                            .and_then(Value::as_str)
                            .ok_or("Missing wallpaper thumbnail")?
                            .into(),
                    )
                }
                Provider::Commons => {
                    let info = row
                        .pointer("/imageinfo/0")
                        .ok_or("Missing Commons image info")?;
                    if !matches!(
                        text(info, "mime")?,
                        "image/jpeg" | "image/png" | "image/webp"
                    ) {
                        return Err("Unsupported Commons format".into());
                    }
                    (
                        text(row, "title")?
                            .strip_prefix("File:")
                            .unwrap_or(text(row, "title")?)
                            .into(),
                        text(info, "descriptionurl")?.into(),
                        dimension(info, "width"),
                        dimension(info, "height"),
                        text(info, "url")?.into(),
                        text(info, "thumburl")?.into(),
                    )
                }
                Provider::Nekosbest => {
                    let image = text(row, "url")?.to_owned();
                    let dimensions = row.get("dimensions").ok_or("Missing image dimensions")?;
                    let (width, height) = (
                        dimension(dimensions, "width"),
                        dimension(dimensions, "height"),
                    );
                    // 此图源没有独立缩略图，预览也要解码原图；先剔除已知超额尺寸再补齐。
                    if width == 0
                        || height == 0
                        || width > 8192
                        || height > 8192
                        || width * height > 16_000_000
                    {
                        return Err("Nekos image exceeds preview dimensions".into());
                    }
                    (
                        text(row, "artist_name")?.into(),
                        text(row, "source_url")?.into(),
                        width,
                        height,
                        image.clone(),
                        image,
                    )
                }
            };
            let title = plain(title.trim(), 120);
            Ok(RemoteImage {
                result_id: String::new(),
                title: if title.is_empty() {
                    format!("{} · {}", provider.name(), images.len() + 1)
                } else {
                    title
                },
                source_url,
                width,
                height,
                original_url,
                preview_url,
            })
        })();
        if let Ok(image) = candidate
            && valid_image(provider, category, &image)
            && seen.insert(image.original_url.clone())
        {
            images.push(image);
        }
    }
    if images.is_empty() {
        return Err("No supported wallpaper images".into());
    }
    Ok(images)
}
