//! 界面通过原生 LAN 服务通信；Wasm 校验本地草稿并生成明确的纯文本会话导出。
#![cfg(any(target_arch = "wasm32", test))]
#[cfg(target_arch = "wasm32")]
use extism_pdk::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    version: u32,
    action: Option<String>,
    state: String,
    fields: BTreeMap<String, String>,
    locale: String,
    #[serde(default, rename = "host")]
    _host: Option<Value>,
    #[serde(default, rename = "results")]
    _results: Vec<Value>,
    #[serde(default, rename = "data")]
    _data: Option<Value>,
}

fn render(input: &str) -> Result<Value, String> {
    if input.len() > 1024 * 1024 {
        return Err("Chat view input exceeds its size limit".into());
    }
    let request: Request = serde_json::from_str(input).map_err(|_| "Invalid chat view request")?;
    if request.version != 2
        || request.state.len() > 256 * 1024
        || request.state.contains('\0')
        || !matches!(request.locale.as_str(), "zh-CN" | "en")
        || request
            .fields
            .keys()
            .any(|key| !matches!(key.as_str(), "nickname" | "draft" | "export"))
    {
        return Err("Unsupported chat view request".into());
    }
    if !request.state.is_empty() {
        let state: BTreeMap<String, Value> =
            serde_json::from_str(&request.state).map_err(|_| "Invalid chat view state")?;
        if !state.is_empty() {
            return Err("Unsupported chat view state; original data was preserved".into());
        }
    }
    let en = request.locale == "en";
    let field = |key: &str| request.fields.get(key).cloned().unwrap_or_default();
    let name = field("nickname");
    let draft = field("draft");
    let exported = field("export");
    if name.len() > 120
        || name.chars().count() > 40
        || name.chars().any(char::is_control)
        || draft.len() > 8192
        || unsafe_text(&draft)
        || exported.len() > 256 * 1024
        || unsafe_text(&exported)
    {
        return Err("Chat nickname, draft or export exceeds its limit".into());
    }
    let mut artifacts = vec![];
    if request.action.as_deref() == Some("export") {
        if exported.trim().is_empty() {
            return Err(if en {
                "Select a conversation with messages before exporting"
            } else {
                "请先选择有消息的会话再导出"
            }
            .into());
        }
        artifacts.push(json!({"name":"lan-chat.txt","mediaType":"text/plain","encoding":"utf8","content":exported}));
    } else if request.action.is_some() {
        return Err("Unsupported chat action".into());
    }
    Ok(json!({
        "version":2,"state":"{}","display":if en {"LAN Chat"} else {"内网通信"},
        "description":if en {"Exchange messages, images and files with trusted LAN devices. Traffic is not encrypted."} else {"与可信内网设备收发消息、图片和文件。通信采用明文传输。"},
        "fields":[
            {"id":"nickname","label":if en {"Nickname"} else {"昵称"},"value":name,"kind":"text","maxBytes":120},
            {"id":"draft","label":if en {"Draft"} else {"消息草稿"},"value":draft,"kind":"textarea","maxBytes":8192},
            {"id":"export","label":if en {"Conversation export"} else {"待导出的会话"},"value":exported,"kind":"textarea","maxBytes":262144}
        ],
        "groups":[{"columns":1,"actions":[{"id":"export","label":if en {"Prepare export"} else {"生成会话导出"},"disabled":false,"primary":false}]}],
        "blocks":[{"kind":"markdown","text":if en {"Open the custom chat interface to exchange messages, images and files. Connection settings and a real local loopback test are available in its menu. Exports retain attachment labels as plain text."} else {"在自定义聊天界面中收发消息、图片和文件，连接设置与真实本地回环测试收在菜单中。会话导出以纯文本保留附件说明。"}}],
        "effects":[],"artifacts":artifacts,"errors":{}
    }))
}

fn unsafe_text(value: &str) -> bool {
    value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

#[cfg(target_arch = "wasm32")]
#[plugin_fn]
pub fn chat(input: String) -> FnResult<String> {
    let view = render(&input).map_err(Error::msg)?;
    Ok(serde_json::to_string(&view)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(action: Option<&str>, fields: Value) -> String {
        json!({"version":2,"action":action,"state":"","fields":fields,"locale":"zh-CN"}).to_string()
    }
    #[test]
    fn bootstrap_requires_no_fake_peers_or_messages() {
        let view = render(&input(None, json!({}))).unwrap();
        assert_eq!(view["display"], "内网通信");
        assert_eq!(view["effects"], json!([]));
        assert_eq!(view["artifacts"], json!([]));
    }
    #[test]
    fn conversation_export_preserves_real_text_and_drafts() {
        let view = render(&input(
            Some("export"),
            json!({"nickname":"阿林","draft":"未发送内容","export":"[12:00] 阿周：你好 🌍\n"}),
        ))
        .unwrap();
        assert_eq!(view["artifacts"][0]["content"], "[12:00] 阿周：你好 🌍\n");
        assert_eq!(view["fields"][1]["value"], "未发送内容");
    }
    #[test]
    fn export_preserves_attachment_labels_without_embedding_file_data() {
        let text =
            "[12:00] 阿周：[图片] 假日.png (2048 B)\n[12:01] 阿林：[文件] 计划.txt (512 B)\n";
        let view = render(&input(Some("export"), json!({"export":text}))).unwrap();
        assert_eq!(view["artifacts"][0]["content"], text);
        assert_eq!(view["artifacts"][0]["mediaType"], "text/plain");
        assert_eq!(view["artifacts"][0]["encoding"], "utf8");
    }
    #[test]
    fn invalid_actions_names_and_oversized_exports_are_rejected() {
        for value in [
            input(Some("send"), json!({})),
            input(Some("export"), json!({})),
            input(None, json!({"nickname":"bad\nname"})),
            input(None, json!({"nickname":"🌍".repeat(31)})),
            input(None, json!({"draft":"bad\u{7}text"})),
            input(Some("export"), json!({"export":"bad\u{1b}text"})),
            input(None, json!({"draft":"x".repeat(8193)})),
            input(Some("export"), json!({"export":"x".repeat(262145)})),
        ] {
            assert!(render(&value).is_err());
        }
    }
    #[test]
    fn state_is_validated_without_resetting_future_or_invalid_data() {
        for state in ["not-json", "null", "[]", "{\"version\":3}"] {
            let mut request: Value = serde_json::from_str(&input(None, json!({}))).unwrap();
            request["state"] = state.into();
            assert!(render(&request.to_string()).is_err());
        }
        let mut request: Value = serde_json::from_str(&input(None, json!({}))).unwrap();
        request["state"] = "{}".into();
        assert!(render(&request.to_string()).is_ok());
    }
    #[test]
    fn export_preserves_literal_text_at_the_utf8_limit() {
        let prefix = "<script>literal & safe</script>\r\n\t界";
        let text = format!("{prefix}{}", "x".repeat(262_144 - prefix.len()));
        assert_eq!(text.len(), 262_144);
        let view = render(&input(Some("export"), json!({"export":text}))).unwrap();
        assert_eq!(view["artifacts"][0]["content"], text);
        assert_eq!(view["artifacts"][0]["mediaType"], "text/plain");
    }
}
