//! Offline text operations. The host supplies capabilities and renders view 2.
#![cfg(any(target_arch = "wasm32", test))]
#[cfg(target_arch = "wasm32")]
use extism_pdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

const INPUT_LIMIT: usize = 32 * 1024;
const OUTPUT_LIMIT: usize = 96 * 1024;
const TEXT_COMMAND_LIMIT: usize = 1024 * 1024;
const MODES: [(&str, &str, &str); 14] = [
    ("original", "原文 / 仅统计", "Original / statistics"),
    ("upper", "转为大写", "Uppercase"),
    ("lower", "转为小写", "Lowercase"),
    ("trim", "清理首尾空白", "Trim outer whitespace"),
    ("trim-lines", "清理每行首尾空白", "Trim every line"),
    ("remove-empty", "移除空白行", "Remove blank lines"),
    (
        "deduplicate",
        "行去重（保留首次出现）",
        "Unique lines / preserve first",
    ),
    (
        "sort",
        "按 Unicode 标量排序行",
        "Sort lines by Unicode scalar",
    ),
    ("reverse-sort", "反向排序行", "Sort lines descending"),
    ("reverse-lines", "反转行顺序", "Reverse line order"),
    ("normalize", "将空白统一为单个空格", "Normalize whitespace"),
    ("number-lines", "为每行编号", "Number every line"),
    ("quote-lines", "为每行添加引用符", "Quote every line"),
    ("unix-lines", "统一为 LF 换行", "Normalize CRLF / CR to LF"),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    version: u32,
    action: Option<String>,
    state: String,
    fields: BTreeMap<String, String>,
    locale: String,
    #[serde(default)]
    host: Option<Host>,
    #[serde(default)]
    results: Vec<Value>,
    #[serde(default)]
    data: Option<Data>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Host {
    permissions: Vec<String>,
    #[serde(rename = "storage")]
    _storage: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Data {
    api_version: u32,
    input_text: String,
    files: Vec<Value>,
    network: Vec<Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: u32,
}

fn validate_text(text: &str, limit: usize) -> Result<(), String> {
    if text.len() > limit || text.contains('\0') {
        return Err(format!("Text exceeds {limit} UTF-8 bytes or contains NUL"));
    }
    Ok(())
}

fn uppercase_text(input: &str) -> Result<String, String> {
    validate_text(input, TEXT_COMMAND_LIMIT)?;
    Ok(input.to_uppercase())
}

fn lowercase_text(input: &str) -> Result<String, String> {
    validate_text(input, TEXT_COMMAND_LIMIT)?;
    Ok(input.to_lowercase())
}

/// Split logical lines, including a final empty line and standalone CR endings.
fn logical_lines(input: &str) -> Vec<String> {
    if input.is_empty() {
        return Vec::new();
    }
    input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(str::to_owned)
        .collect()
}

fn transform(input: &str, mode: &str) -> Result<String, String> {
    validate_text(input, INPUT_LIMIT)?;
    let lines = logical_lines(input);
    let output = match mode {
        "original" => input.into(),
        "upper" => input.to_uppercase(),
        "lower" => input.to_lowercase(),
        "trim" => input.trim().into(),
        "trim-lines" => lines
            .iter()
            .map(|line| line.trim())
            .collect::<Vec<_>>()
            .join("\n"),
        "remove-empty" => lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
        "deduplicate" => {
            let mut seen = HashSet::new();
            lines
                .iter()
                .filter(|line| seen.insert(line.as_str()))
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        }
        "sort" | "reverse-sort" => {
            let mut lines = lines;
            lines.sort();
            if mode == "reverse-sort" {
                lines.reverse();
            }
            lines.join("\n")
        }
        "reverse-lines" => lines.into_iter().rev().collect::<Vec<_>>().join("\n"),
        "normalize" => input.split_whitespace().collect::<Vec<_>>().join(" "),
        "number-lines" => lines
            .iter()
            .enumerate()
            .map(|(index, line)| format!("{}. {line}", index + 1))
            .collect::<Vec<_>>()
            .join("\n"),
        "quote-lines" => lines
            .iter()
            .map(|line| format!("> {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
        "unix-lines" => lines.join("\n"),
        _ => return Err("Unknown text operation".into()),
    };
    if output.len() > OUTPUT_LIMIT {
        return Err("Result exceeds 98304 UTF-8 bytes; use a smaller input".into());
    }
    Ok(output)
}

// Indented code keeps user Markdown, HTML and fence strings literal.
fn literal_markdown(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn button(id: &str, label: String, primary: bool) -> Value {
    json!({"id":id,"label":label,"disabled":false,"primary":primary})
}

fn workshop_view(input: &str) -> Result<Value, String> {
    validate_text(input, 4 * 1024 * 1024)?;
    let request: Request = serde_json::from_str(input)
        .map_err(|error| format!("Invalid workshop request: {error}"))?;
    if request.version != 2
        || !["zh-CN", "en"].contains(&request.locale.as_str())
        || !request.results.is_empty()
    {
        return Err("Unsupported workshop request version, locale or effects".into());
    }
    let zh = request.locale == "zh-CN";
    let tr = |cn: &str, en: &str| if zh { cn.to_owned() } else { en.to_owned() };
    let initial = request.state.is_empty();
    if initial {
        if request.action.is_some() || !request.fields.is_empty() || request.data.is_some() {
            return Err("The initial request must have empty state, fields and action".into());
        }
    } else {
        if request.state.len() > 128 {
            return Err("Invalid workshop state size".into());
        }
        let state: State =
            serde_json::from_str(&request.state).map_err(|_| "Invalid workshop state")?;
        if state.schema != 1
            || request.fields.len() != 2
            || !request.fields.contains_key("text")
            || !request.fields.contains_key("mode")
        {
            return Err("Invalid workshop state or fields".into());
        }
    }
    let action = request.action.as_deref().unwrap_or("apply");
    if ![
        "apply",
        "use-result",
        "clear",
        "demo",
        "export-txt",
        "export-md",
        "host-data",
    ]
    .contains(&action)
    {
        return Err("Unknown workshop action".into());
    }
    if request.data.is_some() != (action == "host-data") {
        return Err("Supplied text is only accepted for host-data".into());
    }
    let mut text = request.fields.get("text").cloned().unwrap_or_else(|| {
        tr(
            "  Hello Rust!  \n你好，扩展。\nHello Rust!\n\n",
            "  Hello Rust!  \nHello reusable plugins.\nHello Rust!\n\n",
        )
    });
    let mut mode = request
        .fields
        .get("mode")
        .cloned()
        .unwrap_or_else(|| "trim-lines".into());
    if !MODES.iter().any(|(id, _, _)| *id == mode) {
        return Err("Unknown workshop operation field".into());
    }
    validate_text(&text, INPUT_LIMIT)?;
    if let Some(data) = request.data {
        let permitted = request.host.as_ref().is_some_and(|host| {
            host.permissions
                .iter()
                .any(|permission| permission == "text:read")
        });
        if !permitted || data.api_version != 2 || !data.files.is_empty() || !data.network.is_empty()
        {
            return Err("Workshop accepts only explicitly authorized host text".into());
        }
        validate_text(&data.input_text, INPUT_LIMIT)?;
        text = data.input_text;
    }
    match action {
        "clear" => text.clear(),
        "demo" => {
            text = tr(
                "  苹果  \nbanana\n苹果\n\n  Banana  \nStraße 😀\n",
                "  apple  \nbanana\napple\n\n  Banana  \nStraße 😀\n",
            )
        }
        _ => {}
    }
    let mut errors = BTreeMap::<String, String>::new();
    let mut result = match transform(&text, &mode) {
        Ok(result) => result,
        Err(_) => {
            errors.insert(
                "text".into(),
                tr(
                    "转换结果超过 96 KiB，请缩小输入；输入已完整保留。",
                    "Result exceeds 96 KiB. Reduce the input; your complete input is preserved.",
                ),
            );
            String::new()
        }
    };
    if action == "use-result" && errors.is_empty() {
        if result.len() > INPUT_LIMIT {
            errors.insert("text".into(), tr("结果超过输入框的 32 KiB 上限，可直接导出或缩小输入。", "Result exceeds the 32 KiB input limit. Export it directly or reduce the input."));
        } else {
            text = result;
            mode = "original".into();
            result = text.clone();
        }
    }
    let mut artifacts = Vec::new();
    if errors.is_empty() && ["export-txt", "export-md"].contains(&action) {
        let markdown = action == "export-md";
        artifacts.push(json!({"name":if markdown {"text-workshop.md"} else {"text-workshop.txt"},"mediaType":if markdown {"text/markdown"} else {"text/plain"},"encoding":"utf8","content":if markdown {literal_markdown(&result)} else {result.clone()}}));
    }
    let metrics = json!({"kind":"metrics","items":[
        {"label":tr("Unicode 标量（不等于视觉字素）", "Unicode scalars / not graphemes"),"value":text.chars().count().to_string()},
        {"label":"UTF-8 bytes","value":text.len().to_string()},
        {"label":tr("逻辑行（包含末尾空行）", "Logical lines / includes trailing empty"),"value":logical_lines(&text).len().to_string()},
        {"label":tr("空白分隔词（中文不分词）", "Whitespace words / no CJK segmentation"),"value":text.split_whitespace().count().to_string()},
        {"label":tr("结果 UTF-8 字节", "Result UTF-8 bytes"),"value":result.len().to_string()}
    ]});
    Ok(json!({
        "version":2,"state":serde_json::to_string(&State{schema:1}).map_err(|_|"Cannot encode state")?,
        "display":tr("文本工坊", "Text workshop"),
        "description":tr("完全离线；输入最多 32 KiB。选择操作后点击处理，原文不会自动覆盖；行操作统一为 LF。", "Fully offline; input up to 32 KiB. Choose an operation and apply it. The original stays editable; line operations use LF."),
        "fields":[
            {"id":"text","label":tr("输入文本", "Input text"),"value":text,"kind":"textarea","maxBytes":INPUT_LIMIT},
            {"id":"mode","label":tr("转换操作", "Operation"),"value":mode,"kind":"select","options":MODES.iter().map(|(id,cn,en)|json!({"value":id,"label":tr(cn,en)})).collect::<Vec<_>>()}
        ],
        "groups":[{"columns":3,"actions":[button("apply",tr("处理 / 统计", "Apply / analyze"),true),button("use-result",tr("将结果作为输入", "Use result as input"),false),button("clear",tr("清空", "Clear"),false),button("demo",tr("填入示例", "Load sample"),false),button("export-txt",tr("生成 TXT", "Generate TXT"),false),button("export-md",tr("生成 Markdown", "Generate Markdown"),false)]}],
        "blocks":[metrics,{"kind":"markdown","text":format!("### {}\n\n{}",tr("转换结果", "Transformed result"), if result.is_empty() {tr("（空文本）", "(Empty text)")} else {literal_markdown(&result)})}],
        "errors":errors,"effects":[],"artifacts":artifacts
    }))
}

#[cfg(target_arch = "wasm32")]
#[plugin_fn]
pub fn uppercase(input: String) -> FnResult<String> {
    uppercase_text(&input).map_err(|error| Error::msg(error).into())
}

#[cfg(target_arch = "wasm32")]
#[plugin_fn]
pub fn lowercase(input: String) -> FnResult<String> {
    lowercase_text(&input).map_err(|error| Error::msg(error).into())
}

#[cfg(target_arch = "wasm32")]
#[plugin_fn]
pub fn workshop(input: String) -> FnResult<String> {
    let view = workshop_view(&input).map_err(Error::msg)?;
    Ok(serde_json::to_string(&view)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(action: &str, text: &str, mode: &str) -> Value {
        json!({"version":2,"action":action,"state":"{\"schema\":1}","fields":{"text":text,"mode":mode},"locale":"en"})
    }
    fn view(value: Value) -> Value {
        workshop_view(&value.to_string()).unwrap()
    }

    #[test]
    fn unicode_conversion_and_metrics_are_explicit() {
        assert_eq!(
            uppercase_text("Straße e\u{301} 😀 中文").unwrap(),
            "STRASSE E\u{301} 😀 中文"
        );
        assert_eq!(lowercase_text("İΣ中文").unwrap(), "i\u{307}ς中文");
        let output = view(request("apply", "e\u{301} 😀\r\n中文\n", "original"));
        let items = &output["blocks"][0]["items"];
        assert_eq!(items[0]["value"], "9");
        assert_eq!(items[1]["value"], "17");
        assert_eq!(items[2]["value"], "3");
        assert_eq!(items[3]["value"], "3");
    }

    #[test]
    fn line_operations_preserve_defined_empty_and_unicode_semantics() {
        let cases = [
            ("trim", " a \n", "a"),
            ("trim-lines", " a \r\n b \r", "a\nb\n"),
            ("remove-empty", "a\n \n\tb\n", "a\n\tb"),
            ("deduplicate", "b\na\nb\n\n", "b\na\n"),
            ("sort", "中\na\n😀", "a\n中\n😀"),
            ("reverse-sort", "a\nb", "b\na"),
            ("reverse-lines", "a\nb\n", "\nb\na"),
            ("normalize", " a\t\u{2003}b\n c ", "a b c"),
            ("number-lines", "a\nb", "1. a\n2. b"),
            ("quote-lines", "a\nb", "> a\n> b"),
            ("unix-lines", "a\r\nb\rc\n", "a\nb\nc\n"),
        ];
        for (mode, input, expected) in cases {
            assert_eq!(transform(input, mode).unwrap(), expected, "{mode}");
        }
        assert_eq!(transform("", "number-lines").unwrap(), "");
    }

    #[test]
    fn unknown_actions_states_fields_and_versions_are_rejected() {
        for value in [
            {
                let mut v = request("execute-shell", "x", "upper");
                v["fields"]["extra"] = json!("x");
                v
            },
            request("apply", "x", "unknown"),
            {
                let mut v = request("apply", "x", "upper");
                v["state"] = json!("{\"schema\":2}");
                v
            },
            {
                let mut v = request("apply", "x", "upper");
                v["state"] = json!("{\"schema\":1,\"extra\":true}");
                v
            },
            {
                let mut v = request("apply", "x", "upper");
                v["version"] = json!(1);
                v
            },
            {
                let mut v = request("apply", "x", "upper");
                v["fields"] = json!({"text":"x"});
                v
            },
            {
                let mut v = request("apply", "x", "upper");
                v["locale"] = json!("unknown");
                v
            },
        ] {
            assert!(workshop_view(&value.to_string()).is_err());
        }
    }

    #[test]
    fn overlong_inputs_are_never_truncated_and_expansion_reports_field_error() {
        assert!(
            workshop_view(&request("apply", &"x".repeat(INPUT_LIMIT + 1), "original").to_string())
                .is_err()
        );
        assert!(workshop_view(&request("apply", "contains\0NUL", "original").to_string()).is_err());
        let source = "\n".repeat(INPUT_LIMIT);
        let output = view(request("apply", &source, "number-lines"));
        assert_eq!(output["fields"][0]["value"], source);
        assert!(!output["errors"]["text"].as_str().unwrap().is_empty());
        assert!(output["artifacts"].as_array().unwrap().is_empty());
    }

    #[test]
    fn exports_preserve_complete_literal_content_and_safe_names() {
        let source = "```\n<script>unsafe()</script>\n# heading\n";
        let output = view(request("export-txt", source, "original"));
        assert_eq!(output["artifacts"][0]["content"], source);
        assert_eq!(output["artifacts"][0]["name"], "text-workshop.txt");
        let markdown = view(request("export-md", source, "original"));
        assert_eq!(
            markdown["artifacts"][0]["content"],
            literal_markdown(source)
        );
        assert!(markdown["state"].as_str().unwrap().len() < 128);
    }

    #[test]
    fn host_text_requires_explicit_grant_and_correct_envelope() {
        let mut value = request("host-data", "draft", "upper");
        value["data"] = json!({"apiVersion":2,"inputText":"Straße", "files":[],"network":[]});
        assert!(workshop_view(&value.to_string()).is_err());
        value["host"] = json!({"permissions":["text:read"],"storage":null});
        assert_eq!(view(value.clone())["fields"][0]["value"], "Straße");
        value["data"]["files"] = json!([{"name":"secret.txt"}]);
        assert!(workshop_view(&value.to_string()).is_err());
    }

    #[test]
    fn checked_in_wasm_fixtures_match_native_contract() {
        let fixtures: Vec<Value> = serde_json::from_str(include_str!("../smoke.json")).unwrap();
        for fixture in fixtures {
            let input = &fixture["input"];
            match fixture["command"].as_str().unwrap() {
                "uppercase" => assert_eq!(
                    uppercase_text(input.as_str().unwrap()).unwrap(),
                    fixture["expectOutput"]
                ),
                "lowercase" => assert_eq!(
                    lowercase_text(input.as_str().unwrap()).unwrap(),
                    fixture["expectOutput"]
                ),
                "workshop" if fixture.get("expectError").is_some() => {
                    assert!(workshop_view(&input.to_string()).is_err())
                }
                "workshop" => {
                    let actual = view(input.clone());
                    assert_eq!(actual["version"], 2);
                    assert_eq!(actual["effects"], json!([]));
                    assert!(actual["errors"].as_object().unwrap().is_empty());
                    if let Some(expected) = fixture["expectJsonSubset"].get("artifacts") {
                        assert_eq!(actual["artifacts"], *expected);
                    }
                    if input["action"] == "apply" {
                        assert_eq!(
                            actual["blocks"][1]["text"],
                            "### Transformed result\n\n    a\n    中\n    😀\n"
                        );
                    }
                    if input["action"] == "host-data" {
                        assert_eq!(actual["fields"][0]["value"], "Straße");
                        assert_eq!(
                            actual["blocks"][1]["text"],
                            "### Transformed result\n\n    STRASSE\n"
                        );
                    }
                }
                _ => panic!("Unknown fixture command"),
            }
        }
    }

    #[test]
    fn initialization_clear_and_chinese_view_are_self_contained() {
        let initial =
            view(json!({"version":2,"action":null,"state":"","fields":{},"locale":"zh-CN"}));
        assert_eq!(initial["display"], "文本工坊");
        assert!(
            initial["fields"][0]["value"]
                .as_str()
                .unwrap()
                .contains("你好")
        );
        assert_eq!(
            view(request("clear", "draft", "original"))["fields"][0]["value"],
            ""
        );
        assert_eq!(
            view(request("use-result", "banana\napple", "sort"))["fields"][0]["value"],
            "apple\nbanana"
        );
    }
}
