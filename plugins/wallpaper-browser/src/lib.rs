//! The HTML owns browsing UI; authorized native services own network, gallery and settings.
use serde_json::{Value, json};

pub fn process(source: &str) -> Result<String, String> {
    if source.len() > 1024 * 1024 {
        return Err("Wallpaper request exceeds its limit".into());
    }
    let request: Value = serde_json::from_str(source).map_err(|_| "Invalid wallpaper request")?;
    if request.get("version").and_then(Value::as_u64) != Some(2)
        || request
            .get("action")
            .is_some_and(|action| !action.is_null())
        || !request.get("fields").is_some_and(Value::is_object)
    {
        return Err("Unsupported wallpaper view request".into());
    }
    let english = request.get("locale").and_then(Value::as_str) == Some("en");
    Ok(json!({
        "version":2,"state":"{}","display":if english { "Wallpaper Library" } else { "壁纸库" },
        "description":if english { "Search Bing images and manage the application gallery. Image licenses are not filtered." } else { "搜索必应图片，管理应用图库；不按图片许可筛选。" },
        "fields":[],"groups":[],"tabs":[],"blocks":[],"errors":{},"effects":[],"artifacts":[]
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
    #[test]
    fn view_bootstrap_has_no_implicit_network_or_storage_effects() {
        let view: Value = serde_json::from_str(
            &process(r#"{"version":2,"action":null,"fields":{},"locale":"en"}"#).unwrap(),
        )
        .unwrap();
        assert_eq!(view["display"], "Wallpaper Library");
        assert_eq!(view["effects"], json!([]));
    }
    #[test]
    fn unknown_actions_and_invalid_input_are_rejected() {
        assert!(process(r#"{"version":2,"action":"download","fields":{}}"#).is_err());
        assert!(process(r#"{"version":3,"action":null,"fields":{}}"#).is_err());
        assert!(process("broken").is_err());
    }
}
