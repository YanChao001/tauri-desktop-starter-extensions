//! 图源请求、JSON 与结果身份归插件；网络、图片资源和图库由通用宿主能力核验。
mod providers;
use providers::{IMAGE_COUNT, MAX_ROUND, Provider};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{DefaultHasher, Hash, Hasher},
};
const MAX_STATE_BYTES: usize = 240_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RemoteImage {
    result_id: String,
    title: String,
    source_url: String,
    width: u64,
    height: u64,
    original_url: String,
    preview_url: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pending {
    provider: Provider,
    query: String,
    category: String,
    page: u32,
    sequence: u32,
    round: u8,
    items: Vec<RemoteImage>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    schema: u8,
    provider: Provider,
    query: String,
    category: String,
    page: u32,
    sequence: u32,
    items: Vec<RemoteImage>,
    pending: Option<Pending>,
    error: String,
}
fn plain(source: &str, maximum: usize) -> String {
    source
        .chars()
        .filter(|character| !character.is_control())
        .take(maximum)
        .collect()
}
fn result_id(image: &RemoteImage) -> String {
    // 列表身份仅属于插件会话，不是原生授权令牌。
    let mut hash = DefaultHasher::new();
    image.original_url.hash(&mut hash);
    image.preview_url.hash(&mut hash);
    image.source_url.hash(&mut hash);
    format!("{:016x}", hash.finish())
}
fn valid_images(provider: Provider, category: &str, images: &[RemoteImage]) -> bool {
    let mut ids = BTreeSet::new();
    let mut originals = BTreeSet::new();
    images.len() <= IMAGE_COUNT
        && images.iter().all(|image| {
            image.result_id == result_id(image)
                && ids.insert(&image.result_id)
                && originals.insert(&image.original_url)
                && !image.title.is_empty()
                && image.title.chars().count() <= 120
                && !image.title.chars().any(char::is_control)
                && image.width <= 30_000
                && image.height <= 30_000
                && providers::valid_image(provider, category, image)
        })
}
fn valid_state(state: &State) -> bool {
    state.schema == 3
        && state.page <= 40
        && valid_selection(state.provider, &state.query, &state.category)
        && state.sequence <= 1_000_000
        && state.error.len() <= 120
        && valid_images(state.provider, &state.category, &state.items)
        && state.pending.as_ref().is_none_or(|pending| {
            pending.sequence == state.sequence
                && pending.sequence > 0
                && (1..=40).contains(&pending.page)
                && valid_selection(pending.provider, &pending.query, &pending.category)
                && pending.round <= MAX_ROUND
                && (pending.round == 0
                    || matches!(pending.provider, Provider::Upx8 | Provider::Nekosbest))
                && valid_images(pending.provider, &pending.category, &pending.items)
        })
}
fn valid_selection(provider: Provider, query: &str, category: &str) -> bool {
    providers::valid_query(query)
        && query == query.trim()
        && provider.valid_category(category)
        && (provider != Provider::Upx8 || query.is_empty() || category == "all")
}
fn tr<'a>(english: bool, zh: &'a str, en: &'a str) -> &'a str {
    if english { en } else { zh }
}
fn effects(pending: &Pending) -> Vec<Value> {
    pending.provider.urls(pending.sequence, pending.round,&pending.query,&pending.category,pending.page).into_iter().enumerate().map(|(offset, url)| {
        let index = usize::from(pending.round) * 3 + offset;
        json!({"kind":"http","id":pending.provider.effect_id(pending.sequence,index),"request":{"url":url,"method":"GET","headers":{"Accept":"application/json","User-Agent":"TauriDesktopStarterWallpaper/2.0 (https://github.com/YanChao001/tauri-desktop-starter-extensions)"},"body":null,"credential":null}})
    }).collect()
}
fn error_code(receipt: &Value) -> &str {
    receipt
        .get("code")
        .and_then(Value::as_str)
        .filter(|code| {
            !code.is_empty()
                && code.len() <= 120
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte == b'_')
        })
        .unwrap_or("SOURCE_FAILED")
}

pub fn process(source: &str) -> Result<String, String> {
    if source.len() > 8 * 1024 * 1024 {
        return Err("Wallpaper request exceeds its limit".into());
    }
    let request: Value = serde_json::from_str(source).map_err(|_| "Invalid wallpaper request")?;
    let fields = request
        .get("fields")
        .and_then(Value::as_object)
        .ok_or("Invalid wallpaper fields")?;
    let locale = request
        .get("locale")
        .and_then(Value::as_str)
        .ok_or("Missing wallpaper locale")?;
    let english = locale == "en";
    let action = match request.get("action") {
        Some(Value::String(value)) => Some(value.as_str()),
        Some(Value::Null) => None,
        _ => return Err("Invalid wallpaper action".into()),
    };
    let encoded = request.get("state").and_then(Value::as_str).unwrap_or("");
    if request.get("version").and_then(Value::as_u64) != Some(2)
        || !matches!(locale, "en" | "zh-CN")
        || encoded.len() > MAX_STATE_BYTES
        || fields.len() > 3
        || fields.iter().any(|(key, value)| {
            !matches!(key.as_str(), "provider" | "query" | "category") || !value.is_string()
        })
    {
        return Err("Unsupported wallpaper view request".into());
    }
    let mut state: State = if encoded.is_empty() {
        State {
            schema: 3,
            category: "all".into(),
            ..State::default()
        }
    } else {
        serde_json::from_str(encoded).map_err(|_| "Invalid wallpaper session; original retained")?
    };
    if !valid_state(&state) {
        return Err("Invalid wallpaper session; original retained".into());
    }
    let (base_provider, base_query, base_category) = state
        .pending
        .as_ref()
        .map(|pending| {
            (
                pending.provider,
                pending.query.as_str(),
                pending.category.as_str(),
            )
        })
        .unwrap_or((
            state.provider,
            state.query.as_str(),
            state.category.as_str(),
        ));
    let provider = if let Some(value) = fields.get("provider") {
        Provider::from_id(value.as_str().ok_or("Invalid wallpaper provider")?)
            .ok_or("Invalid wallpaper provider")?
    } else {
        base_provider
    };
    let query =
        fields
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or(if provider == base_provider {
                base_query
            } else {
                ""
            });
    if query.len() > 400 || query.chars().any(char::is_control) {
        return Err("Invalid wallpaper query".into());
    }
    let query = query.trim().to_owned();
    let category =
        fields
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or(if provider == base_provider {
                base_category
            } else {
                provider.default_category()
            });
    if !provider.valid_category(category) || !providers::valid_query(&query) {
        return Err("Invalid wallpaper filters".into());
    }
    // 此 API 明确规定 content 优先；不能把被上游忽略的分类标成已生效筛选。
    let category = if provider == Provider::Upx8 && !query.is_empty() {
        "all"
    } else {
        category
    }
    .to_owned();
    let granted = request
        .pointer("/host/permissions")
        .and_then(Value::as_array)
        .is_some_and(|permissions| {
            permissions
                .iter()
                .any(|value| value.as_str() == Some("network:https"))
        });
    let results = request
        .get("results")
        .and_then(Value::as_array)
        .ok_or("Invalid wallpaper receipts")?;
    if results.len() > 3 || (action != Some("host-result") && !results.is_empty()) {
        return Err("Unexpected wallpaper receipt".into());
    }
    let mut next_effects = Vec::new();
    match action {
        None => {}
        Some("load") => {
            state.pending = None;
            state.error.clear();
            if !granted {
                state.error = "NETWORK_PERMISSION".into();
            } else {
                state.sequence = state.sequence % 1_000_000 + 1;
                let pending = Pending {
                    provider,
                    query: query.clone(),
                    category: category.clone(),
                    page: if state.provider == provider
                        && state.query == query
                        && state.category == category
                    {
                        state.page % 40 + 1
                    } else {
                        1
                    },
                    sequence: state.sequence,
                    round: 0,
                    items: Vec::new(),
                };
                next_effects = effects(&pending);
                state.pending = Some(pending);
            }
        }
        Some("host-result") => {
            let mut pending = state.pending.take().ok_or("Unexpected wallpaper receipt")?;
            let urls = pending.provider.urls(
                pending.sequence,
                pending.round,
                &pending.query,
                &pending.category,
                pending.page,
            );
            if results.len() != urls.len() {
                return Err("Missing wallpaper receipt".into());
            }
            let mut receipts = BTreeMap::new();
            for receipt in results {
                let id = receipt
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or("Invalid wallpaper receipt")?;
                if receipts.insert(id, receipt).is_some() {
                    return Err("Duplicate wallpaper receipt".into());
                }
            }
            let mut seen: BTreeSet<_> = pending
                .items
                .iter()
                .map(|image| image.original_url.clone())
                .collect();
            for (offset, url) in urls.iter().enumerate() {
                let index = usize::from(pending.round) * 3 + offset;
                let id = pending.provider.effect_id(pending.sequence, index);
                let receipt = receipts
                    .get(id.as_str())
                    .ok_or("Invalid wallpaper receipt identity")?;
                let value = receipt.get("value").filter(|value| !value.is_null());
                let error = receipt.get("error").filter(|value| !value.is_null());
                if value.is_some() == error.is_some() {
                    return Err("Invalid wallpaper receipt".into());
                }
                if let Some(error) = error {
                    state.error = error_code(error).into();
                    continue;
                }
                let value = value.expect("exclusive receipt value");
                if value.get("url").and_then(Value::as_str) != Some(url.as_str())
                    || value.get("status").and_then(Value::as_u64) != Some(200)
                {
                    state.error = if value.get("status").and_then(Value::as_u64) == Some(429) {
                        "SOURCE_RATE_LIMIT"
                    } else {
                        "SOURCE_RESPONSE"
                    }
                    .into();
                    continue;
                }
                match value
                    .get("body")
                    .and_then(Value::as_str)
                    .ok_or("Missing wallpaper body")
                    .and_then(|body| {
                        providers::parse(pending.provider, &pending.category, body)
                            .map_err(|_| "Invalid wallpaper body")
                    }) {
                    Ok(items) => {
                        for mut image in items {
                            if pending.items.len() < IMAGE_COUNT
                                && seen.insert(image.original_url.clone())
                            {
                                image.result_id = result_id(&image);
                                pending.items.push(image);
                            }
                        }
                    }
                    Err(_) => state.error = "SOURCE_RESPONSE".into(),
                }
            }
            if state.error.is_empty() {
                let require_full = pending.query.is_empty()
                    && (pending.provider == Provider::Nekosbest
                        || (pending.provider == Provider::Upx8 && pending.category == "all"));
                if pending.items.len() == IMAGE_COUNT || pending.items.is_empty() || !require_full {
                    state.provider = pending.provider;
                    state.query = pending.query;
                    state.category = pending.category;
                    state.page = pending.page;
                    state.items = pending.items;
                } else if matches!(pending.provider, Provider::Upx8 | Provider::Nekosbest)
                    && pending.round < MAX_ROUND
                {
                    // 接收已完成请求的元数据不授予后续请求资格；每轮补齐重新核验授权。
                    if granted {
                        pending.round += 1;
                        next_effects = effects(&pending);
                        state.pending = Some(pending);
                    } else {
                        state.error = "NETWORK_PERMISSION".into();
                    }
                } else {
                    state.error = "SOURCE_INCOMPLETE".into();
                }
            }
        }
        _ => return Err("Unsupported wallpaper action".into()),
    }
    let mut errors = BTreeMap::new();
    if !state.error.is_empty() {
        errors.insert("provider", match state.error.as_str() {
            "NETWORK_PERMISSION" => tr(english,"请授予 HTTPS 网络权限以加载壁纸；本机图库仍可使用。","Grant HTTPS network access to load wallpapers. The local gallery remains available."),
            "SOURCE_RATE_LIMIT" => tr(english,"图源请求过于频繁，请稍后刷新或切换图源。","The source is rate limited. Refresh later or choose another source."),
            "SOURCE_INCOMPLETE" => tr(english,"图源暂未返回 24 张不同图片，请刷新或切换图源。","The source did not return 24 distinct images. Refresh or choose another source."),
            _ => tr(english,"壁纸加载失败，请检查网络或切换图源后重试。","Wallpaper loading failed. Check your connection or choose another source."),
        });
    }
    let encoded = serde_json::to_string(&state).map_err(|_| "Cannot encode wallpaper session")?;
    if encoded.len() > MAX_STATE_BYTES {
        return Err("Wallpaper session exceeds its limit".into());
    }
    let provider_options = [
        Provider::Upx8,
        Provider::Wallhaven,
        Provider::Commons,
        Provider::Nekosbest,
    ]
    .map(|provider| json!({"value":provider.id(),"label":provider.name()}));
    Ok(json!({"version":2,"state":encoded,"display":tr(english,"壁纸库","Wallpaper Library"),"description":tr(english,"搜索、筛选免费图片 API，每批最多 24 张壁纸。","Search and filter free image APIs for up to 24 wallpapers per batch."),"fields":[{"id":"provider","label":tr(english,"壁纸图源","Wallpaper source"),"value":provider.id(),"kind":"select","options":provider_options,"maxBytes":32},{"id":"query","label":if provider == Provider::Nekosbest {tr(english,"搜索作者或作品信息","Search artist or work metadata")}else{tr(english,"搜索壁纸","Search wallpapers")},"value":query,"kind":"text","maxBytes":400},{"id":"category","label":if provider==Provider::Commons {tr(english,"主题","Theme")}else{tr(english,"分类","Category")},"value":category,"kind":"select","options":providers::category_options(provider,english),"maxBytes":32}],"groups":[],"tabs":[],"blocks":[{"kind":"metrics","items":[{"label":tr(english,"图片","Images"),"value":state.items.len().to_string()}]}],"errors":errors,"effects":next_effects,"artifacts":[]}).to_string())
}
#[cfg(target_arch = "wasm32")]
#[extism_pdk::plugin_fn]
pub fn browse(input: String) -> extism_pdk::FnResult<String> {
    Ok(process(&input).map_err(extism_pdk::Error::msg)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn call(
        action: Option<&str>,
        state: &str,
        fields: Value,
        permissions: &[&str],
        results: Value,
    ) -> Result<Value, String> {
        let request = json!({"version":2,"action":action,"state":state,"fields":fields,"locale":"en","host":{"permissions":permissions,"storage":null},"results":results});
        serde_json::from_str(&process(&request.to_string())?).map_err(|_| "Invalid output".into())
    }
    fn saved(view: &Value) -> State {
        serde_json::from_str(view["state"].as_str().unwrap()).unwrap()
    }
    fn row(provider: Provider, index: usize) -> Value {
        match provider {
            Provider::Upx8 => {
                json!({"url":format!("http://cdn-hsyq-static.shanhutech.cn/bizhi/staticwp/image-{index}.jpg"),"title":"","width":0,"height":0,"source":"birdpaper"})
            }
            Provider::Wallhaven => {
                json!({"id":format!("abc{index}"),"url":format!("https://wallhaven.cc/w/abc{index}"),"path":format!("https://w.wallhaven.cc/full/ab/wallhaven-abc{index}.jpg"),"thumbs":{"large":format!("https://th.wallhaven.cc/lg/ab/abc{index}.jpg")},"purity":"sfw","dimension_x":1920,"dimension_y":1080})
            }
            Provider::Commons => {
                json!({"title":format!("File:Landscape-{index}.jpg"),"imageinfo":[{"descriptionurl":format!("https://commons.wikimedia.org/wiki/File:Landscape-{index}.jpg"),"url":format!("https://upload.wikimedia.org/wikipedia/commons/a/ab/Landscape-{index}.jpg"),"thumburl":format!("https://thumb.wikimedia.org/wikipedia/commons/thumb/a/ab/Landscape-{index}.jpg/960px-Landscape-{index}.jpg"),"width":1920,"height":1080,"mime":"image/jpeg"}]})
            }
            Provider::Nekosbest => {
                json!({"artist_name":"Illustrator","source_url":format!("https://www.pixiv.net/en/artworks/{index}"),"url":format!("https://nekos.best/api/v2/neko/image-{index}.png"),"dimensions":{"width":2000,"height":3000}})
            }
        }
    }
    fn body(provider: Provider, start: usize, count: usize, category: &str) -> String {
        let rows: Vec<_> = (start..start + count)
            .map(|index| {
                let mut value = row(provider, index);
                if provider == Provider::Wallhaven {
                    value["category"] = json!(if category == "all" {
                        "general"
                    } else {
                        category
                    });
                }
                if provider == Provider::Nekosbest {
                    value["url"] = json!(format!(
                        "https://nekos.best/api/v2/{category}/image-{index}.png"
                    ));
                }
                value
            })
            .collect();
        match provider {
            Provider::Upx8 => json!({"code":200,"data":rows}),
            Provider::Wallhaven => json!({"data":rows}),
            Provider::Commons => json!({"query":{"pages":rows}}),
            Provider::Nekosbest => json!({"results":rows}),
        }
        .to_string()
    }
    fn receipts(view: &Value, duplicate_batches: bool) -> Value {
        let state = saved(view);
        let pending = state.pending.unwrap();
        json!(view["effects"].as_array().unwrap().iter().enumerate().map(|(offset,effect)| {
            let start = if duplicate_batches { 0 } else { usize::from(pending.round)*30 + offset*10 };
            let count = match pending.provider { Provider::Upx8 => 10, Provider::Nekosbest => 20, _ => 24 };
            json!({"id":effect["id"],"value":{"url":effect["request"]["url"],"status":200,"headers":{},"body":body(pending.provider,start,count,&pending.category)},"error":null})
        }).collect::<Vec<_>>())
    }
    fn load(provider: Provider) -> Value {
        call(
            Some("load"),
            "",
            json!({"provider":provider.id()}),
            &["network:https"],
            json!([]),
        )
        .unwrap()
    }
    fn finish(view: &Value, results: Value) -> Value {
        call(
            Some("host-result"),
            view["state"].as_str().unwrap(),
            json!({}),
            &["network:https"],
            results,
        )
        .unwrap()
    }

    #[test]
    fn bootstrap_locale_and_restored_pending_do_not_implicitly_run_network() {
        let initial = call(None, "", json!({}), &[], json!([])).unwrap();
        assert_eq!(saved(&initial).schema, 3);
        assert_eq!(initial["effects"], json!([]));
        let pending = load(Provider::Upx8);
        let restored = call(
            None,
            pending["state"].as_str().unwrap(),
            json!({}),
            &[],
            json!([]),
        )
        .unwrap();
        assert_eq!(restored["state"], pending["state"]);
        assert_eq!(restored["effects"], json!([]));
    }
    #[test]
    fn four_providers_produce_24_distinct_stable_images_and_inherit_connection_mode() {
        for provider in [
            Provider::Upx8,
            Provider::Wallhaven,
            Provider::Commons,
            Provider::Nekosbest,
        ] {
            let loading = load(provider);
            assert!(
                loading["effects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|effect| effect["request"].get("networkMode").is_none())
            );
            let completed = finish(&loading, receipts(&loading, false));
            let state = saved(&completed);
            assert_eq!(state.provider, provider);
            assert_eq!(state.items.len(), 24);
            assert!(state.pending.is_none());
            assert!(valid_state(&state));
            assert_eq!(completed["effects"], json!([]));
            let rerendered = call(
                None,
                completed["state"].as_str().unwrap(),
                json!({}),
                &[],
                json!([]),
            )
            .unwrap();
            assert_eq!(rerendered["state"], completed["state"]);
            if provider == Provider::Nekosbest {
                assert_eq!((state.items[0].width, state.items[0].height), (2000, 3000));
            }
        }
    }
    #[test]
    fn random_providers_deduplicate_and_top_up_with_bounded_distinct_effects() {
        for provider in [Provider::Upx8, Provider::Nekosbest] {
            let loading = load(provider);
            let partial = finish(&loading, receipts(&loading, true));
            assert_eq!(
                saved(&partial).pending.as_ref().unwrap().items.len(),
                if provider == Provider::Upx8 { 10 } else { 20 }
            );
            assert!(saved(&partial).items.is_empty());
            assert_ne!(partial["effects"][0]["id"], loading["effects"][0]["id"]);
            let done = finish(&partial, receipts(&partial, false));
            assert_eq!(saved(&done).items.len(), 24);
            let mut duplicate = loading;
            for _ in 0..=MAX_ROUND {
                duplicate = finish(&duplicate, receipts(&duplicate, true));
            }
            assert_eq!(saved(&duplicate).error, "SOURCE_INCOMPLETE");
            assert!(saved(&duplicate).pending.is_none());
            assert_eq!(duplicate["effects"], json!([]));
        }
    }
    #[test]
    fn a_top_up_requires_current_network_access_and_preserves_accepted_results() {
        let loading = load(Provider::Wallhaven);
        let accepted = finish(&loading, receipts(&loading, false));
        for provider in [Provider::Upx8, Provider::Nekosbest] {
            let next = call(
                Some("load"),
                accepted["state"].as_str().unwrap(),
                json!({"provider":provider.id(),"query":"","category":provider.default_category()}),
                &["network:https"],
                json!([]),
            )
            .unwrap();
            let denied = call(
                Some("host-result"),
                next["state"].as_str().unwrap(),
                json!({}),
                &[],
                receipts(&next, true),
            )
            .unwrap();
            let state = saved(&denied);
            assert_eq!(denied["effects"], json!([]));
            assert!(state.pending.is_none());
            assert_eq!(state.error, "NETWORK_PERMISSION");
            assert_eq!(state.provider, saved(&accepted).provider);
            assert_eq!(state.items, saved(&accepted).items);
            assert_eq!(state.query, saved(&accepted).query);
            assert_eq!(state.category, saved(&accepted).category);
            assert_eq!(state.page, saved(&accepted).page);
        }
    }
    #[test]
    fn failed_provider_change_retains_previous_source_and_images_without_retry() {
        let loading = load(Provider::Wallhaven);
        let done = finish(&loading, receipts(&loading, false));
        let switched = call(
            Some("load"),
            done["state"].as_str().unwrap(),
            json!({"provider":"commons"}),
            &["network:https"],
            json!([]),
        )
        .unwrap();
        let failed = finish(
            &switched,
            json!([{"id":"wallpaper-commons-2-0","value":null,"error":{"code":"EXTENSION_TIMEOUT"}}]),
        );
        assert_eq!(saved(&failed).provider, Provider::Wallhaven);
        assert_eq!(saved(&failed).items, saved(&done).items);
        assert!(failed["errors"]["provider"].is_string());
        assert_eq!(failed["effects"], json!([]));
    }
    #[test]
    fn receipts_are_bound_to_provider_sequence_url_and_effect_identity() {
        let loading = load(Provider::Wallhaven);
        for path in ["/0/id", "/0/value/url"] {
            let mut receipt = receipts(&loading, false);
            *receipt.pointer_mut(path).unwrap() = json!("https://unrelated.wikipedia.org/");
            if path == "/0/id" {
                assert!(
                    call(
                        Some("host-result"),
                        loading["state"].as_str().unwrap(),
                        json!({}),
                        &[],
                        receipt
                    )
                    .is_err()
                );
            } else {
                assert_eq!(saved(&finish(&loading, receipt)).error, "SOURCE_RESPONSE");
            }
        }
        let mut both = receipts(&loading, false);
        both[0]["error"] = json!({"code":"FAILED"});
        assert!(
            call(
                Some("host-result"),
                loading["state"].as_str().unwrap(),
                json!({}),
                &[],
                both
            )
            .is_err()
        );
        assert!(
            call(
                Some("host-result"),
                loading["state"].as_str().unwrap(),
                json!({}),
                &[],
                json!([])
            )
            .is_err()
        );
    }
    #[test]
    fn unavailable_network_and_invalid_inputs_never_emit_http() {
        let denied = call(
            Some("load"),
            "",
            json!({}),
            &["network:images", "network:system-proxy"],
            json!([]),
        )
        .unwrap();
        assert_eq!(denied["effects"], json!([]));
        assert_eq!(saved(&denied).error, "NETWORK_PERMISSION");
        for fields in [
            json!({"provider":"bing"}),
            json!({"provider":3}),
            json!({"category":"invalid"}),
        ] {
            assert!(call(Some("load"), "", fields, &[], json!([])).is_err());
        }
        for state in ["{\"schema\":99}", "{\"schema\":1,\"query\":\"landscape\"}"] {
            assert!(call(None, state, json!({}), &[], json!([])).is_err());
        }
        assert!(call(Some("search"), "", json!({}), &[], json!([])).is_err());
    }
    #[test]
    fn search_fields_are_trimmed_bounded_and_encoded_as_one_query_parameter() {
        let query = "A&B + 猫 ?purity=111";
        for provider in [
            Provider::Upx8,
            Provider::Wallhaven,
            Provider::Commons,
            Provider::Nekosbest,
        ] {
            let loading=call(Some("load"),"",json!({"provider":provider.id(),"query":format!("  {query}  "),"category":provider.default_category()}),&["network:https"],json!([])).unwrap();
            let pending = saved(&loading).pending.unwrap();
            assert_eq!(pending.query, query);
            let url =
                url::Url::parse(loading["effects"][0]["request"]["url"].as_str().unwrap()).unwrap();
            let pairs: BTreeMap<_, _> = url
                .query_pairs()
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect();
            let key = match provider {
                Provider::Upx8 => "content",
                Provider::Wallhaven => "q",
                Provider::Commons => "gsrsearch",
                Provider::Nekosbest => "query",
            };
            let expected = if provider == Provider::Commons {
                format!("{query} filetype:bitmap")
            } else {
                query.into()
            };
            assert_eq!(pairs[key], expected);
            assert_eq!(
                pairs.get("purity").map(String::as_str),
                if provider == Provider::Wallhaven {
                    Some("100")
                } else {
                    None
                }
            );
        }
        for query in [
            "x".repeat(101),
            "😀".repeat(101),
            "\nforest".into(),
            "forest\u{0000}".into(),
        ] {
            assert!(
                call(
                    Some("load"),
                    "",
                    json!({"query":query}),
                    &["network:https"],
                    json!([])
                )
                .is_err()
            );
        }
        assert!(
            call(
                Some("load"),
                "",
                json!({"query":"😀".repeat(100)}),
                &["network:https"],
                json!([])
            )
            .is_ok()
        );
    }
    #[test]
    fn provider_categories_map_to_real_api_filters_and_neko_image_identity() {
        for (category, expected) in [
            ("all", "111"),
            ("general", "100"),
            ("anime", "010"),
            ("people", "001"),
        ] {
            let urls = Provider::Wallhaven.urls(1, 0, "mountain", category, 1);
            let url = url::Url::parse(&urls[0]).unwrap();
            assert!(
                url.query_pairs()
                    .any(|(key, value)| key == "categories" && value == expected)
            );
        }
        let loading = call(
            Some("load"),
            "",
            json!({"provider":"upx8","query":"森林","category":"anime"}),
            &["network:https"],
            json!([]),
        )
        .unwrap();
        assert_eq!(saved(&loading).pending.unwrap().category, "all");
        assert!(
            !loading["effects"][0]["request"]["url"]
                .as_str()
                .unwrap()
                .contains("category=")
        );
        for category in ["neko", "kitsune", "waifu", "husbando"] {
            let loading = call(
                Some("load"),
                "",
                json!({"provider":"nekosbest","category":category,"query":""}),
                &["network:https"],
                json!([]),
            )
            .unwrap();
            assert!(
                loading["effects"][0]["request"]["url"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("/api/v2/{category}?"))
            );
            let complete = finish(&loading, receipts(&loading, false));
            assert_eq!(saved(&complete).category, category);
            assert_eq!(saved(&complete).items.len(), 24);
            let wrong = body(Provider::Nekosbest, 0, 2, "neko");
            if category != "neko" {
                assert!(providers::parse(Provider::Nekosbest, category, &wrong).is_err());
            }
        }
        assert!(
            call(
                Some("load"),
                "",
                json!({"provider":"wallhaven","category":"kitsune"}),
                &["network:https"],
                json!([])
            )
            .is_err()
        );
    }
    #[test]
    fn partial_and_zero_searches_are_successes_with_their_actual_filters() {
        for provider in [
            Provider::Upx8,
            Provider::Wallhaven,
            Provider::Commons,
            Provider::Nekosbest,
        ] {
            let loading=call(Some("load"),"",json!({"provider":provider.id(),"query":"small collection","category":provider.default_category()}),&["network:https"],json!([])).unwrap();
            let mut values = receipts(&loading, true);
            for receipt in values.as_array_mut().unwrap() {
                receipt["value"]["body"] = json!(body(provider, 0, 3, provider.default_category()));
            }
            let complete = finish(&loading, values);
            let current = saved(&complete);
            assert_eq!(current.query, "small collection");
            assert_eq!(current.items.len(), 3);
            assert!(current.error.is_empty());
            let next=call(Some("load"),complete["state"].as_str().unwrap(),json!({"provider":provider.id(),"query":"no matching images","category":provider.default_category()}),&["network:https"],json!([])).unwrap();
            let mut values = receipts(&next, false);
            for receipt in values.as_array_mut().unwrap() {
                receipt["value"]["body"] = json!(if provider == Provider::Commons {
                    "{\"batchcomplete\":true}".to_owned()
                } else {
                    body(provider, 0, 0, provider.default_category())
                });
            }
            let empty = finish(&next, values);
            let state = saved(&empty);
            assert!(state.items.is_empty());
            assert_eq!(state.query, "no matching images");
            assert_eq!(state.provider, provider);
            assert!(state.error.is_empty());
            assert_eq!(empty["effects"], json!([]));
        }
    }
    #[test]
    fn commons_unscaled_previews_keep_the_exact_original_file_identity() {
        // 官 API 对小于 iiurlwidth 的 JPEG 返回原图地址，只改变追踪参数，不添加 /thumb/。
        let row = json!({"title":"File:Example.jpg","imageinfo":[{
            "descriptionurl":"https://commons.wikimedia.org/wiki/File:Example.jpg",
            "url":"https://upload.wikimedia.org/wikipedia/commons/a/a9/Example.jpg?utm_content=original",
            "thumburl":"https://upload.wikimedia.org/wikipedia/commons/a/a9/Example.jpg?utm_content=thumbnail_unscaled",
            "width":172,"height":178,"mime":"image/jpeg"
        }]});
        let encoded = json!({"query":{"pages":[row.clone()]}}).to_string();
        let items = providers::parse(Provider::Commons, "all", &encoded).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].width, items[0].height), (172, 178));
        let loading = call(
            Some("load"),
            "",
            json!({"provider":"commons","query":"Example.jpg","category":"all"}),
            &["network:https"],
            json!([]),
        )
        .unwrap();
        let mut values = receipts(&loading, false);
        values[0]["value"]["body"] = json!(encoded);
        let done = finish(&loading, values);
        assert!(valid_state(&saved(&done)));
        assert_eq!(saved(&done).items.len(), 1);
        for preview in [
            "https://upload.wikimedia.org/wikipedia/commons/a/a9/Different.jpg",
            "https://thumb.wikimedia.org/wikipedia/commons/a/a9/Example.jpg",
            "https://upload.wikimedia.org.evil.com/wikipedia/commons/a/a9/Example.jpg",
            "http://upload.wikimedia.org/wikipedia/commons/a/a9/Example.jpg",
        ] {
            let mut invalid = row.clone();
            invalid["imageinfo"][0]["thumburl"] = json!(preview);
            assert!(
                providers::parse(
                    Provider::Commons,
                    "all",
                    &json!({"query":{"pages":[invalid]}}).to_string()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn pagination_resets_on_filter_change_and_failure_preserves_accepted_filters() {
        let loading = call(
            Some("load"),
            "",
            json!({"provider":"wallhaven","query":"mountain","category":"general"}),
            &["network:https"],
            json!([]),
        )
        .unwrap();
        let first = finish(&loading, receipts(&loading, false));
        let next = call(
            Some("load"),
            first["state"].as_str().unwrap(),
            json!({"provider":"wallhaven","query":"mountain","category":"general"}),
            &["network:https"],
            json!([]),
        )
        .unwrap();
        assert_eq!(saved(&next).pending.as_ref().unwrap().page, 2);
        assert!(
            next["effects"][0]["request"]["url"]
                .as_str()
                .unwrap()
                .ends_with("&page=2")
        );
        let changed = call(
            Some("load"),
            first["state"].as_str().unwrap(),
            json!({"provider":"wallhaven","query":"ocean","category":"anime"}),
            &["network:https"],
            json!([]),
        )
        .unwrap();
        assert_eq!(saved(&changed).pending.as_ref().unwrap().page, 1);
        let failed = finish(
            &changed,
            json!([{"id":changed["effects"][0]["id"],"value":null,"error":{"code":"EXTENSION_TIMEOUT"}}]),
        );
        let current = saved(&failed);
        assert_eq!(current.query, "mountain");
        assert_eq!(current.category, "general");
        assert_eq!(current.page, 1);
        assert_eq!(current.items, saved(&first).items);
        assert_eq!(failed["fields"][1]["value"], "ocean");
    }
    #[test]
    fn parser_rejects_bing_fallback_unsafe_sources_mismatched_thumbnails_and_non_sfw() {
        let mut upx8 = row(Provider::Upx8, 0);
        upx8["source"] = json!("bing");
        assert!(
            providers::parse(
                Provider::Upx8,
                Provider::Upx8.default_category(),
                &json!({"code":200,"data":[upx8]}).to_string()
            )
            .is_err()
        );
        for url in [
            "https://127.0.0.1/image.jpg",
            "https://cdn-hsyq-static.shanhutech.cn.evil.com/image.jpg",
            "http://images.wikipedia.org/image.jpg",
        ] {
            let mut upx8 = row(Provider::Upx8, 0);
            upx8["url"] = json!(url);
            assert!(
                providers::parse(
                    Provider::Upx8,
                    Provider::Upx8.default_category(),
                    &json!({"code":200,"data":[upx8]}).to_string()
                )
                .is_err()
            );
        }
        let mut wallhaven = row(Provider::Wallhaven, 0);
        wallhaven["purity"] = json!("nsfw");
        assert!(
            providers::parse(
                Provider::Wallhaven,
                Provider::Wallhaven.default_category(),
                &json!({"data":[wallhaven]}).to_string()
            )
            .is_err()
        );
        let mut neko = row(Provider::Nekosbest, 0);
        neko["dimensions"] = json!({"width":5000,"height":4000});
        assert!(
            providers::parse(
                Provider::Nekosbest,
                Provider::Nekosbest.default_category(),
                &json!({"results":[neko]}).to_string()
            )
            .is_err()
        );
        let mut wallpaper = row(Provider::Commons, 0);
        wallpaper["imageinfo"][0]["thumburl"] = json!(
            "https://thumb.wikimedia.org/wikipedia/commons/thumb/a/ab/Different.jpg/960px-Different.jpg"
        );
        assert!(
            providers::parse(
                Provider::Commons,
                Provider::Commons.default_category(),
                &json!({"query":{"pages":[wallpaper]}}).to_string()
            )
            .is_err()
        );
    }
    #[test]
    fn state_revalidates_metadata_identity_and_provider_provenance() {
        let loading = load(Provider::Nekosbest);
        let done = finish(&loading, receipts(&loading, false));
        let original = saved(&done);
        let mut changed = original.clone();
        changed.provider = Provider::Wallhaven;
        assert!(!valid_state(&changed));
        let mut changed = original.clone();
        changed.items[0].preview_url = "https://nekos.best/api/v2/neko/different.png".into();
        changed.items[0].result_id = result_id(&changed.items[0]);
        assert!(!valid_state(&changed));
        let mut changed = original;
        changed.items[0].result_id = "fabricated".into();
        assert!(!valid_state(&changed));
    }
    #[test]
    fn public_url_rejects_credentials_reserved_dns_ip_literals_and_fragments() {
        assert!(providers::public_url("https://nekos.best/id/1/640/400").is_ok());
        for url in [
            "http://nekos.best/a",
            "https://user:secret@nekos.best/a",
            "https://nekos.best:444/a",
            "https://nekos.best/a#fragment",
            "https://127.0.0.1/a",
            "https://2130706433/a",
            "https://[::1]/a",
            "https://8.8.8.8/a",
            "https://localhost/a",
            "https://metadata.google.internal/a",
            "https://router.home.arpa/a",
            "https://images.local/a",
            "https://images.example/a",
            "https://nekos.best./a",
        ] {
            assert!(providers::public_url(url).is_err(), "{url}");
        }
    }
}
