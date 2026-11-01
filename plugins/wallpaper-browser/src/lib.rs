//! 图源请求、网页解析与结果身份归插件；网络、图片资源和图库由通用宿主能力核验。
mod providers;
use providers::{IMAGE_COUNT, Provider};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{DefaultHasher, Hash, Hasher},
};
const MAX_STATE_BYTES: usize = 240_000;
const MAX_PAGE: u32 = 40;
const MAX_SEEN: usize = 960;
// Generated from repository.config.json and listings.repository by repository:sync.
const USER_AGENT: &str = include_str!("../user-agent.generated.txt");

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RemoteImage {
    result_id: String,
    title: String,
    source_url: String,
    width: u64,
    height: u64,
    original_url: String,
    display_url: String,
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
    reset_history: bool,
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
    has_more: bool,
    seen: Vec<String>,
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
    let mut seen = BTreeSet::new();
    state.schema == 4
        && state.page <= MAX_PAGE
        && valid_selection(state.provider, &state.query, &state.category)
        && state.sequence <= 1_000_000
        && state.error.len() <= 120
        && state.seen.len() <= MAX_SEEN
        && state.seen.len() <= state.page as usize * IMAGE_COUNT
        && state.seen.iter().all(|id| {
            id.len() == 16
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                && seen.insert(id)
        })
        && valid_images(state.provider, &state.category, &state.items)
        && state
            .items
            .iter()
            .all(|item| seen.contains(&item.result_id))
        && (state.page != 0 || (state.items.is_empty() && state.seen.is_empty() && state.has_more))
        && (state.page != MAX_PAGE || !state.has_more)
        && state.pending.as_ref().is_none_or(|pending| {
            pending.sequence == state.sequence
                && pending.sequence > 0
                && valid_selection(pending.provider, &pending.query, &pending.category)
                && if pending.reset_history {
                    pending.page == 1
                } else {
                    state.has_more
                        && pending.provider == state.provider
                        && pending.query == state.query
                        && pending.category == state.category
                        && pending.page == state.page + 1
                        && pending.page <= MAX_PAGE
                }
        })
}
fn valid_selection(provider: Provider, query: &str, category: &str) -> bool {
    providers::valid_query(query)
        && query == query.trim()
        && provider.valid_category(category)
        && (query.is_empty() || category == "all")
}
fn tr<'a>(english: bool, zh: &'a str, en: &'a str) -> &'a str {
    if english { en } else { zh }
}
fn effects(pending: &Pending) -> Vec<Value> {
    pending.provider.urls(pending.sequence,0,&pending.query,&pending.category,pending.page).into_iter().enumerate().map(|(index, url)| {
        json!({"kind":"http","id":pending.provider.effect_id(pending.sequence,index),"request":{"url":url,"method":"GET","headers":{"Accept":if pending.provider == Provider::Wallcoo { "application/json" } else { "text/html" },"User-Agent":USER_AGENT.trim_end()},"body":null,"credential":null}})
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
            schema: 4,
            category: "all".into(),
            has_more: true,
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
    // 关键词搜索与分类不是这些来源共同支持的交集；不把被忽略的分类标成已生效。
    let category = if !query.is_empty() { "all" } else { category }.to_owned();
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
    if results.len() > 1 || (action != Some("host-result") && !results.is_empty()) {
        return Err("Unexpected wallpaper receipt".into());
    }
    let mut next_effects = Vec::new();
    match action {
        None => {}
        Some("search" | "next" | "load") => {
            state.pending = None;
            state.error.clear();
            if !granted {
                state.error = "NETWORK_PERMISSION".into();
            } else {
                let reset_history = action == Some("search")
                    || state.provider != provider
                    || state.query != query
                    || state.category != category;
                if reset_history || state.has_more {
                    state.sequence = state.sequence % 1_000_000 + 1;
                    let pending = Pending {
                        provider,
                        query: query.clone(),
                        category: category.clone(),
                        page: if reset_history { 1 } else { state.page + 1 },
                        sequence: state.sequence,
                        reset_history,
                    };
                    next_effects = effects(&pending);
                    state.pending = Some(pending);
                }
            }
        }
        Some("host-result") => {
            let pending = state.pending.take().ok_or("Unexpected wallpaper receipt")?;
            let urls = pending.provider.urls(
                pending.sequence,
                0,
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
            let mut seen: BTreeSet<String> = if pending.reset_history {
                BTreeSet::new()
            } else {
                state.seen.iter().cloned().collect()
            };
            let mut items = Vec::new();
            let mut source_has_more = true;
            for (offset, url) in urls.iter().enumerate() {
                let id = pending.provider.effect_id(pending.sequence, offset);
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
                let Some(body) = value.get("body").and_then(Value::as_str) else {
                    state.error = "SOURCE_RESPONSE".into();
                    continue;
                };
                if !providers::page_matches(pending.provider, pending.page, body) {
                    state.error = "SOURCE_RESPONSE".into();
                    continue;
                }
                match providers::parse(pending.provider, &pending.category, &pending.query, body) {
                    Ok(parsed) => {
                        source_has_more =
                            providers::has_more(pending.provider, body).unwrap_or(true);
                        for mut image in parsed.into_iter().take(IMAGE_COUNT) {
                            image.result_id = result_id(&image);
                            if seen.insert(image.result_id.clone()) {
                                // 新列表、筛选与历史在确认整批成功后一起提交。
                                items.push(image);
                            }
                        }
                    }
                    Err(_) => state.error = "SOURCE_RESPONSE".into(),
                }
            }
            if !granted {
                state.error = "NETWORK_PERMISSION".into();
            }
            if state.error.is_empty() {
                state.provider = pending.provider;
                state.query = pending.query;
                state.category = pending.category;
                state.page = pending.page;
                state.has_more = source_has_more && !items.is_empty() && pending.page < MAX_PAGE;
                state.items = items;
                state.seen = seen.into_iter().collect();
            }
        }
        _ => return Err("Unsupported wallpaper action".into()),
    }
    if !valid_state(&state) {
        return Err("Invalid wallpaper session; original retained".into());
    }
    let mut errors = BTreeMap::new();
    if !state.error.is_empty() {
        errors.insert("provider", match state.error.as_str() {
            "NETWORK_PERMISSION" => tr(english,"请授予 HTTPS 网络权限以加载壁纸；本机图库仍可使用。","Grant HTTPS network access to load wallpapers. The local gallery remains available."),
            "SOURCE_RATE_LIMIT" => tr(english,"图源请求过于频繁，请稍后刷新或切换图源。","The source is rate limited. Refresh later or choose another source."),
            _ => tr(english,"壁纸加载失败，请检查网络或切换图源后重试。","Wallpaper loading failed. Check your connection or choose another source."),
        });
    }
    let encoded = serde_json::to_string(&state).map_err(|_| "Cannot encode wallpaper session")?;
    if encoded.len() > MAX_STATE_BYTES {
        return Err("Wallpaper session exceeds its limit".into());
    }
    let provider_options = [
        Provider::Bizhihui,
        Provider::Wallcoo,
        Provider::Wallpaperscraft,
        Provider::Goodfon,
    ]
    .map(|provider| json!({"value":provider.id(),"label":provider.name()}));
    Ok(json!({"version":2,"state":encoded,"display":tr(english,"壁纸库","Wallpaper Library"),"description":tr(english,"搜索、筛选四个免费壁纸网站，每批最多 24 张图片。","Search and filter four free wallpaper sites for up to 24 images per batch."),"fields":[{"id":"provider","label":tr(english,"壁纸图源","Wallpaper source"),"value":provider.id(),"kind":"select","options":provider_options,"maxBytes":32},{"id":"query","label":tr(english,"搜索壁纸","Search wallpapers"),"value":query,"kind":"text","maxBytes":400},{"id":"category","label":tr(english,"分类","Category"),"value":category,"kind":"select","options":providers::category_options(provider,english),"maxBytes":32}],"groups":[],"tabs":[],"blocks":[{"kind":"metrics","items":[{"label":tr(english,"图片","Images"),"value":state.items.len().to_string()}]}],"errors":errors,"effects":next_effects,"artifacts":[]}).to_string())
}
#[cfg(target_arch = "wasm32")]
#[extism_pdk::plugin_fn]
pub fn browse(input: String) -> extism_pdk::FnResult<String> {
    Ok(process(&input).map_err(extism_pdk::Error::msg)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use providers::fixtures;

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
    fn run(action: &str, view: &Value, fields: Value) -> Value {
        call(
            Some(action),
            view["state"].as_str().unwrap_or(""),
            fields,
            &["network:https"],
            json!([]),
        )
        .unwrap()
    }
    fn start(provider: Provider) -> Value {
        call(
            Some("search"),
            "",
            json!({"provider":provider.id()}),
            &["network:https"],
            json!([]),
        )
        .unwrap()
    }
    fn body(provider: Provider, start: usize, count: usize) -> String {
        if provider == Provider::Wallcoo {
            let mut listing: Value = serde_json::from_str(fixtures::WALLCOO).unwrap();
            let original = listing["items"][0].clone();
            listing["items"] = json!((start..start + count).map(|index| {
                let row = original.to_string().replace("cqewj8", &format!("cqew{index:04}"))
                    .replace("x29724", &format!("x{}", 29724 + index));
                let mut item: Value = serde_json::from_str(&row).unwrap();
                item["id"] = json!(260612 + index);
                item["link"] = json!(format!("https://www.wallcoo.com/landscape/avebury-stones-at-dawn-wallpaper-{index}/"));
                item
            }).collect::<Vec<_>>());
            return listing.to_string();
        }
        let (fixture, marker, closing) = match provider {
            Provider::Bizhihui => (fixtures::BIZHIHUI, "<ul ", "</ul>"),
            Provider::Wallcoo => unreachable!("Wallcoo listings use JSON"),
            Provider::Wallpaperscraft => (fixtures::WALLPAPERSCRAFT, "<ul ", "</ul>"),
            Provider::Goodfon => (fixtures::GOODFON, "<div class=wallpapers>", "</div>"),
        };
        let opening = fixture.find(marker).unwrap();
        let content_start = opening + fixture[opening..].find('>').unwrap() + 1;
        let content_end = fixture.rfind(closing).unwrap();
        let card = &fixture[content_start..content_end];
        let cards = (start..start + count)
            .map(|index| {
                card.replace(
                    "202610291793244133809052",
                    &format!("202610291793244133{index:06}"),
                )
                .replace("24385.html", &format!("{}.html", 24385 + index))
                .replace(
                    "mountains_spruce_art_167976",
                    &format!("mountains_spruce_art_{}", 167976 + index),
                )
                .replace("ice-snow-mountain", &format!("ice-snow-mountain-{index}"))
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "{}{}{}",
            &fixture[..content_start],
            cards,
            &fixture[content_end..]
        )
    }
    fn receipt(view: &Value, body: &str) -> Value {
        let effect = &view["effects"][0];
        json!([{"id":effect["id"],"value":{"url":effect["request"]["url"],"status":200,"headers":{},"body":body},"error":null}])
    }
    fn finish(view: &Value, values: Value) -> Value {
        call(
            Some("host-result"),
            view["state"].as_str().unwrap(),
            json!({}),
            &["network:https"],
            values,
        )
        .unwrap()
    }
    fn accept(view: &Value, provider: Provider, start: usize, count: usize) -> Value {
        finish(view, receipt(view, &body(provider, start, count)))
    }

    #[test]
    fn bootstrap_and_restored_pending_do_not_implicitly_repeat_network() {
        let initial = call(None, "", json!({}), &[], json!([])).unwrap();
        let state = saved(&initial);
        assert_eq!(state.page, 0);
        assert!(state.has_more);
        assert!(state.items.is_empty());
        assert!(state.seen.is_empty());
        assert_eq!(initial["effects"], json!([]));
        let pending = start(Provider::Bizhihui);
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
    fn short_valid_pages_from_each_source_become_visible_after_one_request() {
        for provider in [
            Provider::Bizhihui,
            Provider::Wallcoo,
            Provider::Wallpaperscraft,
            Provider::Goodfon,
        ] {
            let loading = start(provider);
            assert_eq!(loading["effects"].as_array().unwrap().len(), 1);
            assert!(
                loading["effects"][0]["request"]
                    .get("networkMode")
                    .is_none()
            );
            let done = accept(&loading, provider, 0, 3);
            let state = saved(&done);
            assert_eq!(state.items.len(), 3, "{}", provider.id());
            assert_eq!(state.seen.len(), 3);
            assert_eq!(state.page, 1);
            assert!(state.has_more);
            assert!(state.error.is_empty());
            assert!(state.pending.is_none());
            assert_eq!(done["effects"], json!([]));
            assert!(valid_state(&state));
        }
    }

    #[test]
    fn wallcoo_final_nonempty_page_disables_next_without_an_extra_invalid_request() {
        let loading = start(Provider::Wallcoo);
        let mut listing: Value = serde_json::from_str(fixtures::WALLCOO).unwrap();
        listing["pagination"]["has_more"] = json!(false);
        listing["pagination"]["total_pages"] = json!(1);
        listing["pagination"]["total_items"] = json!(1);
        let done = finish(&loading, receipt(&loading, &listing.to_string()));
        assert_eq!(saved(&done).items.len(), 1);
        assert!(!saved(&done).has_more);
        assert_eq!(run("next", &done, json!({}))["effects"], json!([]));
    }

    #[test]
    fn mismatched_wallcoo_pagination_keeps_the_accepted_page_and_history() {
        let loading = start(Provider::Wallcoo);
        let first = accept(&loading, Provider::Wallcoo, 0, 2);
        let next = run("next", &first, json!({}));
        assert_eq!(saved(&next).pending.unwrap().page, 2);
        for (current_page, per_page) in [(1, 24), (2, 12)] {
            let mut listing: Value =
                serde_json::from_str(&body(Provider::Wallcoo, 100, 2)).unwrap();
            listing["pagination"]["current_page"] = json!(current_page);
            listing["pagination"]["per_page"] = json!(per_page);
            let failed = finish(&next, receipt(&next, &listing.to_string()));
            let accepted = saved(&first);
            let state = saved(&failed);
            assert_eq!(
                state.page, accepted.page,
                "wrong page metadata must not commit"
            );
            assert_eq!(state.items, accepted.items);
            assert_eq!(state.seen, accepted.seen);
            assert_eq!(state.provider, accepted.provider);
            assert_eq!(state.query, accepted.query);
            assert_eq!(state.category, accepted.category);
            assert_eq!(state.has_more, accepted.has_more);
            assert_eq!(state.error, "SOURCE_RESPONSE");
            assert_eq!(
                saved(&run("next", &failed, json!({})))
                    .pending
                    .unwrap()
                    .page,
                2
            );
        }
    }

    #[test]
    fn search_restarts_the_first_page_and_resets_seen_only_after_success() {
        let loading = start(Provider::Bizhihui);
        let first = accept(&loading, Provider::Bizhihui, 0, 2);
        let next = run("next", &first, json!({}));
        let second = accept(&next, Provider::Bizhihui, 2, 2);
        assert_eq!(saved(&second).seen.len(), 4);
        let search = run("search", &second, json!({}));
        assert_eq!(saved(&search).pending.unwrap().page, 1);
        assert_eq!(saved(&search).seen.len(), 4);
        let restarted = accept(&search, Provider::Bizhihui, 0, 2);
        assert_eq!(saved(&restarted).page, 1);
        assert_eq!(saved(&restarted).items, saved(&first).items);
        assert_eq!(saved(&restarted).seen.len(), 2);
    }

    #[test]
    fn next_omits_previous_images_and_duplicate_only_pages_end_without_stale_items() {
        let loading = start(Provider::Bizhihui);
        let first = accept(&loading, Provider::Bizhihui, 0, 2);
        let next = run("next", &first, json!({}));
        assert_eq!(saved(&next).pending.unwrap().page, 2);
        let mixed = accept(&next, Provider::Bizhihui, 1, 3);
        assert_eq!(saved(&mixed).items.len(), 2);
        assert_eq!(saved(&mixed).seen.len(), 4);
        let next = run("load", &mixed, json!({}));
        assert_eq!(saved(&next).pending.unwrap().page, 3);
        let exhausted = accept(&next, Provider::Bizhihui, 0, 4);
        assert!(saved(&exhausted).items.is_empty());
        assert!(!saved(&exhausted).has_more);
        assert_eq!(saved(&exhausted).page, 3);
        assert!(saved(&exhausted).error.is_empty());
        assert_eq!(run("next", &exhausted, json!({}))["effects"], json!([]));
    }

    #[test]
    fn repeated_original_is_omitted_even_when_its_source_metadata_changes() {
        let loading = start(Provider::Bizhihui);
        let first = finish(&loading, receipt(&loading, fixtures::BIZHIHUI));
        let next = run("next", &first, json!({}));
        let relocated = fixtures::BIZHIHUI.replace("24385.html", "99999.html");
        let repeated = finish(&next, receipt(&next, &relocated));
        assert!(saved(&repeated).items.is_empty());
        assert_eq!(saved(&repeated).seen, saved(&first).seen);
        assert!(!saved(&repeated).has_more);
    }

    #[test]
    fn empty_success_replaces_the_previous_collection_and_exposes_the_actual_search() {
        let loading = start(Provider::Goodfon);
        let first = accept(&loading, Provider::Goodfon, 0, 2);
        let search = run("search", &first, json!({"query":"no matches"}));
        let empty = finish(
            &search,
            receipt(
                &search,
                "<title>Wallpaper search</title><a>GoodFon</a><div class=wallpapers></div>",
            ),
        );
        let state = saved(&empty);
        assert!(state.items.is_empty());
        assert!(state.seen.is_empty());
        assert_eq!(state.query, "no matches");
        assert_eq!(state.page, 1);
        assert!(!state.has_more);
        assert!(state.error.is_empty());
    }

    #[test]
    fn failed_next_retains_success_and_retries_the_same_page() {
        let loading = start(Provider::Bizhihui);
        let first = accept(&loading, Provider::Bizhihui, 0, 2);
        let next = run("next", &first, json!({}));
        let failed = finish(
            &next,
            json!([{"id":next["effects"][0]["id"],"value":null,"error":{"code":"EXTENSION_TIMEOUT"}}]),
        );
        assert_eq!(saved(&failed).items, saved(&first).items);
        assert_eq!(saved(&failed).seen, saved(&first).seen);
        assert_eq!(saved(&failed).page, 1);
        assert!(saved(&failed).has_more);
        assert_eq!(saved(&failed).error, "EXTENSION_TIMEOUT");
        assert!(failed["errors"]["provider"].is_string());
        assert_eq!(
            saved(&run("next", &failed, json!({})))
                .pending
                .unwrap()
                .page,
            2
        );
    }

    #[test]
    fn changed_filters_reset_pagination_but_failure_keeps_the_accepted_filters() {
        let loading = start(Provider::Bizhihui);
        let first = accept(&loading, Provider::Bizhihui, 0, 2);
        let changed = run(
            "next",
            &first,
            json!({"provider":"goodfon","query":"ocean"}),
        );
        let pending = saved(&changed).pending.unwrap();
        assert_eq!(pending.page, 1);
        assert!(pending.reset_history);
        let failed = finish(
            &changed,
            json!([{"id":changed["effects"][0]["id"],"value":null,"error":{"code":"EXTENSION_TIMEOUT"}}]),
        );
        let state = saved(&failed);
        assert_eq!(state.provider, Provider::Bizhihui);
        assert_eq!(state.query, "");
        assert_eq!(state.items, saved(&first).items);
        assert_eq!(failed["fields"][0]["value"], "goodfon");
        assert_eq!(failed["fields"][1]["value"], "ocean");
    }

    #[test]
    fn forty_pages_are_bounded_without_wrapping_to_seen_first_page() {
        let loading = start(Provider::Bizhihui);
        let first = accept(&loading, Provider::Bizhihui, 0, 1);
        let mut state = saved(&first);
        state.page = 39;
        state.seen = (50000..50935).map(|id| format!("{id:016x}")).collect();
        state.seen.push(state.items[0].result_id.clone());
        assert!(valid_state(&state));
        let view = json!({"state":serde_json::to_string(&state).unwrap()});
        let next = run("next", &view, json!({}));
        assert_eq!(saved(&next).pending.unwrap().page, 40);
        let last = accept(&next, Provider::Bizhihui, 1000, 24);
        assert_eq!(saved(&last).page, 40);
        assert_eq!(saved(&last).seen.len(), 960);
        assert!(!saved(&last).has_more);
        assert_eq!(run("next", &last, json!({}))["effects"], json!([]));
        assert_eq!(
            saved(&run("search", &last, json!({})))
                .pending
                .unwrap()
                .page,
            1
        );
    }

    #[test]
    fn receipts_reject_wrong_identity_url_status_and_ambiguous_results() {
        let loading = start(Provider::Bizhihui);
        let values = receipt(&loading, fixtures::BIZHIHUI);
        for path in ["/0/id", "/0/value/url"] {
            let mut wrong = values.clone();
            *wrong.pointer_mut(path).unwrap() = json!("https://unrelated.org/");
            if path.ends_with("id") {
                assert!(
                    call(
                        Some("host-result"),
                        loading["state"].as_str().unwrap(),
                        json!({}),
                        &["network:https"],
                        wrong
                    )
                    .is_err()
                );
            } else {
                let state = saved(&finish(&loading, wrong));
                assert_eq!(state.page, 0);
                assert_eq!(state.error, "SOURCE_RESPONSE");
            }
        }
        let mut limited = values.clone();
        limited[0]["value"]["status"] = json!(429);
        assert_eq!(saved(&finish(&loading, limited)).error, "SOURCE_RATE_LIMIT");
        let mut both = values;
        both[0]["error"] = json!({"code":"FAILED"});
        assert!(
            call(
                Some("host-result"),
                loading["state"].as_str().unwrap(),
                json!({}),
                &["network:https"],
                both
            )
            .is_err()
        );
        assert!(
            call(
                Some("host-result"),
                loading["state"].as_str().unwrap(),
                json!({}),
                &["network:https"],
                json!([])
            )
            .is_err()
        );
    }

    #[test]
    fn revoked_network_cannot_commit_a_late_success_or_issue_a_request() {
        let denied = call(
            Some("search"),
            "",
            json!({}),
            &["network:images", "network:system-proxy"],
            json!([]),
        )
        .unwrap();
        assert_eq!(denied["effects"], json!([]));
        assert_eq!(saved(&denied).error, "NETWORK_PERMISSION");
        let loading = start(Provider::Bizhihui);
        let denied = call(
            Some("host-result"),
            loading["state"].as_str().unwrap(),
            json!({}),
            &[],
            receipt(&loading, fixtures::BIZHIHUI),
        )
        .unwrap();
        assert!(saved(&denied).items.is_empty());
        assert!(saved(&denied).seen.is_empty());
        assert_eq!(saved(&denied).page, 0);
        assert_eq!(saved(&denied).error, "NETWORK_PERMISSION");
    }

    #[test]
    fn strict_state_validation_rejects_legacy_future_forged_and_unbounded_history() {
        let initial = call(None, "", json!({}), &[], json!([])).unwrap();
        for schema in [1, 3, 5, 99] {
            let mut state = saved(&initial);
            state.schema = schema;
            assert!(
                call(
                    None,
                    &serde_json::to_string(&state).unwrap(),
                    json!({}),
                    &[],
                    json!([])
                )
                .is_err()
            );
        }
        let loading = start(Provider::Bizhihui);
        let done = accept(&loading, Provider::Bizhihui, 0, 2);
        for invalid in ["0123456789abcdeg", "0123456789ABCDEF", "fabricated"] {
            let mut state = saved(&done);
            state.seen[0] = invalid.into();
            assert!(!valid_state(&state));
        }
        let mut state = saved(&done);
        state.seen.push(state.seen[0].clone());
        assert!(!valid_state(&state));
        let mut state = saved(&done);
        state.seen.clear();
        assert!(!valid_state(&state));
        let mut state = saved(&done);
        state.seen = (0..961).map(|id| format!("{id:016x}")).collect();
        assert!(!valid_state(&state));
        let mut state = saved(&done);
        state.items[0].result_id = "0000000000000000".into();
        assert!(!valid_state(&state));
        let mut state = saved(&done);
        state.provider = Provider::Goodfon;
        assert!(!valid_state(&state));
    }

    #[test]
    fn queries_are_trimmed_bounded_and_clear_categories_the_source_cannot_intersect() {
        for provider in [
            Provider::Bizhihui,
            Provider::Wallcoo,
            Provider::Wallpaperscraft,
            Provider::Goodfon,
        ] {
            let loading = call(
                Some("search"),
                "",
                json!({"provider":provider.id(),"query":"  mountain & lake  "}),
                &["network:https"],
                json!([]),
            )
            .unwrap();
            let pending = saved(&loading).pending.unwrap();
            assert_eq!(pending.query, "mountain & lake");
            assert_eq!(pending.category, "all");
        }
        let filtered = call(
            Some("search"),
            "",
            json!({"provider":"bizhihui","category":"fengjing","query":"lake"}),
            &["network:https"],
            json!([]),
        )
        .unwrap();
        assert_eq!(saved(&filtered).pending.unwrap().category, "all");
        for query in [
            "x".repeat(101),
            "😀".repeat(101),
            "\nforest".into(),
            "forest\u{0000}".into(),
        ] {
            assert!(
                call(
                    Some("search"),
                    "",
                    json!({"query":query}),
                    &["network:https"],
                    json!([])
                )
                .is_err()
            );
        }
        for fields in [
            json!({"provider":"upx8"}),
            json!({"provider":3}),
            json!({"category":"invalid"}),
        ] {
            assert!(call(Some("search"), "", fields, &["network:https"], json!([])).is_err());
        }
        assert!(
            call(
                Some("search"),
                "",
                json!({"query":"😀".repeat(100)}),
                &["network:https"],
                json!([])
            )
            .is_ok()
        );
        assert!(call(Some("unknown"), "", json!({}), &[], json!([])).is_err());
    }
}
