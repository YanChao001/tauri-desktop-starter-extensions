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
const MAX_STATE_BYTES: usize = 240_000;

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
#[derive(Clone, PartialEq, Serialize, Deserialize)]
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
    #[serde(default)]
    pending_revision: Option<i32>,
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
            pending_revision: None,
            plot: Plot::default(),
        }
    }
}
fn tr<'a>(zh: bool, cn: &'a str, en: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn localized_error(error: String, zh: bool) -> String {
    if !zh {
        return error;
    }
    let translated = match error.as_str() {
        "Expression is empty" => "请输入表达式。",
        "Expression exceeds 4096 bytes" => "表达式不能超过 4096 个 UTF-8 字节。",
        "Expression exceeds 2048 nodes" => "表达式过于复杂，不能超过 2048 个节点。",
        "Expression exceeds nesting depth 64" => "表达式嵌套不能超过 64 层。",
        "A function accepts at most 16 arguments" => "每个函数最多接受 16 个参数。",
        "Variables must be finite numbers" => "变量必须为有限数值。",
        "Division by zero" => "除数不能为零。",
        "Modulo by zero" => "取余运算的除数不能为零。",
        "Memory overflow" => "记忆寄存器数值溢出。",
        "tan is undefined at this angle" => "此角度的正切没有定义。",
        "Inverse sine/cosine requires an argument between -1 and 1" => {
            "反正弦和反余弦的参数必须介于 -1 和 1 之间。"
        }
        "atan2 is undefined when both arguments are zero" => "atan2 的两个参数不能同时为零。",
        "sqrt requires a non-negative argument" => "平方根的参数不能为负数。",
        "Logarithms require a positive argument" => "对数的参数必须为正数。",
        "Logarithm base must be positive and different from 1" => {
            "对数的底数必须为正数且不等于 1。"
        }
        "clamp lower bound must not exceed its upper bound" => "clamp 的下界不能大于上界。",
        "Factorial requires an integer between 0 and 170" => {
            "阶乘的参数必须为 0 至 170 之间的整数。"
        }
        "Result is not finite; check the domain and numeric range" => {
            "结果不是有限数值，请检查定义域与数值范围。"
        }
        "Zero cannot be raised to a negative power" => "零不能取负指数幂。",
        "A negative base requires an integer exponent; use cbrt for real cube roots" => {
            "负底数只接受整数指数，实数立方根请使用 cbrt。"
        }
        "Enter between one and three functions, one per line" => "请输入一至三个函数，每行一个。",
        "Graph ranges must increase and span at least 1e-10" => {
            "坐标上界必须大于下界，跨度至少为 1e-10。"
        }
        "Choose an existing curve" => "请选择已输入的曲线。",
        "Interval bounds and width must be finite" => "区间端点及宽度必须为有限数值。",
        "No sampled zero or sign-changing root found in the interval" => {
            "区间内未找到采样零点或符号变化的夹根区间。"
        }
        "Root did not converge; the interval may contain a singularity" => {
            "求根未收敛，区间内可能存在奇点或不连续。"
        }
        "Root iteration limit reached; narrow the interval" => "求根已达迭代上限，请缩小区间。",
        "Integration interval is too narrow to subdivide" => "积分区间太窄，无法继续细分。",
        "Integration evaluation limit reached; narrow the interval" => {
            "积分已达采样上限，请缩小区间。"
        }
        "Integration cannot resolve this interval; narrow the numeric range" => {
            "积分无法分辨此区间，请缩小数值范围。"
        }
        "Integration did not converge; check for singularities or narrow the interval" => {
            "积分未收敛，请检查奇点或缩小区间。"
        }
        _ => {
            for (prefix, text) in [
                (
                    "Unexpected token at byte ",
                    "表达式有意外符号，请检查括号并用 * 表示乘法。",
                ),
                ("Unexpected character at byte ", "表达式含有不支持的字符。"),
                ("Invalid number at byte ", "表达式中有无效的数值。"),
                (
                    "Invalid scientific notation at byte ",
                    "科学计数法格式无效，例如 1.2e-3。",
                ),
                (
                    "Expected a number, variable or '(' at byte ",
                    "此处需要数值、变量或左括号。",
                ),
                ("Expected ')' at byte ", "表达式缺少右括号。"),
                ("Unknown function: ", "未知函数："),
                ("Unknown identifier: ", "未知标识符："),
            ] {
                if let Some(detail) = error.strip_prefix(prefix) {
                    return if prefix.starts_with("Unknown") {
                        format!("{text}{detail}")
                    } else {
                        text.into()
                    };
                }
            }
            if error.starts_with("Function '") {
                return "函数参数数量或括号不正确，请查看函数速查。".into();
            }
            if let Some((field, reason)) = error.split_once(": ")
                && ["xmin", "xmax", "ymin", "ymax", "a", "b"].contains(&field)
            {
                return format!(
                    "{field}：{}",
                    if reason.contains("±1e12") {
                        "请输入 ±1e12 范围内的有限数值。"
                    } else {
                        "请输入有限数值。"
                    }
                );
            }
            return error;
        }
    };
    translated.into()
}
fn bounded(value: &str, max: usize) -> bool {
    value.len() <= max && !value.contains('\0')
}
fn valid_state(state: &State) -> bool {
    state.schema == 1
        && state.ans.is_finite()
        && state.memory.is_finite()
        && state.pending == state.pending_revision.is_some()
        && state
            .pending_revision
            .is_none_or(|revision| (0..i32::MAX).contains(&revision))
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
            // Keep token separation without expanding thousands of tabs/newlines
            // into escaped JSON in the bounded workspace and Markdown history.
            expression: expression.split_whitespace().collect::<Vec<_>>().join(" "),
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
        || !bounded(&request.state, MAX_STATE_BYTES)
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
    let mut save_revision = None;
    state.notice.clear();
    let action = request.action.as_deref();
    if action != Some("host-result") && !request.results.is_empty() {
        return Err("Unexpected storage receipt".into());
    }
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
                if store.revision == i32::MAX {
                    return Err("Storage revision is exhausted; original retained".into());
                }
                // Validate existing data even for overwrite; do not destroy future or corrupt data.
                if let Some(value) = store.values.get(STORAGE_KEY) {
                    let old: State = serde_json::from_value(value.clone())
                        .map_err(|_| "Saved calculator data is invalid; original retained")?;
                    if !valid_state(&old) || old.pending {
                        return Err(
                            "Saved calculator data is incompatible; original retained".into()
                        );
                    }
                }
                save_revision = Some(store.revision);
                state.pending = true;
                state.pending_revision = Some(store.revision);
                state.notice = tr(zh, "正在保存工作区…", "Saving workspace…").into();
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
            if let Some(value) = &receipt.value {
                let saved: State = serde_json::from_value(value["values"][STORAGE_KEY].clone())
                    .map_err(|_| "Invalid successful save receipt")?;
                if value["revision"].as_i64()
                    != state
                        .pending_revision
                        .map(|revision| i64::from(revision) + 1)
                    || !valid_state(&saved)
                    || saved.pending
                    || saved.ans != state.ans
                    || saved.memory != state.memory
                    || saved.fields != state.fields
                    || saved.history != state.history
                {
                    return Err("Invalid successful save receipt".into());
                }
                state.notice = tr(zh, "工作区已保存。", "Workspace saved.").into();
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
            state.pending = false;
            state.pending_revision = None;
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
    // Graph samples are derived and can expand an imported or long-history
    // workspace. Preserve answers, memory and current fields; discard only the
    // oldest history until the next request fits the same state budget.
    let mut trimmed = false;
    while serde_json::to_vec(&state)
        .map_err(|_| "Cannot encode calculator state")?
        .len()
        > MAX_STATE_BYTES - 256
        && !state.history.is_empty()
    {
        state.history.pop();
        trimmed = true;
    }
    if trimmed {
        state.notice.push_str(tr(
            zh,
            " 会话额度已满，已移除最旧的历史记录。",
            " The session limit was reached; the oldest history entries were removed.",
        ));
    }
    if let Some(revision) = save_revision {
        let mut saved = state.clone();
        saved.pending = false;
        saved.pending_revision = None;
        saved.notice.clear();
        saved.plot = Plot::default();
        effects.push(json!({"kind":"storage","id":"save-workspace","expectedRevision":revision,"changes":{STORAGE_KEY:saved}}));
    }
    for error in errors.values_mut() {
        *error = localized_error(std::mem::take(error), zh);
        // The host accepts at most 1000 UTF-8 bytes per field error. Unknown
        // identifiers can consume the entire 4096-byte expression allowance.
        if error.len() > 1000 {
            let mut end = 997;
            while !error.is_char_boundary(end) {
                end -= 1;
            }
            error.truncate(end);
            error.push('…');
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
    let encoded_state =
        serde_json::to_string(&state).map_err(|_| "Cannot encode calculator state")?;
    if encoded_state.len() > MAX_STATE_BYTES {
        return Err("Calculator workspace exceeds the session limit; original retained".into());
    }
    serde_json::to_string(&json!({"version":2,"state":encoded_state,"display":tr(zh,"科学计算器","Scientific calculator"),"description":description,"fields":fields,"groups":[{"columns":4,"actions":buttons}],"blocks":[{"kind":"metrics","items":[{"label":tr(zh,"结果","Result"),"value":state.result},{"label":tr(zh,"记忆","Memory"),"value":format_number(state.memory)},{"label":tr(zh,"历史","History"),"value":state.history.len().to_string()}]}],"effects":effects,"errors":errors,"artifacts":artifacts})).map_err(|_|"Cannot encode calculator view".into())
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
            json!([{"id":"save-workspace","value":{"revision":8,"values":{STORAGE_KEY:saved["effects"][0]["changes"][STORAGE_KEY]}},"error":null}]),
        )
        .unwrap();
        let state: State = serde_json::from_str(confirmed["state"].as_str().unwrap()).unwrap();
        assert!(!state.pending);
        assert_eq!(confirmed["description"], "Workspace saved.");
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
    #[test]
    fn repeated_viewport_updates_keep_fixed_samples_and_preserve_work() {
        let mut view = call(Some("calculate"), "", json!({"expression":"6*7"}));
        for step in 0..80 {
            let start = -10.0 + f64::from(step) / 137.0;
            view = call(
                Some("plot"),
                view["state"].as_str().unwrap(),
                json!({"expression":"unsaved draft 你好", "xmin":start.to_string(),"xmax":(start+20.0).to_string()}),
            );
            let text = view["state"].as_str().unwrap();
            assert!(text.len() < 240_000);
            let state: Value = serde_json::from_str(text).unwrap();
            assert_eq!(state["ans"], 42.0);
            assert_eq!(state["history"].as_array().unwrap().len(), 1);
            assert_eq!(state["fields"]["expression"], "unsaved draft 你好");
            assert_eq!(view["errors"], json!({}));
            let series = state["plot"]["series"].as_array().unwrap();
            assert_eq!(series.len(), 3);
            assert!(
                series
                    .iter()
                    .all(|item| item["points"].as_array().unwrap().len() == SAMPLES + 1)
            );
        }
    }
    #[test]
    fn narrow_ranges_at_large_offsets_survive_numeric_roundtrips() {
        let view = call(
            Some("plot"),
            "",
            json!({"xmin":"999999999999.999","xmax":"1000000000000","functions":"x"}),
        );
        assert_eq!(view["errors"], json!({}));
        let state: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert!(state["plot"]["xmax"].as_f64().unwrap() > state["plot"]["xmin"].as_f64().unwrap());
        assert_eq!(state["fields"]["xmin"], "999999999999.999");
    }

    #[test]
    fn save_requires_the_exact_revision_and_workspace_receipt() {
        let saved = with_storage("save", "", json!({}), json!([])).unwrap();
        let workspace = &saved["effects"][0]["changes"][STORAGE_KEY];
        for (revision, value) in [
            (7, workspace.clone()),
            (9, workspace.clone()),
            (8, json!({})),
        ] {
            assert!(with_storage("host-result", saved["state"].as_str().unwrap(), json!({}),
                json!([{"id":"save-workspace","value":{"revision":revision,"values":{STORAGE_KEY:value}},"error":null}])).is_err());
        }
        let mut changed = workspace.clone();
        changed["memory"] = json!(42);
        assert!(with_storage("host-result", saved["state"].as_str().unwrap(), json!({}),
            json!([{"id":"save-workspace","value":{"revision":8,"values":{STORAGE_KEY:changed}},"error":null}])).is_err());
        assert!(process(&json!({"version":2,"action":"save","state":"","fields":{},"locale":"en","host":{"permissions":["storage"],"storage":{"revision":i32::MAX,"values":{}}}}).to_string()).is_err());
    }

    #[test]
    fn long_whitespace_history_stays_roundtrippable_and_exports_one_line() {
        let source = format!("1{}+{}1", "\t\n".repeat(1000), "\t".repeat(90));
        let mut view = call(None, "", json!({}));
        for _ in 0..MAX_HISTORY {
            view = call(
                Some("calculate"),
                view["state"].as_str().unwrap(),
                json!({"expression":source}),
            );
            assert!(view["state"].as_str().unwrap().len() <= MAX_STATE_BYTES);
        }
        let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.history.len(), MAX_HISTORY);
        assert_eq!(state.history[0].expression, "1 + 1");
        let exported = call(Some("export"), view["state"].as_str().unwrap(), json!({}));
        assert_eq!(
            exported["artifacts"][0]["content"]
                .as_str()
                .unwrap()
                .lines()
                .count(),
            MAX_HISTORY + 2
        );
    }

    #[test]
    fn oversized_derived_state_discards_only_oldest_history_before_saving() {
        let mut state = State {
            ans: 42.0,
            memory: 17.0,
            ..State::default()
        };
        state
            .fields
            .insert("expression".into(), "unsaved draft".into());
        // Every history entry is individually valid. Escaped backslashes expand
        // on serialization; derived curve samples then exceed the state budget.
        for index in 0..MAX_HISTORY {
            state.history.push(History {
                expression: format!("{index}{}", "\\".repeat(3000)),
                value: index as f64,
                angle: "rad".into(),
            });
        }
        let original = serde_json::to_string(&state).unwrap();
        assert!(original.len() < MAX_STATE_BYTES);
        let view = with_storage("save", &original, json!({}), json!([])).unwrap();
        let retained: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert!(retained.history.len() < MAX_HISTORY);
        assert_eq!(retained.history[0].value, 0.0);
        assert_eq!(retained.ans, 42.0);
        assert_eq!(retained.memory, 17.0);
        assert_eq!(retained.fields["expression"], "unsaved draft");
        let receipt = json!([{"id":"save-workspace","value":{"revision":8,"values":{STORAGE_KEY:view["effects"][0]["changes"][STORAGE_KEY]}},"error":null}]);
        let confirmed = with_storage(
            "host-result",
            view["state"].as_str().unwrap(),
            json!({}),
            receipt,
        )
        .unwrap();
        assert_eq!(confirmed["description"], "Workspace saved.");
    }

    #[test]
    fn chinese_validation_errors_are_localized() {
        for (fields, key, expected) in [
            (
                json!({"expression":"sqrt(-1)"}),
                "expression",
                "平方根的参数不能为负数。",
            ),
            (
                json!({"xmin":"invalid"}),
                "functions",
                "xmin：请输入有限数值。",
            ),
        ] {
            let output = process(&json!({"version":2,"action":"calculate","state":"","fields":fields,"locale":"zh-CN"}).to_string()).unwrap();
            let view: Value = serde_json::from_str(&output).unwrap();
            assert_eq!(view["errors"][key], expected);
        }
    }

    #[test]
    fn long_identifier_errors_fit_the_host_field_error_limit() {
        for locale in ["en", "zh-CN"] {
            let output = process(&json!({"version":2,"action":"calculate","state":"","fields":{"expression":"a".repeat(4096),"functions":"a".repeat(4096)},"locale":locale}).to_string()).unwrap();
            let view: Value = serde_json::from_str(&output).unwrap();
            assert_eq!(view["errors"].as_object().unwrap().len(), 2);
            for error in view["errors"].as_object().unwrap().values() {
                assert!(error.as_str().unwrap().len() <= 1000);
            }
        }
    }
}
