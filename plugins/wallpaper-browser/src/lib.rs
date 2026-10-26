//! 搜索来源、网页解析与结果状态归插件；网络、图片资源与图库由通用宿主能力核验。
mod bing;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{DefaultHasher, Hash, Hasher},
};
const MAX_STATE_BYTES: usize = 240_000;
const EFFECT_ID: &str = "bing-search";
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
    query: String,
    network_mode: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    schema: u8,
    query: String,
    network_mode: String,
    items: Vec<RemoteImage>,
    pending: Option<Pending>,
    error: String,
}
impl Default for State {
    fn default() -> Self {
        Self {
            schema: 1,
            query: "landscape".into(),
            network_mode: "direct".into(),
            items: Vec::new(),
            pending: None,
            error: String::new(),
        }
    }
}
fn plain(source: &str, maximum: usize) -> String {
    source
        .chars()
        .filter(|character| !character.is_control())
        .take(maximum)
        .collect()
}
fn valid_query(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= 100 && !value.chars().any(char::is_control)
}
fn valid_mode(value: &str) -> bool {
    matches!(value, "direct" | "system")
}
fn result_id(image: &RemoteImage) -> String {
    // 标识只用于插件会话内的列表身份，不是原生授权令牌。
    let mut hash = DefaultHasher::new();
    image.original_url.hash(&mut hash);
    image.preview_url.hash(&mut hash);
    image.source_url.hash(&mut hash);
    format!("{:016x}", hash.finish())
}
fn valid_state(state: &State) -> bool {
    let mut ids = BTreeSet::new();
    state.schema == 1
        && valid_query(&state.query)
        && valid_mode(&state.network_mode)
        && state.items.len() <= 12
        && state.error.len() <= 120
        && state
            .pending
            .as_ref()
            .is_none_or(|pending| valid_query(&pending.query) && valid_mode(&pending.network_mode))
        && state.items.iter().all(|image| {
            image.result_id == result_id(image)
                && ids.insert(&image.result_id)
                && image.title.chars().count() <= 120
                && !image.title.chars().any(char::is_control)
                && image.width <= 30_000
                && image.height <= 30_000
                && bing::approved_public_url(&image.original_url).is_ok()
                && bing::approved_public_url(&image.source_url).is_ok()
                && bing::approved_thumbnail_url(&image.preview_url).is_ok()
        })
}
fn tr<'a>(english: bool, zh: &'a str, en: &'a str) -> &'a str {
    if english { en } else { zh }
}
pub fn process(source: &str) -> Result<String, String> {
    if source.len() > 4 * 1024 * 1024 {
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
        || fields.len() > 2
        || fields.iter().any(|(key, value)| {
            !matches!(key.as_str(), "query" | "networkMode") || !value.is_string()
        })
    {
        return Err("Unsupported wallpaper view request".into());
    }
    let mut state: State = if encoded.is_empty() {
        State::default()
    } else {
        serde_json::from_str(encoded).map_err(|_| "Invalid wallpaper session; original retained")?
    };
    if !valid_state(&state) {
        return Err("Invalid wallpaper session; original retained".into());
    }
    let query = fields
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or(&state.query)
        .trim()
        .to_owned();
    let mode = fields
        .get("networkMode")
        .and_then(Value::as_str)
        .unwrap_or(&state.network_mode)
        .to_owned();
    if query.len() > 400 || query.chars().any(char::is_control) || !valid_mode(&mode) {
        return Err("Invalid wallpaper fields".into());
    }
    let permissions = request
        .pointer("/host/permissions")
        .and_then(Value::as_array);
    let granted = |permission: &str| {
        permissions.is_some_and(|values| {
            values
                .iter()
                .any(|value| value.as_str() == Some(permission))
        })
    };
    let results = request.get("results").and_then(Value::as_array);
    if results.is_some_and(|values| {
        values.len() > 1 || (action != Some("host-result") && !values.is_empty())
    }) {
        return Err("Unexpected search receipt".into());
    }
    let mut effects = Vec::new();
    match action {
        None => {}
        Some("search") => {
            // 用户的新搜索可替换恢复会话中的中断意图；不取消宿主实际在途的工作。
            state.pending = None;
            state.error.clear();
            if !valid_query(&query) {
                state.error = "INVALID_QUERY".into();
            } else if !granted("network:https") {
                state.error = "NETWORK_PERMISSION".into();
            } else if mode == "system" && !granted("network:system-proxy") {
                state.error = "PROXY_PERMISSION".into();
            } else {
                state.pending = Some(Pending {
                    query: query.clone(),
                    network_mode: mode.clone(),
                });
                effects.push(json!({"kind":"http","id":EFFECT_ID,"request":{"url":bing::search_url(&query),"method":"GET","headers":{"Accept":"text/html","User-Agent":"TauriDesktopStarterWallpaper/1.0 (https://github.com/YanChao001/tauri-desktop-starter-extensions)"},"body":null,"credential":null,"networkMode":mode}}));
            }
        }
        Some("host-result") => {
            let pending = state.pending.take().ok_or("Unexpected search receipt")?;
            let receipt = results
                .and_then(|values| values.first())
                .ok_or("Missing search receipt")?;
            let value = receipt.get("value").filter(|value| !value.is_null());
            let error = receipt.get("error").filter(|value| !value.is_null());
            if receipt.get("id").and_then(Value::as_str) != Some(EFFECT_ID)
                || value.is_some() == error.is_some()
            {
                return Err("Invalid search receipt".into());
            }
            if let Some(error) = error {
                state.error = error
                    .get("code")
                    .and_then(Value::as_str)
                    .filter(|code| {
                        !code.is_empty()
                            && code.len() <= 120
                            && code
                                .bytes()
                                .all(|byte| byte.is_ascii_uppercase() || byte == b'_')
                    })
                    .unwrap_or("SEARCH_FAILED")
                    .into();
            } else if let Some(value) = value {
                if value.get("url").and_then(Value::as_str)
                    != Some(bing::search_url(&pending.query).as_str())
                    || value.get("status").and_then(Value::as_u64) != Some(200)
                {
                    state.error = "SEARCH_RESPONSE".into();
                } else if let Some(body) = value.get("body").and_then(Value::as_str) {
                    match bing::parse_images(body.as_bytes(), &pending.query) {
                        Ok(mut items) => {
                            for image in &mut items {
                                image.result_id = result_id(image);
                            }
                            state.items = items;
                            state.query = pending.query;
                            state.network_mode = pending.network_mode;
                            state.error.clear();
                        }
                        Err(_) => state.error = "SEARCH_RESPONSE".into(),
                    }
                } else {
                    state.error = "SEARCH_RESPONSE".into();
                }
            }
        }
        _ => return Err("Unsupported wallpaper action".into()),
    }
    let mut errors = BTreeMap::new();
    if !state.error.is_empty() {
        errors.insert("query", match state.error.as_str() {
            "INVALID_QUERY" => tr(english, "搜索关键词需为 1–100 个字符，且不含控制字符。", "Enter 1–100 search characters without control characters."),
            "NETWORK_PERMISSION" => tr(english, "请授予 HTTPS 网络权限以搜索图片；本机图库仍可使用。", "Grant HTTPS network access to search images. The local gallery remains available."),
            "PROXY_PERMISSION" => tr(english, "系统代理需要单独授权，请授权或选择直接连接。", "System proxy access needs its own grant. Grant it or choose a direct connection."),
            _ => tr(english, "搜索失败，请检查网络连接或更换关键词后重试。", "Image search failed. Check your connection or try another keyword."),
        });
    }
    let encoded = serde_json::to_string(&state).map_err(|_| "Cannot encode wallpaper session")?;
    if encoded.len() > MAX_STATE_BYTES {
        return Err("Wallpaper session exceeds its limit".into());
    }
    Ok(json!({
        "version":2,"state":encoded,"display":tr(english,"壁纸库","Wallpaper Library"),
        "description":tr(english,"搜索必应图片，管理应用图库；不按图片许可筛选。","Search Bing images and manage the application gallery. Image licenses are not filtered."),
        "fields":[{"id":"query","label":tr(english,"搜索关键词","Search query"),"value":query,"maxBytes":400},{"id":"networkMode","label":tr(english,"网络连接","Network connection"),"kind":"select","value":mode,"options":[{"value":"direct","label":tr(english,"直接连接","Direct connection")},{"value":"system","label":tr(english,"跟随系统代理","Follow system proxy")}]}],
        "groups":[],"tabs":[],"blocks":[{"kind":"metrics","items":[{"label":tr(english,"图片","Images"),"value":state.items.len().to_string()}]}],"errors":errors,"effects":effects,"artifacts":[]
    }).to_string())
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
    fn markup(query: &str) -> String {
        format!(
            "<input id='sb_form_q' value='{query}'><a class='iusc' href='/images/search?q={query}&amp;expw=1920&amp;exph=1080' m='{{\"murl\":\"https://images.wikipedia.org/one.jpg\",\"purl\":\"https://www.wikipedia.org/one\",\"turl\":\"https://ts2.mm.bing.net/th?id=OIP.test\",\"t\":\"Mountain\"}}'>image</a>"
        )
    }
    fn receipt(query: &str) -> Value {
        json!([{"id":EFFECT_ID,"value":{"url":bing::search_url(query),"status":200,"headers":{},"body":markup(query)},"error":null}])
    }
    #[test]
    fn bootstrap_and_locale_render_do_not_run_network() {
        let initial = call(None, "", json!({}), &[], json!([])).unwrap();
        assert_eq!(initial["display"], "Wallpaper Library");
        assert_eq!(initial["effects"], json!([]));
        let next = call(
            None,
            initial["state"].as_str().unwrap(),
            json!({"query":"unsent"}),
            &[],
            json!([]),
        )
        .unwrap();
        assert_eq!(initial["state"], next["state"]);
        assert_eq!(next["fields"][0]["value"], "unsent");
    }
    #[test]
    fn search_uses_declared_https_effect_and_matching_receipt() {
        let first = call(
            Some("search"),
            "",
            json!({"query":"landscape","networkMode":"direct"}),
            &["network:https"],
            json!([]),
        )
        .unwrap();
        assert_eq!(first["effects"][0]["kind"], "http");
        assert_eq!(first["effects"][0]["request"]["networkMode"], "direct");
        let done = call(
            Some("host-result"),
            first["state"].as_str().unwrap(),
            json!({}),
            &["network:https"],
            receipt("landscape"),
        )
        .unwrap();
        let state: State = serde_json::from_str(done["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.items.len(), 1);
        assert_eq!(state.items[0].result_id, result_id(&state.items[0]));
        assert!(state.pending.is_none());
        assert_eq!(done["effects"], json!([]));
        let rendered = call(
            None,
            done["state"].as_str().unwrap(),
            json!({}),
            &[],
            json!([]),
        )
        .unwrap();
        assert_eq!(rendered["state"], done["state"]);
        assert!(call(Some("host-result"), "", json!({}), &[], json!([])).is_err());
    }
    #[test]
    fn denied_network_or_proxy_keeps_offline_view_available() {
        for (permissions, mode) in [(vec![], "direct"), (vec!["network:https"], "system")] {
            let result = call(
                Some("search"),
                "",
                json!({"query":"landscape","networkMode":mode}),
                &permissions,
                json!([]),
            )
            .unwrap();
            assert_eq!(result["effects"], json!([]));
            assert!(result["errors"]["query"].as_str().is_some());
        }
        let system = call(
            Some("search"),
            "",
            json!({"networkMode":"system"}),
            &["network:https", "network:system-proxy"],
            json!([]),
        )
        .unwrap();
        assert_eq!(system["effects"][0]["request"]["networkMode"], "system");
    }
    #[test]
    fn failure_preserves_previous_results_and_does_not_retry() {
        let pending = call(Some("search"), "", json!({}), &["network:https"], json!([])).unwrap();
        let completed = call(
            Some("host-result"),
            pending["state"].as_str().unwrap(),
            json!({}),
            &["network:https"],
            receipt("landscape"),
        )
        .unwrap();
        let next = call(
            Some("search"),
            completed["state"].as_str().unwrap(),
            json!({"query":"ocean"}),
            &["network:https"],
            json!([]),
        )
        .unwrap();
        let failed = call(
            Some("host-result"),
            next["state"].as_str().unwrap(),
            json!({}),
            &["network:https"],
            json!([{"id":EFFECT_ID,"value":null,"error":{"code":"EXTENSION_TIMEOUT"}}]),
        )
        .unwrap();
        let state: State = serde_json::from_str(failed["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.query, "landscape");
        assert_eq!(state.items.len(), 1);
        assert_eq!(failed["effects"], json!([]));
        assert!(failed["errors"]["query"].as_str().is_some());
    }
    #[test]
    fn malformed_receipts_future_state_and_arbitrary_actions_are_rejected() {
        assert!(call(Some("download"), "", json!({}), &[], json!([])).is_err());
        assert!(call(None, "{\"schema\":99}", json!({}), &[], json!([])).is_err());
        let pending = call(Some("search"), "", json!({}), &["network:https"], json!([])).unwrap();
        for receipt in [
            json!({"id":"wrong","value":{},"error":null}),
            json!({"id":EFFECT_ID,"value":{},"error":{}}),
        ] {
            assert!(
                call(
                    Some("host-result"),
                    pending["state"].as_str().unwrap(),
                    json!({}),
                    &["network:https"],
                    json!([receipt])
                )
                .is_err()
            );
        }
    }
    #[test]
    fn an_explicit_search_replaces_an_interrupted_restored_intent_without_implicit_requests() {
        let first = call(Some("search"), "", json!({}), &["network:https"], json!([])).unwrap();
        let rendered = call(
            None,
            first["state"].as_str().unwrap(),
            json!({}),
            &[],
            json!([]),
        )
        .unwrap();
        assert_eq!(rendered["state"], first["state"]);
        assert_eq!(rendered["effects"], json!([]));
        let next = call(
            Some("search"),
            first["state"].as_str().unwrap(),
            json!({"query":"ocean"}),
            &["network:https"],
            json!([]),
        )
        .unwrap();
        assert_eq!(
            next["effects"][0]["request"]["url"],
            bing::search_url("ocean")
        );
        assert_eq!(
            next["effects"][0]["request"]["headers"]["Accept"],
            "text/html"
        );
        let state: State = serde_json::from_str(next["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.pending.unwrap().query, "ocean");
    }
}
