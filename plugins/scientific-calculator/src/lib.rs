//! Scientific computation is performed in Wasm; HTML only renders and navigates results.
#[cfg(target_arch = "wasm32")]
use extism_pdk::*;
mod math;
use math::{Angle, Vars};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const STORAGE_KEY: &str = "calculator-workspace";
const MAX_HISTORY: usize = 32;
const SAMPLES: usize = 640;

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
    results: Vec<Receipt>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Host {
    permissions: Vec<String>,
    storage: Option<Storage>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Storage {
    revision: i32,
    values: BTreeMap<String, Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    id: String,
    value: Option<Value>,
    error: Option<Value>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    expression: String,
    value: f64,
    angle: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: u8,
    ans: f64,
    memory: f64,
    history: Vec<History>,
    result: String,
    notice: String,
    fields: BTreeMap<String, String>,
    pending: bool,
    #[serde(default, deserialize_with = "ignore_plot")]
    plot: Plot,
}
fn ignore_plot<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Plot, D::Error> {
    let _ = serde::de::IgnoredAny::deserialize(deserializer)?;
    Ok(Plot::default())
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct Plot {
    series: Vec<Series>,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
}
#[derive(Clone, Serialize, Deserialize)]
struct Series {
    expression: String,
    points: Vec<(f64, Option<f64>)>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            schema: 1,
            ans: 0.0,
            memory: 0.0,
            history: Vec::new(),
            result: "0".into(),
            notice: String::new(),
            fields: BTreeMap::from([
                ("expression".into(), "sin(pi / 6)^2 + cos(pi / 6)^2".into()),
                ("angle".into(), "rad".into()),
                ("functions".into(), "sin(x)\ncos(x)\nx^2 / 10".into()),
                ("xmin".into(), "-10".into()),
                ("xmax".into(), "10".into()),
                ("ymin".into(), "-3".into()),
                ("ymax".into(), "3".into()),
                ("a".into(), "0".into()),
                ("b".into(), "3.141592653589793".into()),
                ("curve".into(), "0".into()),
            ]),
            pending: false,
            plot: Plot::default(),
        }
    }
}
fn tr<'a>(zh: bool, cn: &'a str, en: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn bounded(value: &str, max: usize) -> bool {
    value.len() <= max && !value.contains('\0')
}
fn valid_state(state: &State) -> bool {
    state.schema == 1
        && state.ans.is_finite()
        && state.memory.is_finite()
        && bounded(&state.result, 240)
        && bounded(&state.notice, 600)
        && state.history.len() <= MAX_HISTORY
        && state.history.iter().all(|entry| {
            bounded(&entry.expression, 4352)
                && entry.value.is_finite()
                && ["rad", "deg"].contains(&entry.angle.as_str())
        })
        && state.fields.len() == 10
        && State::default().fields.keys().all(|key| {
            state.fields.get(key).is_some_and(|value| {
                bounded(
                    value,
                    if ["functions", "expression"].contains(&key.as_str()) {
                        4096
                    } else {
                        80
                    },
                )
            })
        })
        && ["rad", "deg"].contains(&state.fields["angle"].as_str())
        && ["0", "1", "2"].contains(&state.fields["curve"].as_str())
}
fn vars(state: &State) -> Result<Vars, String> {
    let angle = match state.fields["angle"].as_str() {
        "rad" => Angle::Rad,
        "deg" => Angle::Deg,
        _ => return Err("Angle mode must be rad or deg".into()),
    };
    Ok(Vars {
        x: 0.0,
        ans: state.ans,
        mem: state.memory,
        angle,
    })
}
fn number(state: &State, key: &str) -> Result<f64, String> {
    let value: f64 = state.fields[key]
        .trim()
        .parse()
        .map_err(|_| format!("{key}: enter a finite number"))?;
    if !value.is_finite() || value.abs() > 1e12 {
        return Err(format!("{key}: use a finite value within ±1e12"));
    }
    Ok(value)
}
fn functions(state: &State) -> Result<Vec<&str>, String> {
    let values: Vec<_> = state.fields["functions"]
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if values.is_empty() || values.len() > 3 {
        return Err("Enter between one and three functions, one per line".into());
    }
    Ok(values)
}
fn plot(state: &State) -> Result<Plot, String> {
    let (xmin, xmax, ymin, ymax) = (
        number(state, "xmin")?,
        number(state, "xmax")?,
        number(state, "ymin")?,
        number(state, "ymax")?,
    );
    if xmax <= xmin || ymax <= ymin || xmax - xmin < 1e-10 || ymax - ymin < 1e-10 {
        return Err("Graph ranges must increase and span at least 1e-10".into());
    }
    let base = vars(state)?;
    let mut series = Vec::new();
    for source in functions(state)? {
        let expression = math::compile(source)?;
        let mut previous: Option<f64> = None;
        let points = (0..=SAMPLES)
            .map(|index| {
                let x = xmin + (xmax - xmin) * index as f64 / SAMPLES as f64;
                let mut value = expression
                    .eval(Vars { x, ..base })
                    .ok()
                    .filter(|v| v.abs() <= 1e100);
                // Do not draw vertical bridges across sampled asymptotes.
                if let (Some(before), Some(y)) = (previous, value)
                    && (y - before).abs() > (ymax - ymin) * 8.0
                {
                    value = None;
                }
                previous = value;
                (x, value)
            })
            .collect();
        series.push(Series {
            expression: source.into(),
            points,
        });
    }
    Ok(Plot {
        series,
        xmin,
        xmax,
        ymin,
        ymax,
    })
}
fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if value.abs() >= 1e12 || value.abs() < 1e-8 {
        return format!("{value:.12e}");
    }
    let result = format!("{value:.12}");
    result.trim_end_matches('0').trim_end_matches('.').into()
}
fn record(state: &mut State, expression: String, value: f64) {
    state.ans = value;
    state.result = format_number(value);
    state.history.insert(
        0,
        History {
            expression,
            value,
            angle: state.fields["angle"].clone(),
        },
    );
    state.history.truncate(MAX_HISTORY);
}
fn storage(host: Option<&Host>) -> Result<Option<&Storage>, String> {
    let Some(host) = host else {
        return Ok(None);
    };
    if !host.permissions.iter().any(|p| p == "storage") {
        return Ok(None);
    }
    let storage = host
        .storage
        .as_ref()
        .ok_or("Missing granted storage snapshot")?;
    if storage.revision < 0 {
        return Err("Invalid host revision".into());
    }
    Ok(Some(storage))
}
pub fn process(input: &str) -> Result<String, String> {
    if !bounded(input, 1_048_576) {
        return Err("Calculator request exceeds limit".into());
    }
    let request: Request = serde_json::from_str(input).map_err(|_| "Invalid calculator request")?;
    if request.version != 2
        || !["zh-CN", "en"].contains(&request.locale.as_str())
        || request.fields.len() > 10
        || request.results.len() > 1
        || !bounded(&request.state, 240_000)
    {
        return Err("Invalid calculator protocol".into());
    }
    let zh = request.locale == "zh-CN";
    let mut state: State = if request.state.is_empty() {
        State::default()
    } else {
        serde_json::from_str(&request.state)
            .map_err(|_| "Invalid calculator session; original retained")?
    };
    if !valid_state(&state) {
        return Err("Incompatible calculator session; original retained".into());
    }
    state.result = format_number(state.ans);
    for (key, value) in &request.fields {
        if !state.fields.contains_key(key)
            || !bounded(
                value,
                if ["expression", "functions"].contains(&key.as_str()) {
                    4096
                } else {
                    80
                },
            )
        {
            return Err("Invalid calculator field".into());
        }
        state.fields.insert(key.clone(), value.clone());
    }
    if !["rad", "deg"].contains(&state.fields["angle"].as_str())
        || !["0", "1", "2"].contains(&state.fields["curve"].as_str())
    {
        return Err("Invalid calculator mode or curve".into());
    }
    let store = storage(request.host.as_ref())?;
    let mut errors = BTreeMap::<String, String>::new();
    let mut effects = Vec::<Value>::new();
    let mut artifacts = Vec::<Value>::new();
    state.notice.clear();
    let action = request.action.as_deref();
    if state.pending && action != Some("host-result") {
        return Err("A save is awaiting its matching host receipt".into());
    }
    match action {
        None | Some("plot") => {}
        Some("calculate") => {
            let source = state.fields["expression"].trim().to_owned();
            match vars(&state).and_then(|v| math::evaluate(&source, v)) {
                Ok(value) => record(&mut state, source, value),
                Err(error) => {
                    errors.insert("expression".into(), error);
                }
            }
        }
        Some("memory-store") => state.memory = state.ans,
        Some("memory-add") => {
            let value = state.memory + state.ans;
            if value.is_finite() {
                state.memory = value;
            } else {
                errors.insert("expression".into(), "Memory overflow".into());
            }
        }
        Some("memory-clear") => state.memory = 0.0,
        Some("history-clear") => state.history.clear(),
        Some("reset-plot") => {
            for (key, value) in [
                ("xmin", "-10"),
                ("xmax", "10"),
                ("ymin", "-3"),
                ("ymax", "3"),
            ] {
                state.fields.insert(key.into(), value.into());
            }
        }
        Some("root") | Some("integrate") => {
            let computed = (|| {
                let values = functions(&state)?;
                let index: usize = state.fields["curve"]
                    .parse()
                    .map_err(|_| "Choose an existing curve")?;
                let source = *values.get(index).ok_or("Choose an existing curve")?;
                let (a, b) = (number(&state, "a")?, number(&state, "b")?);
                let base = vars(&state)?;
                let result = if action == Some("root") {
                    math::root(source, a, b, base)?
                } else {
                    math::integrate(source, a, b, base)?
                };
                Ok::<_, String>((source.to_owned(), result, a, b))
            })();
            match computed {
                Ok((source, result, a, b)) => {
                    let description = if action == Some("root") {
                        format!("root({source}, {a}, {b})")
                    } else {
                        format!("integral({source}, {a}, {b})")
                    };
                    record(&mut state, description, result);
                    state.notice = tr(
                        zh,
                        "数值近似结果；请结合曲线检查区间与定义域。",
                        "Numerical approximation; review the interval and domain on the graph.",
                    )
                    .into();
                }
                Err(error) => {
                    errors.insert("a".into(), error);
                }
            }
        }
        Some("save") => {
            if let Some(store) = store {
                // Validate existing data even for overwrite; do not destroy future or corrupt data.
                if let Some(value) = store.values.get(STORAGE_KEY) {
                    let old: State = serde_json::from_value(value.clone())
                        .map_err(|_| "Saved calculator data is invalid; original retained")?;
                    if !valid_state(&old) {
                        return Err(
                            "Saved calculator data is incompatible; original retained".into()
                        );
                    }
                }
                let mut saved = state.clone();
                saved.pending = false;
                saved.plot = Plot::default();
                effects.push(json!({"kind":"storage","id":"save-workspace","expectedRevision":store.revision,"changes":{STORAGE_KEY:saved}}));
                state.pending = true;
                state.notice = tr(
                    zh,
                    "正在保存，等待宿主确认。",
                    "Saving; awaiting host confirmation.",
                )
                .into();
            } else {
                state.notice=tr(zh,"保存需要可选存储授权；计算与绘图仍可离线使用。","Saving requires the optional storage permission; calculation and graphs remain available offline.").into();
            }
        }
        Some("load") => {
            if let Some(store) = store {
                if let Some(value) = store.values.get(STORAGE_KEY) {
                    let saved: State = serde_json::from_value(value.clone())
                        .map_err(|_| "Saved calculator data is invalid; original retained")?;
                    if !valid_state(&saved) || saved.pending {
                        return Err(
                            "Saved calculator data is incompatible; original retained".into()
                        );
                    }
                    state = saved;
                    state.notice =
                        tr(zh, "已载入保存的计算工作区。", "Saved workspace loaded.").into();
                } else {
                    state.notice = tr(zh, "尚无保存的工作区。", "No saved workspace yet.").into();
                }
            } else {
                state.notice = tr(
                    zh,
                    "请在扩展管理中选择授予存储权限。",
                    "Choose the storage permission in extension management to load saved data.",
                )
                .into();
            }
        }
        Some("host-result") => {
            if !state.pending {
                return Err("Unexpected storage receipt".into());
            }
            let receipt = request.results.first().ok_or("Missing save receipt")?;
            if receipt.id != "save-workspace" || receipt.value.is_some() == receipt.error.is_some()
            {
                return Err("Invalid save receipt".into());
            }
            state.pending = false;
            if let Some(value) = &receipt.value {
                if value["revision"]
                    .as_i64()
                    .is_none_or(|r| r < 0 || r > i32::MAX as i64)
                    || !value["values"].is_object()
                {
                    return Err("Invalid successful save receipt".into());
                }
                state.notice = tr(zh, "宿主已确认保存。", "Host confirmed the save.").into();
            } else {
                let code = receipt
                    .error
                    .as_ref()
                    .and_then(|e| e["code"].as_str())
                    .filter(|s| bounded(s, 80))
                    .unwrap_or("UNKNOWN");
                state.notice = format!(
                    "{} [{code}] {}",
                    tr(zh, "保存未确认。", "Save was not confirmed."),
                    tr(
                        zh,
                        "工作区已保留，请审阅后手动重试。",
                        "Workspace retained; review before retrying manually."
                    )
                );
            }
        }
        Some("export") => {
            let mut body = format!(
                "# {}\n\n",
                tr(zh, "科学计算记录", "Scientific calculation history")
            );
            for entry in &state.history {
                body.push_str(&format!(
                    "- `{}` = **{}** ({})\n",
                    entry.expression.replace('`', "'"),
                    format_number(entry.value),
                    entry.angle
                ));
            }
            artifacts.push(json!({"name":"calculator-history.md","mediaType":"text/markdown","encoding":"utf8","content":body}));
        }
        Some(_) => return Err("Unknown calculator action".into()),
    }
    match plot(&state) {
        Ok(value) => state.plot = value,
        Err(error) => {
            errors.insert("functions".into(), error);
            state.plot = Plot::default();
        }
    }
    let labels = [
        ("expression", "表达式", "Expression", "text"),
        ("angle", "角度制", "Angle mode", "select"),
        (
            "functions",
            "函数（每行一条）",
            "Functions (one per line)",
            "textarea",
        ),
        ("xmin", "X 最小值", "X minimum", "text"),
        ("xmax", "X 最大值", "X maximum", "text"),
        ("ymin", "Y 最小值", "Y minimum", "text"),
        ("ymax", "Y 最大值", "Y maximum", "text"),
        ("a", "区间起点 a", "Interval start a", "text"),
        ("b", "区间终点 b", "Interval end b", "text"),
        ("curve", "曲线", "Curve", "select"),
    ];
    let fields:Vec<_>=labels.iter().map(|(id,cn,en,kind)|{
        let mut value=json!({"id":id,"label":tr(zh,cn,en),"value":state.fields[*id],"kind":kind,"maxBytes":if ["expression","functions"].contains(id) {4096} else {80}});
        if *id=="angle" {value["options"]=json!([{"value":"rad","label":"RAD"},{"value":"deg","label":"DEG"}]);}
        if *id=="curve" {value["options"]=json!([{"value":"0","label":"f₁"},{"value":"1","label":"f₂"},{"value":"2","label":"f₃"}]);}
        value
    }).collect();
    let actions = [
        ("calculate", "计算", "Calculate"),
        ("memory-store", "MS", "MS"),
        ("memory-add", "M+", "M+"),
        ("memory-clear", "MC", "MC"),
        ("history-clear", "清空历史", "Clear history"),
        ("plot", "绘图", "Plot"),
        ("reset-plot", "重置坐标", "Reset axes"),
        ("root", "区间求根", "Find root"),
        ("integrate", "定积分", "Integrate"),
        ("export", "导出记录", "Export history"),
        ("save", "保存工作区", "Save workspace"),
        ("load", "载入工作区", "Load workspace"),
    ];
    let buttons:Vec<_>=actions.iter().map(|(id,cn,en)|json!({"id":id,"label":tr(zh,cn,en),"disabled":state.pending||(["save","load"].contains(id)&&store.is_none()),"primary":*id=="calculate"})).collect();
    let description = if !state.notice.is_empty() {
        state.notice.clone()
    } else {
        tr(zh,"离线科学计算 · 支持 ans、mem、pi、e 与 x · 图形和数值分析为近似。","Offline scientific calculation · ans, mem, pi, e and x · Graphs and numerical analysis are approximations.").into()
    };
    serde_json::to_string(&json!({"version":2,"state":serde_json::to_string(&state).map_err(|_|"Cannot encode calculator state")?,"display":tr(zh,"科学计算器","Scientific calculator"),"description":description,"fields":fields,"groups":[{"columns":4,"actions":buttons}],"blocks":[{"kind":"metrics","items":[{"label":tr(zh,"结果","Result"),"value":state.result},{"label":tr(zh,"记忆","Memory"),"value":format_number(state.memory)},{"label":tr(zh,"历史","History"),"value":state.history.len().to_string()}]}],"effects":effects,"errors":errors,"artifacts":artifacts})).map_err(|_|"Cannot encode calculator view".into())
}
#[cfg(target_arch = "wasm32")]
#[plugin_fn]
pub fn workspace(input: String) -> FnResult<String> {
    Ok(process(&input).map_err(Error::msg)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn call(action: Option<&str>, state: &str, fields: Value) -> Value {
        serde_json::from_str(
            &process(
                &json!({"version":2,"action":action,"state":state,"fields":fields,"locale":"en"})
                    .to_string(),
            )
            .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn initializes_without_permissions() {
        let view = call(None, "", json!({}));
        assert_eq!(view["effects"], json!([]));
        assert_eq!(view["display"], "Scientific calculator");
    }
    #[test]
    fn calculation_records_result_and_history() {
        let v = call(Some("calculate"), "", json!({"expression":"2^3^2"}));
        let s: State = serde_json::from_str(v["state"].as_str().unwrap()).unwrap();
        assert_eq!(s.ans, 512.0);
        assert_eq!(s.history.len(), 1);
    }
    #[test]
    fn invalid_expression_retains_previous_answer() {
        let v = call(Some("calculate"), "", json!({"expression":"1/0"}));
        let s: State = serde_json::from_str(v["state"].as_str().unwrap()).unwrap();
        assert_eq!(s.ans, 0.0);
        assert!(v["errors"]["expression"].is_string());
    }
    #[test]
    fn degree_mode_and_memory() {
        let v = call(
            Some("calculate"),
            "",
            json!({"expression":"sin(90)","angle":"deg"}),
        );
        let v = call(
            Some("memory-store"),
            v["state"].as_str().unwrap(),
            json!({}),
        );
        let s: State = serde_json::from_str(v["state"].as_str().unwrap()).unwrap();
        assert!((s.memory - 1.0).abs() < 1e-12);
    }
    #[test]
    fn plot_has_three_real_series() {
        let v = call(None, "", json!({}));
        let state: Value = serde_json::from_str(v["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["plot"]["series"].as_array().unwrap().len(), 3);
        assert_eq!(
            state["plot"]["series"][0]["points"]
                .as_array()
                .unwrap()
                .len(),
            641
        );
    }
    #[test]
    fn singularities_create_gaps() {
        let v = call(Some("plot"), "", json!({"functions":"1/x"}));
        let state: Value = serde_json::from_str(v["state"].as_str().unwrap()).unwrap();
        assert!(
            state["plot"]["series"][0]["points"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p[1].is_null())
        );
    }
    #[test]
    fn future_session_is_preserved() {
        let s = State {
            schema: 99,
            ..State::default()
        };
        let result=process(&json!({"version":2,"action":null,"state":serde_json::to_string(&s).unwrap(),"fields":{},"locale":"en"}).to_string());
        assert!(result.is_err());
    }
    #[test]
    fn forged_derived_graph_is_recomputed() {
        let v = call(None, "", json!({}));
        let mut s: Value = serde_json::from_str(v["state"].as_str().unwrap()).unwrap();
        s["plot"] = json!({"forged":true});
        let v = call(None, &s.to_string(), json!({}));
        let s: Value = serde_json::from_str(v["state"].as_str().unwrap()).unwrap();
        assert!(s["plot"]["series"].is_array());
    }
    #[test]
    fn numerical_tools_use_selected_curve() {
        let v = call(
            Some("integrate"),
            "",
            json!({"functions":"x^2","a":"0","b":"3"}),
        );
        let s: State = serde_json::from_str(v["state"].as_str().unwrap()).unwrap();
        assert!((s.ans - 9.0).abs() < 1e-7);
    }
    #[test]
    fn storage_is_optional_and_initially_idle() {
        let v = call(Some("save"), "", json!({}));
        assert_eq!(v["effects"], json!([]));
        assert!(v["description"].as_str().unwrap().contains("optional"));
    }
    fn with_storage(
        action: &str,
        state: &str,
        values: Value,
        results: Value,
    ) -> Result<Value, String> {
        let output = process(&json!({"version":2,"action":action,"state":state,"fields":{},"locale":"en","host":{"permissions":["storage"],"storage":{"revision":7,"values":values}},"results":results}).to_string())?;
        serde_json::from_str(&output).map_err(|_| "Invalid output".into())
    }
    #[test]
    fn save_uses_host_revision_and_waits_for_matching_receipt() {
        let saved = with_storage("save", "", json!({}), json!([])).unwrap();
        assert_eq!(saved["effects"][0]["expectedRevision"], 7);
        assert_eq!(saved["effects"][0]["changes"][STORAGE_KEY]["schema"], 1);
        let pending: State = serde_json::from_str(saved["state"].as_str().unwrap()).unwrap();
        assert!(pending.pending);
        let confirmed = with_storage(
            "host-result",
            saved["state"].as_str().unwrap(),
            json!({}),
            json!([{"id":"save-workspace","value":{"revision":8,"values":{}},"error":null}]),
        )
        .unwrap();
        let state: State = serde_json::from_str(confirmed["state"].as_str().unwrap()).unwrap();
        assert!(!state.pending);
        assert_eq!(confirmed["description"], "Host confirmed the save.");
    }
    #[test]
    fn conflict_preserves_workspace_and_never_retries() {
        let calculated = call(Some("calculate"), "", json!({"expression":"6*7"}));
        let saved = with_storage(
            "save",
            calculated["state"].as_str().unwrap(),
            json!({}),
            json!([]),
        )
        .unwrap();
        let failed = with_storage("host-result", saved["state"].as_str().unwrap(), json!({}), json!([{"id":"save-workspace","value":null,"error":{"code":"CONFLICT","message":"changed","retryable":false}}])).unwrap();
        let state: State = serde_json::from_str(failed["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.ans, 42.0);
        assert_eq!(state.history.len(), 1);
        assert!(!state.pending);
        assert_eq!(failed["effects"], json!([]));
        assert!(failed["description"].as_str().unwrap().contains("CONFLICT"));
    }
    #[test]
    fn mismatched_receipts_are_rejected() {
        let saved = with_storage("save", "", json!({}), json!([])).unwrap();
        assert!(
            with_storage(
                "host-result",
                saved["state"].as_str().unwrap(),
                json!({}),
                json!([{"id":"other-operation","value":{"revision":8,"values":{}},"error":null}])
            )
            .is_err()
        );
        assert!(with_storage("host-result", "", json!({}), json!([])).is_err());
    }
    #[test]
    fn future_saved_data_is_never_overwritten() {
        let state = State {
            schema: 99,
            ..State::default()
        };
        assert!(with_storage("save", "", json!({STORAGE_KEY:state}), json!([])).is_err());
        assert!(
            with_storage(
                "load",
                "",
                json!({STORAGE_KEY:{"schema":1,"broken":true}}),
                json!([])
            )
            .is_err()
        );
    }
    #[test]
    fn stored_workspace_roundtrips_without_derived_samples() {
        let calculated = call(Some("calculate"), "", json!({"expression":"10+20"}));
        let saved = with_storage(
            "save",
            calculated["state"].as_str().unwrap(),
            json!({}),
            json!([]),
        )
        .unwrap();
        let value = &saved["effects"][0]["changes"][STORAGE_KEY];
        assert_eq!(value["plot"]["series"], json!([]));
        let loaded = with_storage("load", "", json!({STORAGE_KEY:value}), json!([])).unwrap();
        let state: Value = serde_json::from_str(loaded["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["ans"], 30.0);
        assert_eq!(state["plot"]["series"].as_array().unwrap().len(), 3);
    }
    #[test]
    fn invalid_select_values_never_produce_an_invalid_view() {
        for fields in [json!({"angle":"other"}), json!({"curve":"7"})] {
            assert!(
                process(
                    &json!({"version":2,"action":null,"state":"","fields":fields,"locale":"en"})
                        .to_string()
                )
                .is_err()
            );
        }
    }
}
