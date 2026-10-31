//! Scientific computation is performed in Wasm; HTML only renders and navigates results.
#[cfg(target_arch = "wasm32")]
use extism_pdk::*;
mod converter;
mod currency;
mod date;
mod decimal;
mod math;
mod programmer;
use math::{Angle, Vars};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const STORAGE_KEY: &str = "calculator-workspace";
const MAX_HISTORY: usize = 32;
const SAMPLES: usize = 640;
const MAX_STATE_BYTES: usize = 240_000;
const MAX_MEMORIES: usize = 64;

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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    expression: String,
    value: f64,
    angle: String,
    #[serde(default)]
    exact: String,
    #[serde(default)]
    mode: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: u8,
    ans: f64,
    #[serde(default)]
    ans_exact: String,
    memory: f64,
    #[serde(default)]
    memory_set: bool,
    #[serde(default)]
    memories: Vec<String>,
    history: Vec<History>,
    result: String,
    notice: String,
    fields: BTreeMap<String, String>,
    pending: bool,
    #[serde(default)]
    pending_revision: Option<i32>,
    #[serde(default, deserialize_with = "ignore_plot")]
    plot: Plot,
    #[serde(default)]
    programmer: Option<programmer::ProgrammerOutput>,
    #[serde(default)]
    date: Option<date::DateOutput>,
    #[serde(default)]
    carry: bool,
    #[serde(default, deserialize_with = "ignore_converter")]
    converter: Option<converter::Output>,
    #[serde(default)]
    currency: Option<currency::Snapshot>,
    #[serde(default)]
    currency_sequence: u32,
    #[serde(default)]
    currency_pending: Option<u32>,
}
fn ignore_plot<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Plot, D::Error> {
    let _ = serde::de::IgnoredAny::deserialize(deserializer)?;
    Ok(Plot::default())
}
fn ignore_converter<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<converter::Output>, D::Error> {
    let _ = serde::de::IgnoredAny::deserialize(deserializer)?;
    Ok(None)
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
            ans_exact: "0".into(),
            memory: 0.0,
            memory_set: false,
            memories: Vec::new(),
            history: Vec::new(),
            result: "0".into(),
            notice: String::new(),
            fields: BTreeMap::from([
                ("expression".into(), "0".into()),
                ("angle".into(), "deg".into()),
                ("functions".into(), String::new()),
                ("xmin".into(), "-10".into()),
                ("xmax".into(), "10".into()),
                ("ymin".into(), "-3".into()),
                ("ymax".into(), "3".into()),
                ("a".into(), "0".into()),
                ("b".into(), "3.141592653589793".into()),
                ("curve".into(), "0".into()),
                ("mode".into(), "standard".into()),
                ("radix".into(), "10".into()),
                ("bits".into(), "64".into()),
                ("dateFrom".into(), String::new()),
                ("dateTo".into(), String::new()),
                ("dateYears".into(), "0".into()),
                ("dateMonths".into(), "0".into()),
                ("dateDays".into(), "0".into()),
                ("dateOperation".into(), "difference".into()),
                ("converterCategory".into(), "length".into()),
                ("converterFrom".into(), String::new()),
                ("converterTo".into(), String::new()),
                ("converterInput".into(), "0".into()),
            ]),
            pending: false,
            pending_revision: None,
            plot: Plot::default(),
            programmer: None,
            date: None,
            carry: false,
            converter: None,
            currency: None,
            currency_sequence: 0,
            currency_pending: None,
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
        "Nonzero literal is below the supported floating-point range" => {
            "非零数值小于科学计算支持的浮点范围。"
        }
        "Power result is below the supported floating-point range" => {
            "幂运算的非零结果小于科学计算支持的浮点范围。"
        }
        "Modulo by zero" => "取余运算的除数不能为零。",
        "Memory overflow" => "记忆寄存器数值溢出。",
        "Invalid unit catalogue" => "单位目录不可用，请重新载入插件。",
        "Choose a supported converter category" => "请选择支持的换算类别。",
        "Choose a supported unit" => "请选择当前类别支持的单位。",
        "Missing unit conversion factor" => "此单位缺少换算系数。",
        "Converter amount exceeds 256 bytes" => "换算数值不能超过 256 个 UTF-8 字节。",
        "Negative values are not supported in this category" => "此换算类别不支持负数。",
        "Invalid currency rate response" => "汇率来源返回了无效数据，已有缓存已保留。",
        "Currency source is rate limited" => "汇率来源暂时限流，请稍后手动重试。",
        "Currency source request failed" => "汇率来源请求失败，已有缓存已保留。",
        "Currency rates are unavailable" => "尚无可用的汇率，请先更新汇率。",
        "Choose a supported currency" => "请选择支持的货币。",
        "Currency amount exceeds 256 bytes" => "货币数值不能超过 256 个 UTF-8 字节。",
        "Currency source returned older rates; previous cache retained" => {
            "来源返回了较旧汇率，已保留现有缓存。"
        }
        "Memory limit reached" => "记忆列表已满，请先清除不再需要的项目。",
        "Choose an existing memory item" => "请选择现有的记忆项目。",
        "Invalid date" => "请输入有效的公历日期。",
        "Date out of range" => "日期必须在 1601 年至 9999 年之间。",
        "Date offset must be between 0 and 999" => "年、月、日偏移量必须为 0 至 999 之间的整数。",
        "Choose add or subtract for date offset" => "日期偏移请选择增加或减去。",
        "Invalid decimal number" => "请输入有效的十进制数。",
        "Decimal exponent must be between -9999 and 9999" => {
            "十进制指数必须在 -9999 至 9999 之间。"
        }
        "Decimal result is outside the supported exponent range" => {
            "结果超出支持的十进制指数范围。"
        }
        "Expression exceeds calculation budget" => "算式超出计算额度，请简化输入。",
        "Expression nesting exceeds 64 levels" => "表达式嵌套不能超过 64 层。",
        "Unknown decimal function or variable" | "Unknown decimal function" => {
            "该模式不支持此函数或变量。"
        }
        "Unknown decimal operator" => "该模式不支持此运算。",
        "Decimal power requires an integer exponent" => "精确幂运算要求指数为整数。",
        "Zero to the power of zero is undefined" => "零的零次幂没有定义。",
        "Square root requires a non-negative value" => "平方根的参数不能为负数。",
        "Missing closing parenthesis" => "表达式缺少右括号。",
        "Cannot divide by zero" => "除数不能为零。",
        "Bit index is outside the selected word size" => "位索引超出当前字长。",
        "Shift count must be smaller than the selected word size" => "位移量必须小于当前字长。",
        "Radix must be 2, 8, 10 or 16" => "进制必须为 2、8、10 或 16。",
        "Word size must be 8, 16, 32 or 64 bits" => "字长必须为 8、16、32 或 64 位。",
        "Invalid bit index" => "请输入有效的位索引。",
        "Expression exceeds 2048 tokens" => "表达式过于复杂。",
        "Integer function requires parentheses" => "整数函数需要括号。",
        "Integer function requires two arguments" => "整数函数需要两个参数。",
        "Bit function requires a value and bit index" => "位函数需要数值和位索引。",
        "tan is undefined at this angle" => "此角度的正切没有定义。",
        "sec is undefined at this angle" => "此角度的正割没有定义。",
        "csc/cot is undefined at this angle" => "此角度的余割和余切没有定义。",
        "Inverse sine/cosine requires an argument between -1 and 1" => {
            "反正弦和反余弦的参数必须介于 -1 和 1 之间。"
        }
        "atan2 is undefined when both arguments are zero" => "atan2 的两个参数不能同时为零。",
        "Inverse cotangent is undefined at zero" => "反余切在零处没有定义。",
        "Inverse secant/cosecant requires an absolute argument of at least 1" => {
            "反正割和反余割的参数绝对值不能小于 1。"
        }
        "Hyperbolic cosecant/cotangent is undefined at zero" => {
            "双曲余割和双曲余切在零处没有定义。"
        }
        "Inverse hyperbolic cosine requires an argument of at least 1" => {
            "反双曲余弦的参数不能小于 1。"
        }
        "Inverse hyperbolic tangent requires an argument between -1 and 1, exclusive" => {
            "反双曲正切的参数必须大于 -1 且小于 1。"
        }
        "Inverse hyperbolic secant requires an argument greater than 0 and at most 1" => {
            "反双曲正割的参数必须大于 0 且不大于 1。"
        }
        "Inverse hyperbolic cosecant is undefined at zero" => "反双曲余割在零处没有定义。",
        "Inverse hyperbolic cotangent requires an absolute argument greater than 1" => {
            "反双曲余切的参数绝对值必须大于 1。"
        }
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
        "Root degree must not be zero" => "根指数不能为零。",
        "Zero cannot have a negative root degree" => "零不能取负根指数。",
        "A negative radicand requires an odd integer root degree" => {
            "负数的实数根要求根指数为奇整数。"
        }
        "A negative base requires an integer exponent; use cbrt for real cube roots" => {
            "负底数只接受整数指数，实数立方根请使用 cbrt。"
        }
        "Enter between one and eight functions, one per line" => "请输入一至八个函数，每行一个。",
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
                ("Expected a decimal number at byte ", "此处需要十进制数。"),
                (
                    "Invalid integer expression at byte ",
                    "整数表达式含有无效符号。",
                ),
                ("Expected an integer at byte ", "此处需要整数。"),
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
            if error.starts_with("Invalid base-") {
                return "此数值含有当前进制不支持的数字。".into();
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
fn field_limit(key: &str) -> usize {
    match key {
        "expression" | "functions" | "operand" | "left" | "right" => 4096,
        "converterInput" => 256,
        _ => 80,
    }
}
fn upgrade_state(state: &mut State) {
    let defaults = State::default();
    for (key, value) in defaults.fields {
        if ![
            "expression",
            "angle",
            "functions",
            "xmin",
            "xmax",
            "ymin",
            "ymax",
            "a",
            "b",
            "curve",
        ]
        .contains(&key.as_str())
        {
            state.fields.entry(key.clone()).or_insert_with(|| {
                if key == "mode" {
                    "scientific".into()
                } else {
                    value
                }
            });
        }
    }
    if state.ans_exact.is_empty() {
        state.ans_exact = format_number(state.ans);
    }
    for entry in &mut state.history {
        if entry.exact.is_empty() {
            entry.exact = format_number(entry.value);
        }
        if entry.mode.is_empty() {
            entry.mode = "scientific".into();
        }
    }
    if state.memories.is_empty() && (state.memory_set || state.memory != 0.0) {
        state.memories.push(format_number(state.memory));
    }
    sync_memory(state);
}
fn sync_memory(state: &mut State) {
    state.memory_set = !state.memories.is_empty();
    state.memory = state
        .memories
        .first()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(0.0);
}
fn valid_exact(value: &str) -> bool {
    if !bounded(value, 4096) {
        return false;
    }
    let value = value
        .strip_prefix('-')
        .or_else(|| value.strip_prefix('+'))
        .unwrap_or(value);
    let mut parts = value.split(['e', 'E']);
    let mantissa = parts.next().unwrap_or("");
    let exponent = match parts.next() {
        None => 0,
        Some(exponent) => match exponent.parse::<i32>() {
            Ok(exponent) if (-9999..=9999).contains(&exponent) => exponent,
            _ => return false,
        },
    };
    if parts.next().is_some() {
        return false;
    }
    let mut decimals = None;
    let mut digits = 0_i32;
    let mut first_nonzero = None;
    for character in mantissa.bytes() {
        if character == b'.' && decimals.is_none() {
            decimals = Some(digits);
        } else if character.is_ascii_digit() {
            if character != b'0' && first_nonzero.is_none() {
                first_nonzero = Some(digits);
            }
            digits += 1;
        } else {
            return false;
        }
    }
    digits > 0
        && first_nonzero.is_none_or(|first| {
            (-9999..=9999).contains(&(exponent + decimals.unwrap_or(digits) - first - 1))
        })
}
fn valid_state(state: &State) -> bool {
    state.schema == 1
        && state.ans.is_finite()
        && state.memory.is_finite()
        && valid_exact(&state.ans_exact)
        && state.memories.len() <= MAX_MEMORIES
        && state.memories.iter().all(|value| valid_exact(value))
        && state
            .currency
            .as_ref()
            .is_none_or(currency::Snapshot::valid)
        && state.currency_pending.is_none_or(|sequence| {
            sequence > 0 && sequence == state.currency_sequence && !state.pending
        })
        && state.programmer.as_ref().is_none_or(|value| {
            [2, 8, 10, 16].contains(&value.radix)
                && [8, 16, 32, 64].contains(&value.bits)
                && bounded(&value.value, 80)
                && bounded(&value.dec, 80)
                && bounded(&value.hex, 80)
                && bounded(&value.oct, 80)
                && bounded(&value.bin, 80)
                && bounded(&value.unsigned, 80)
                && programmer::evaluate(&value.value, value.radix, value.bits).is_ok_and(
                    |expected| {
                        expected.dec == value.dec
                            && expected.hex == value.hex
                            && expected.oct == value.oct
                            && expected.bin == value.bin
                            && expected.unsigned == value.unsigned
                    },
                )
        })
        && state.date.as_ref().is_none_or(|value| {
            value.total_days <= 3_100_000
                && value.years <= 8399
                && value.months < 12
                && value.weeks < 5
                && value.days < 7
                && date::difference(&value.date, &value.date).is_ok()
        })
        && state.pending == state.pending_revision.is_some()
        && state
            .pending_revision
            .is_none_or(|revision| (0..i32::MAX).contains(&revision))
        && valid_exact(&state.result)
        && bounded(&state.notice, 600)
        && state.history.len() <= MAX_HISTORY
        && state.history.iter().all(|entry| {
            bounded(&entry.expression, 4352)
                && entry.value.is_finite()
                && valid_exact(&entry.exact)
                && ["standard", "scientific", "graph", "programmer"].contains(&entry.mode.as_str())
                && ["rad", "deg", "grad"].contains(&entry.angle.as_str())
        })
        && state.fields.len() == State::default().fields.len()
        && State::default().fields.keys().all(|key| {
            state
                .fields
                .get(key)
                .is_some_and(|value| bounded(value, field_limit(key)))
        })
        && ["rad", "deg", "grad"].contains(&state.fields["angle"].as_str())
        && state.fields["curve"]
            .parse::<usize>()
            .is_ok_and(|curve| curve < 8)
        && [
            "standard",
            "scientific",
            "graph",
            "programmer",
            "date",
            "converter",
        ]
        .contains(&state.fields["mode"].as_str())
        && ["2", "8", "10", "16"].contains(&state.fields["radix"].as_str())
        && ["8", "16", "32", "64"].contains(&state.fields["bits"].as_str())
        && ["difference", "add", "subtract"].contains(&state.fields["dateOperation"].as_str())
        && converter::CATEGORIES.contains(&state.fields["converterCategory"].as_str())
}
fn scientific_source(state: &State, source: &str) -> String {
    let mut output = String::new();
    let mut chars = source.char_indices().peekable();
    while let Some((start, character)) = chars.next() {
        if character.is_ascii_alphabetic() || character == '_' {
            let mut end = start + character.len_utf8();
            while let Some(&(offset, next)) = chars.peek() {
                if !next.is_ascii_alphanumeric() && next != '_' {
                    break;
                }
                chars.next();
                end = offset + next.len_utf8();
            }
            let word = &source[start..end];
            match word.to_ascii_lowercase().as_str() {
                "ans" => {
                    output.push('(');
                    output.push_str(&state.ans_exact);
                    output.push(')');
                }
                "mem" => {
                    output.push('(');
                    output.push_str(state.memories.first().map(String::as_str).unwrap_or("0"));
                    output.push(')');
                }
                _ => output.push_str(word),
            }
        } else {
            output.push(character);
        }
    }
    output
}
fn exact_value(state: &State, source: &str, standard: bool) -> Result<String, String> {
    let source = scientific_source(state, source);
    if standard {
        decimal::standard(&source)
    } else {
        if source.contains('%') {
            vars(state)
                .and_then(|vars| math::evaluate(&source, vars))
                .map(format_number)
        } else {
            match decimal::scientific(&source)? {
                Some(value) => Ok(value),
                None => vars(state)
                    .and_then(|vars| math::evaluate(&source, vars))
                    .map(format_number),
            }
        }
    }
}
fn vars(state: &State) -> Result<Vars, String> {
    let angle = match state.fields["angle"].as_str() {
        "rad" => Angle::Rad,
        "deg" => Angle::Deg,
        "grad" => Angle::Grad,
        _ => return Err("Angle mode must be rad, deg or grad".into()),
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
        .map(|source| {
            source
                .split_once('=')
                .filter(|(left, _)| left.trim().eq_ignore_ascii_case("y"))
                .map(|(_, right)| right.trim())
                .unwrap_or(source)
        })
        .filter(|line| !line.is_empty())
        .collect();
    if values.is_empty() || values.len() > 8 {
        return Err("Enter between one and eight functions, one per line".into());
    }
    Ok(values)
}
fn fitted_y_range(series: &[Series]) -> Option<(f64, f64)> {
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for y in series
        .iter()
        .flat_map(|series| &series.points)
        .filter_map(|(_, y)| *y)
    {
        if y.is_finite() && y.abs() <= 1e12 {
            low = low.min(y);
            high = high.max(y);
        }
    }
    if !low.is_finite() {
        return None;
    }
    let margin = if high == low {
        (low.abs() * 0.1).max(0.5)
    } else {
        ((high - low) * 0.1)
            .max(low.abs().max(high.abs()) * f64::EPSILON * 4.0)
            .max(1e-10)
    };
    Some(((low - margin).max(-1e12), (high + margin).min(1e12)))
}
fn plot(state: &State, fit_y: bool) -> Result<Plot, String> {
    let (xmin, xmax, mut ymin, mut ymax) = (
        number(state, "xmin")?,
        number(state, "xmax")?,
        number(state, "ymin")?,
        number(state, "ymax")?,
    );
    if xmax <= xmin || ymax <= ymin || xmax - xmin < 1e-10 || ymax - ymin < 1e-10 {
        return Err("Graph ranges must increase and span at least 1e-10".into());
    }
    if state.fields["functions"].trim().is_empty() {
        return Ok(Plot {
            series: Vec::new(),
            xmin,
            xmax,
            ymin,
            ymax,
        });
    }
    let base = vars(state)?;
    let mut series = Vec::new();
    let mut expressions = Vec::new();
    let sources = functions(state)?;
    let samples = SAMPLES.min(SAMPLES * 3 / sources.len());
    for source in sources {
        let expression = math::compile(&scientific_source(state, source))?;
        let points = (0..=samples)
            .map(|index| {
                let x = xmin + (xmax - xmin) * index as f64 / samples as f64;
                let value = expression
                    .eval(Vars { x, ..base })
                    .ok()
                    .filter(|v| v.abs() <= 1e100);
                (x, value)
            })
            .collect();
        series.push(Series {
            expression: source.into(),
            points,
        });
        expressions.push(expression);
    }
    // Fit the raw samples before detecting jumps: the old, narrow viewport must
    // not discard steep but continuous parts of a newly entered function.
    if fit_y && let Some((low, high)) = fitted_y_range(&series) {
        ymin = low;
        ymax = high;
    }
    for (curve, expression) in series.iter_mut().zip(&expressions) {
        // A jump across a sampled pole runs against the slopes on both sides.
        // Confirm it with a midpoint outside the endpoint values: slope changes
        // alone also occur in continuous oscillating curves.
        let poles: Vec<_> = curve
            .points
            .windows(4)
            .enumerate()
            .filter_map(|(index, points)| {
                let [(_, Some(a)), (bx, Some(b)), (cx, Some(c)), (_, Some(d))] = points else {
                    return None;
                };
                let (left, jump, right) = (b - a, c - b, d - c);
                if !(left * jump < 0.0
                    && right * jump < 0.0
                    && jump.abs() > left.abs()
                    && jump.abs() > right.abs())
                {
                    return None;
                }
                expression
                    .eval(Vars {
                        x: bx + (cx - bx) / 2.0,
                        ..base
                    })
                    .ok()
                    .is_none_or(|middle| middle < b.min(*c) || middle > b.max(*c))
                    .then_some(index + 2)
            })
            .collect();
        for index in poles {
            curve.points[index].1 = None;
        }
        let mut previous: Option<f64> = None;
        for (_, value) in &mut curve.points {
            // Do not draw vertical bridges across sampled asymptotes.
            if let (Some(before), Some(y)) = (previous, *value)
                && (y - before).abs() > (ymax - ymin) * 8.0
            {
                *value = None;
            }
            previous = *value;
        }
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
    record_exact(state, expression, format_number(value), false);
}
fn record_exact(state: &mut State, expression: String, exact: String, standard: bool) {
    let value = exact
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .unwrap_or(0.0);
    state.ans = value;
    state.ans_exact = exact.clone();
    state.result = exact.clone();
    state.history.insert(
        0,
        History {
            // Keep token separation without expanding thousands of tabs/newlines
            // into escaped JSON in the bounded workspace and Markdown history.
            expression: expression.split_whitespace().collect::<Vec<_>>().join(" "),
            value,
            angle: state.fields["angle"].clone(),
            exact,
            mode: if standard { "standard" } else { "scientific" }.into(),
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
fn update_converter(
    state: &mut State,
    locale: &str,
    reset_units: bool,
) -> Result<(), (String, String)> {
    let category = state.fields["converterCategory"].clone();
    let input = state.fields["converterInput"].clone();
    let (default_from, default_to) = if category == "currency" {
        (
            "USD".into(),
            if locale == "en" { "EUR" } else { "CNY" }.into(),
        )
    } else {
        converter::defaults(&category, locale)
            .map_err(|error| ("converterCategory".into(), error))?
    };
    let supported = if category == "currency" {
        currency::codes(state.currency.as_ref())
    } else {
        converter::units(&category, locale)
            .map_err(|error| ("converterCategory".into(), error))?
            .into_iter()
            .filter(|unit| !unit.whimsical)
            .map(|unit| unit.id)
            .collect()
    };
    for (key, fallback) in [("converterFrom", default_from), ("converterTo", default_to)] {
        if state.fields[key].is_empty() || reset_units && !supported.contains(&state.fields[key]) {
            state.fields.insert(key.into(), fallback);
        }
    }
    let from = state.fields["converterFrom"].clone();
    let to = state.fields["converterTo"].clone();
    if category == "currency" {
        let codes = currency::codes(state.currency.as_ref());
        let units = codes
            .iter()
            .map(|code| converter::UnitOption {
                id: code.clone(),
                label: code.clone(),
                abbreviation: code.clone(),
                whimsical: false,
            })
            .collect();
        let mut output = converter::Output {
            category,
            from: from.clone(),
            to: to.clone(),
            input: input.clone(),
            result: None,
            units,
            equivalents: Vec::new(),
            supports_negative: false,
            updated_at: state.currency.as_ref().map(|snapshot| snapshot.updated_at),
            next_update_at: state
                .currency
                .as_ref()
                .map(|snapshot| snapshot.next_update_at),
            rate: None,
            source: Some("https://www.exchangerate-api.com".into()),
        };
        let computed = (|| {
            if !codes.contains(&from) {
                return Err(("converterFrom".into(), "Choose a supported currency".into()));
            }
            if !codes.contains(&to) {
                return Err(("converterTo".into(), "Choose a supported currency".into()));
            }
            if input.len() > 256 {
                return Err((
                    "converterInput".into(),
                    "Currency amount exceeds 256 bytes".into(),
                ));
            }
            let canonical =
                decimal::normalize(&input).map_err(|error| ("converterInput".into(), error))?;
            if canonical.starts_with('-') {
                return Err((
                    "converterInput".into(),
                    "Negative values are not supported in this category".into(),
                ));
            }
            if let Some(snapshot) = &state.currency {
                output.result = Some(
                    currency::convert(snapshot, &from, &to, &input)
                        .map_err(|error| ("converterInput".into(), error))?,
                );
                output.rate = Some(
                    currency::convert(snapshot, &from, &to, "1")
                        .map_err(|error| ("converterInput".into(), error))?,
                );
            }
            Ok(())
        })();
        state.converter = Some(output);
        computed
    } else {
        let description = converter::describe(&category, &from, &to, &input, locale)
            .map_err(|error| ("converterCategory".into(), error))?;
        for (key, selection) in [("converterFrom", &from), ("converterTo", &to)] {
            if !description
                .units
                .iter()
                .any(|unit| unit.id == *selection && !unit.whimsical)
            {
                state.converter = Some(description);
                return Err((key.into(), "Choose a supported unit".into()));
            }
        }
        match converter::convert(&category, &from, &to, &input, locale) {
            Ok(output) => {
                state.converter = Some(output);
                Ok(())
            }
            Err(error) => {
                state.converter = Some(description);
                Err(("converterInput".into(), error))
            }
        }
    }
}
pub fn process(input: &str) -> Result<String, String> {
    if !bounded(input, 1_048_576) {
        return Err("Calculator request exceeds limit".into());
    }
    let request: Request = serde_json::from_str(input).map_err(|_| "Invalid calculator request")?;
    if request.version != 2
        || !["zh-CN", "en"].contains(&request.locale.as_str())
        || request.fields.len() > 32
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
    upgrade_state(&mut state);
    if !valid_state(&state) {
        return Err("Incompatible calculator session; original retained".into());
    }
    state.result = decimal::normalize(&state.result)
        .map_err(|_| "Invalid calculator result; original retained")?;
    let previous_converter_category = state.fields["converterCategory"].clone();
    // Explicit axis edits take priority over automatic fitting. Ordinary pan,
    // zoom, repeated plots, locale refreshes and restored sessions retain them.
    let auto_fit_plot = request.action.as_deref() == Some("plot")
        && ["functions", "angle"].iter().any(|key| {
            request
                .fields
                .get(*key)
                .is_some_and(|value| value != &state.fields[*key])
        })
        && ["xmin", "xmax", "ymin", "ymax"].iter().all(|key| {
            request.fields.get(*key).is_none_or(|value| {
                value.trim().parse::<f64>().ok() == state.fields[*key].trim().parse::<f64>().ok()
            })
        });
    for (key, value) in &request.fields {
        if (!state.fields.contains_key(key)
            && !["operand", "left", "right", "operator", "memoryIndex"].contains(&key.as_str()))
            || !bounded(value, field_limit(key))
        {
            return Err("Invalid calculator field".into());
        }
        if !["operand", "left", "right", "operator", "memoryIndex"].contains(&key.as_str()) {
            state.fields.insert(key.clone(), value.clone());
        }
    }
    if !["rad", "deg", "grad"].contains(&state.fields["angle"].as_str())
        || !state.fields["curve"]
            .parse::<usize>()
            .is_ok_and(|curve| curve < 8)
        || ![
            "standard",
            "scientific",
            "graph",
            "programmer",
            "date",
            "converter",
        ]
        .contains(&state.fields["mode"].as_str())
        || !["2", "8", "10", "16"].contains(&state.fields["radix"].as_str())
        || !["8", "16", "32", "64"].contains(&state.fields["bits"].as_str())
        || !["difference", "add", "subtract"].contains(&state.fields["dateOperation"].as_str())
        || !converter::CATEGORIES.contains(&state.fields["converterCategory"].as_str())
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
    if (state.pending || state.currency_pending.is_some()) && action != Some("host-result") {
        return Err("A save is awaiting its matching host receipt".into());
    }
    match action {
        None | Some("plot") | Some("fit-plot") | Some("converter-convert") => {}
        Some("currency-refresh") => {
            let granted = request.host.as_ref().is_some_and(|host| {
                host.permissions
                    .iter()
                    .any(|permission| permission == "network:https")
            });
            if !granted {
                state.notice = tr(zh,"更新汇率需要可选的 HTTPS 网络权限；离线单位换算仍可使用。","Refreshing rates requires the optional HTTPS network permission; offline unit conversion remains available.").into();
            } else {
                state.currency_sequence = state
                    .currency_sequence
                    .checked_add(1)
                    .ok_or("Currency request sequence is exhausted; workspace retained")?;
                state.currency_pending = Some(state.currency_sequence);
                effects.push(json!({"kind":"http","id":format!("currency-rates-{}",state.currency_sequence),"request":{"url":currency::URL,"method":"GET","headers":{"Accept":"application/json"},"body":null,"credential":null}}));
                state.notice = tr(zh, "正在更新汇率…", "Updating exchange rates…").into();
            }
        }
        Some("calculate") | Some("evaluate") | Some("standard-calculate") => {
            let source = if action == Some("evaluate") {
                request
                    .fields
                    .get("operand")
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(&state.fields["expression"])
            } else {
                &state.fields["expression"]
            }
            .trim()
            .to_owned();
            let standard =
                action == Some("standard-calculate") || state.fields["mode"] == "standard";
            match exact_value(&state, &source, standard) {
                Ok(value) if action != Some("evaluate") => {
                    record_exact(&mut state, source, value, standard)
                }
                Ok(value) => state.result = value,
                Err(error) => {
                    errors.insert("expression".into(), error);
                }
            }
        }
        Some("standard-evaluate") | Some("standard-unary") => {
            let operand = request
                .fields
                .get("operand")
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(&state.result);
            let operation = request
                .fields
                .get("operator")
                .map(String::as_str)
                .unwrap_or("");
            let computed = if action == Some("standard-unary") {
                decimal::unary(operand, operation)
            } else if let (Some(left), Some(right)) = (
                request
                    .fields
                    .get("left")
                    .filter(|value| !value.trim().is_empty()),
                request
                    .fields
                    .get("right")
                    .filter(|value| !value.trim().is_empty()),
            ) {
                decimal::binary(left, operation, right)
            } else {
                decimal::standard(&state.fields["expression"])
            };
            match computed {
                Ok(value) => state.result = value,
                Err(error) => {
                    errors.insert("expression".into(), error);
                }
            }
        }
        Some("programmer-clear") => {
            let radix = state.fields["radix"]
                .parse()
                .map_err(|_| "Invalid programmer radix")?;
            let bits = state.fields["bits"]
                .parse()
                .map_err(|_| "Invalid programmer word size")?;
            let output = programmer::evaluate("0", radix, bits)?;
            state.result = output.dec.clone();
            state.fields.insert("expression".into(), "0".into());
            state.carry = false;
            state.programmer = Some(output);
        }
        Some("programmer-evaluate")
        | Some("programmer-preview")
        | Some("programmer-toggle-bit") => {
            let radix = state.fields["radix"]
                .parse()
                .map_err(|_| "Invalid programmer radix")?;
            let bits = state.fields["bits"]
                .parse()
                .map_err(|_| "Invalid programmer word size")?;
            let source = if action == Some("programmer-preview") {
                request
                    .fields
                    .get("operand")
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(&state.fields["expression"])
            } else {
                &state.fields["expression"]
            };
            let computed = if action == Some("programmer-toggle-bit") {
                request
                    .fields
                    .get("operand")
                    .ok_or_else(|| "Invalid bit index".to_owned())
                    .and_then(|value| {
                        value
                            .parse::<u32>()
                            .map_err(|_| "Invalid bit index".to_owned())
                    })
                    .and_then(|index| {
                        programmer::toggle_bit(source, radix, bits, index).map(|mut output| {
                            output.carry = state.carry;
                            output
                        })
                    })
            } else {
                programmer::evaluate_with_carry(source, radix, bits, state.carry)
            };
            match computed {
                Ok(value) => {
                    state.result = value.dec.clone();
                    if action != Some("programmer-preview") {
                        state.carry = value.carry;
                    }
                    if action == Some("programmer-toggle-bit") {
                        state
                            .fields
                            .insert("expression".into(), value.value.clone());
                    }
                    state.programmer = Some(value);
                }
                Err(error) => {
                    errors.insert("expression".into(), error);
                }
            }
        }
        Some("date-difference") | Some("date-offset") => {
            let computed = if action == Some("date-difference") {
                date::difference(&state.fields["dateFrom"], &state.fields["dateTo"])
            } else {
                (|| {
                    if !["add", "subtract"].contains(&state.fields["dateOperation"].as_str()) {
                        return Err("Choose add or subtract for date offset".into());
                    }
                    let get = |key: &str| {
                        state.fields[key]
                            .parse::<u32>()
                            .map_err(|_| "Date offset must be between 0 and 999".to_owned())
                    };
                    date::offset(
                        &state.fields["dateFrom"],
                        get("dateYears")?,
                        get("dateMonths")?,
                        get("dateDays")?,
                        state.fields["dateOperation"] == "subtract",
                    )
                })()
            };
            match computed {
                Ok(value) => state.date = Some(value),
                Err(error) => {
                    errors.insert("dateFrom".into(), error);
                }
            }
        }
        Some("memory-store") | Some("memory-add") | Some("memory-subtract") => {
            let source = request
                .fields
                .get("operand")
                .filter(|value| !value.trim().is_empty())
                .or_else(|| request.fields.get("expression"))
                .unwrap_or(&state.result);
            match exact_value(&state, source, state.fields["mode"] == "standard") {
                Ok(value) => {
                    let index = request
                        .fields
                        .get("memoryIndex")
                        .filter(|value| !value.is_empty())
                        .map(|value| value.parse::<usize>())
                        .transpose();
                    match index {
                        Ok(index) => {
                            let index = index.unwrap_or(0);
                            if action == Some("memory-store") {
                                if state.memories.len() >= MAX_MEMORIES {
                                    errors
                                        .insert("expression".into(), "Memory limit reached".into());
                                } else {
                                    state.memories.insert(0, value);
                                }
                            } else if index >= state.memories.len()
                                && (!state.memories.is_empty() || index != 0)
                            {
                                errors.insert(
                                    "memoryIndex".into(),
                                    "Choose an existing memory item".into(),
                                );
                            } else {
                                let previous =
                                    state.memories.get(index).map(String::as_str).unwrap_or("0");
                                match decimal::binary(
                                    previous,
                                    if action == Some("memory-add") {
                                        "+"
                                    } else {
                                        "-"
                                    },
                                    &value,
                                ) {
                                    Ok(value) => {
                                        if state.memories.is_empty() {
                                            state.memories.push(value);
                                        } else {
                                            state.memories[index] = value;
                                        }
                                    }
                                    Err(error) => {
                                        errors.insert("expression".into(), error);
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            errors.insert(
                                "memoryIndex".into(),
                                "Choose an existing memory item".into(),
                            );
                        }
                    }
                }
                Err(error) => {
                    errors.insert("expression".into(), error);
                }
            }
        }
        Some("memory-recall") | Some("memory-remove") => {
            let index = request
                .fields
                .get("memoryIndex")
                .filter(|value| !value.is_empty())
                .map(String::as_str)
                .unwrap_or("0")
                .parse::<usize>();
            match index.ok().filter(|&index| index < state.memories.len()) {
                Some(index) if action == Some("memory-recall") => {
                    state.result = state.memories[index].clone()
                }
                Some(index) => {
                    state.memories.remove(index);
                }
                None => {
                    errors.insert(
                        "memoryIndex".into(),
                        "Choose an existing memory item".into(),
                    );
                }
            }
        }
        Some("memory-clear") => {
            state.memories.clear();
        }
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
                    math::root(&scientific_source(&state, source), a, b, base)?
                } else {
                    math::integrate(&scientific_source(&state, source), a, b, base)?
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
                    let mut old: State = serde_json::from_value(value.clone())
                        .map_err(|_| "Saved calculator data is invalid; original retained")?;
                    upgrade_state(&mut old);
                    if !valid_state(&old) || old.pending || old.currency_pending.is_some() {
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
                    let mut saved: State = serde_json::from_value(value.clone())
                        .map_err(|_| "Saved calculator data is invalid; original retained")?;
                    upgrade_state(&mut saved);
                    if !valid_state(&saved) || saved.pending || saved.currency_pending.is_some() {
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
            if let Some(sequence) = state.currency_pending {
                let receipt = request.results.first().ok_or("Missing currency receipt")?;
                if receipt.id != format!("currency-rates-{sequence}")
                    || receipt.value.is_some() == receipt.error.is_some()
                {
                    return Err("Invalid currency receipt".into());
                }
                state.currency_pending = None;
                if let Some(value) = &receipt.value {
                    match currency::from_http(value) {
                        Ok(snapshot)
                            if state
                                .currency
                                .as_ref()
                                .is_none_or(|old| snapshot.updated_at >= old.updated_at) =>
                        {
                            state.currency = Some(snapshot);
                            state.notice = tr(zh, "汇率已更新。", "Exchange rates updated.").into();
                        }
                        Ok(_) => {
                            errors.insert(
                                "converterInput".into(),
                                "Currency source returned older rates; previous cache retained"
                                    .into(),
                            );
                        }
                        Err(error) => {
                            errors.insert("converterInput".into(), error);
                        }
                    }
                } else {
                    state.notice = tr(zh,"汇率更新失败，已有缓存已保留；可稍后手动重试。","Rate refresh failed. Existing cached rates were retained; retry manually later.").into();
                }
            } else {
                if !state.pending {
                    return Err("Unexpected storage receipt".into());
                }
                let receipt = request.results.first().ok_or("Missing save receipt")?;
                if receipt.id != "save-workspace"
                    || receipt.value.is_some() == receipt.error.is_some()
                {
                    return Err("Invalid save receipt".into());
                }
                if let Some(value) = &receipt.value {
                    let mut saved: State =
                        serde_json::from_value(value["values"][STORAGE_KEY].clone())
                            .map_err(|_| "Invalid successful save receipt")?;
                    upgrade_state(&mut saved);
                    if value["revision"].as_i64()
                        != state
                            .pending_revision
                            .map(|revision| i64::from(revision) + 1)
                        || !valid_state(&saved)
                        || saved.pending
                        || saved.ans != state.ans
                        || saved.ans_exact != state.ans_exact
                        || saved.result != state.result
                        || saved.memory != state.memory
                        || saved.memory_set != state.memory_set
                        || saved.memories != state.memories
                        || saved.carry != state.carry
                        || saved.programmer != state.programmer
                        || saved.date != state.date
                        || saved.currency != state.currency
                        || saved.currency_sequence != state.currency_sequence
                        || saved.currency_pending.is_some()
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
        }
        Some("export") => {
            let mut body = format!("# {}\n\n", tr(zh, "计算记录", "Calculation history"));
            for entry in &state.history {
                body.push_str(&format!(
                    "- `{}` = **{}** ({})\n",
                    entry.expression.replace('`', "'"),
                    entry.exact,
                    entry.angle
                ));
            }
            artifacts.push(json!({"name":"calculator-history.md","mediaType":"text/markdown","encoding":"utf8","content":body}));
        }
        Some(_) => return Err("Unknown calculator action".into()),
    }
    sync_memory(&mut state);
    if state.fields["mode"] == "converter"
        || action == Some("converter-convert")
        || action == Some("currency-refresh")
    {
        let reset_units = previous_converter_category != state.fields["converterCategory"];
        if let Err((key, error)) = update_converter(&mut state, &request.locale, reset_units) {
            errors.insert(key, error);
        }
    }
    let fit_y = auto_fit_plot || action == Some("fit-plot");
    match plot(&state, fit_y) {
        Ok(value) => {
            if fit_y {
                state.fields.insert("ymin".into(), value.ymin.to_string());
                state.fields.insert("ymax".into(), value.ymax.to_string());
            }
            if action == Some("fit-plot") && fitted_y_range(&value.series).is_none() {
                state.notice = tr(
                    zh,
                    "当前范围内没有可绘制的采样点。请检查函数与坐标范围。",
                    "No drawable samples in this range. Check the functions and axis limits.",
                )
                .into();
            }
            state.plot = value;
        }
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
        saved.converter = None;
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
        ("mode", "计算模式", "Calculator mode", "select"),
        ("radix", "进制", "Number base", "select"),
        ("bits", "字长", "Word size", "select"),
        ("dateFrom", "起始日期", "From date", "date"),
        ("dateTo", "结束日期", "To date", "date"),
        ("dateYears", "年", "Years", "text"),
        ("dateMonths", "月", "Months", "text"),
        ("dateDays", "日", "Days", "text"),
        ("dateOperation", "日期操作", "Date operation", "select"),
        (
            "converterCategory",
            "换算类别",
            "Conversion category",
            "select",
        ),
        ("converterFrom", "源单位", "From unit", "text"),
        ("converterTo", "目标单位", "To unit", "text"),
        ("converterInput", "换算数值", "Conversion value", "text"),
    ];
    let mut fields:Vec<_>=labels.iter().map(|(id,cn,en,kind)|{
        let mut value=json!({"id":id,"label":tr(zh,cn,en),"value":state.fields[*id],"kind":kind,"maxBytes":field_limit(id)});
        if *id=="angle" {value["options"]=json!([{"value":"deg","label":"DEG"},{"value":"rad","label":"RAD"},{"value":"grad","label":"GRAD"}]);}
        if *id=="curve" {value["options"]=json!((0..8).map(|index|json!({"value":index.to_string(),"label":format!("f{}",index+1)})).collect::<Vec<_>>());}
        if *id=="mode" { value["options"]=json!([("standard","标准","Standard"),("scientific","科学","Scientific"),("graph","绘图","Graphing"),("programmer","程序员","Programmer"),("date","日期计算","Date calculation"),("converter","转换器","Converter")].map(|(value,cn,en)|json!({"value":value,"label":tr(zh,cn,en)}))); }
        if *id=="converterCategory" { value["options"]=json!(converter::CATEGORIES.iter().map(|category|json!({"value":category,"label":if *category == "currency" {tr(zh,"货币","Currency").to_owned()} else {converter::category_label(category,&request.locale).unwrap_or_else(|_|category.to_string())}})).collect::<Vec<_>>()); }
        if *id=="radix" { value["options"]=json!([("16","HEX"),("10","DEC"),("8","OCT"),("2","BIN")].map(|(value,label)|json!({"value":value,"label":label}))); }
        if *id=="bits" { value["options"]=json!([("64","QWORD"),("32","DWORD"),("16","WORD"),("8","BYTE")].map(|(value,label)|json!({"value":value,"label":label}))); }
        if *id=="dateOperation" { value["options"]=json!([("difference","日期差","Difference"),("add","增加","Add"),("subtract","减去","Subtract")].map(|(value,cn,en)|json!({"value":value,"label":tr(zh,cn,en)}))); }
        value
    }).collect();
    for (id, cn, en) in [
        ("operand", "当前操作数", "Current operand"),
        ("left", "左操作数", "Left operand"),
        ("right", "右操作数", "Right operand"),
        ("operator", "运算", "Operation"),
        ("memoryIndex", "记忆位置", "Memory position"),
    ] {
        fields.push(json!({"id":id,"label":tr(zh,cn,en),"value":request.fields.get(id).map(String::as_str).unwrap_or(""),"kind":"text","maxBytes":if ["operand","left","right"].contains(&id) {4096} else {80}}));
    }
    let actions = [
        ("calculate", "计算", "Calculate"),
        ("evaluate", "预览", "Preview"),
        ("converter-convert", "换算", "Convert units"),
        ("currency-refresh", "更新汇率", "Update rates"),
        (
            "standard-evaluate",
            "即时计算",
            "Evaluate standard operation",
        ),
        ("standard-unary", "单目计算", "Evaluate standard function"),
        (
            "standard-calculate",
            "标准计算",
            "Calculate standard expression",
        ),
        (
            "programmer-evaluate",
            "整数计算",
            "Evaluate integer expression",
        ),
        ("programmer-preview", "整数预览", "Preview integer operand"),
        ("programmer-clear", "清除整数输入", "Clear integer input"),
        ("programmer-toggle-bit", "切换位", "Toggle bit"),
        ("date-difference", "计算日期差", "Calculate date difference"),
        ("date-offset", "计算目标日期", "Calculate target date"),
        ("memory-store", "MS", "MS"),
        ("memory-add", "M+", "M+"),
        ("memory-subtract", "M−", "M−"),
        ("memory-recall", "MR", "MR"),
        ("memory-remove", "清除此记忆", "Clear this memory"),
        ("memory-clear", "MC", "MC"),
        ("history-clear", "清空历史", "Clear history"),
        ("plot", "绘图", "Plot"),
        ("fit-plot", "适配 Y 轴", "Fit Y axis"),
        ("reset-plot", "重置坐标", "Reset axes"),
        ("root", "区间求根", "Find root"),
        ("integrate", "定积分", "Integrate"),
        ("export", "导出记录", "Export history"),
        ("save", "保存工作区", "Save workspace"),
        ("load", "载入工作区", "Load workspace"),
    ];
    let buttons:Vec<_>=actions.iter().map(|(id,cn,en)|json!({"id":id,"label":tr(zh,cn,en),"disabled":state.pending||state.currency_pending.is_some()||(["save","load"].contains(id)&&store.is_none()),"primary":*id=="calculate"})).collect();
    let description = if !state.notice.is_empty() {
        state.notice.clone()
    } else {
        tr(
            zh,
            "标准 · 科学 · 绘图 · 程序员 · 日期计算",
            "Standard · Scientific · Graphing · Programmer · Date calculation",
        )
        .into()
    };
    let encoded_state =
        serde_json::to_string(&state).map_err(|_| "Cannot encode calculator state")?;
    if encoded_state.len() > MAX_STATE_BYTES {
        return Err("Calculator workspace exceeds the session limit; original retained".into());
    }
    let metric = |value: &str| {
        if value.len() <= 512 {
            value.to_owned()
        } else {
            format!("{}…", &value[..509])
        }
    };
    let displayed_result = if state.fields["mode"] == "converter" {
        state
            .converter
            .as_ref()
            .and_then(|output| output.result.clone())
            .unwrap_or_else(|| "—".into())
    } else if state.fields["mode"] == "date" {
        state
            .date
            .as_ref()
            .map(|value| {
                if state.fields["dateOperation"] == "difference" {
                    value.total_days.to_string()
                } else {
                    value.date.clone()
                }
            })
            .unwrap_or_else(|| state.result.clone())
    } else {
        state.result.clone()
    };
    serde_json::to_string(&json!({"version":2,"state":encoded_state,"display":tr(zh,"计算器","Calculator"),"description":description,"fields":fields,"groups":[{"columns":4,"actions":buttons}],"blocks":[{"kind":"metrics","items":[{"label":tr(zh,"结果","Result"),"value":metric(&displayed_result)},{"label":tr(zh,"记忆","Memory"),"value":metric(state.memories.first().map(String::as_str).unwrap_or("0"))},{"label":tr(zh,"历史","History"),"value":state.history.len().to_string()}]}],"effects":effects,"errors":errors,"artifacts":artifacts})).map_err(|_|"Cannot encode calculator view".into())
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
        let mut supplied = json!({"mode":"scientific"});
        supplied
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        serde_json::from_str(
            &process(
                &json!({"version":2,"action":action,"state":state,"fields":supplied,"locale":"en"})
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
        assert_eq!(view["display"], "Calculator");
    }
    #[test]
    fn calculator_and_converters_fit_the_host_field_budget() {
        let view: Value = serde_json::from_str(
            &process(
                &json!({"version":2,"action":null,"state":"","fields":{},"locale":"en"})
                    .to_string(),
            )
            .unwrap(),
        )
        .unwrap();
        let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.fields["mode"], "standard");
        assert_eq!(state.fields["dateFrom"], "");
        assert_eq!(view["fields"].as_array().unwrap().len(), 28);
        assert!(view["errors"].as_object().unwrap().is_empty());
    }
    #[test]
    fn converters_preserve_input_direction_and_decimal_entry_across_requests() {
        let converted = call(
            Some("converter-convert"),
            "",
            json!({"mode":"converter","converterCategory":"length","converterFrom":"34","converterTo":"33","converterInput":"1."}),
        );
        let state: Value = serde_json::from_str(converted["state"].as_str().unwrap()).unwrap();
        assert_eq!(converted["errors"], json!({}));
        assert_eq!(state["converter"]["input"], "1.");
        assert_eq!(state["fields"]["converterInput"], "1.");
        assert_eq!(state["converter"]["result"], "0.001");
        let converted = call(
            Some("converter-convert"),
            converted["state"].as_str().unwrap(),
            json!({"mode":"converter","converterInput":"1.2"}),
        );
        let state: Value = serde_json::from_str(converted["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["converter"]["result"], "0.0012");
        let converted = call(
            Some("converter-convert"),
            converted["state"].as_str().unwrap(),
            json!({"mode":"converter","converterCategory":"volume","converterFrom":"74","converterTo":"75","converterInput":"2"}),
        );
        let state: Value = serde_json::from_str(converted["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["converter"]["result"], "2000");
        let reversed = call(
            Some("converter-convert"),
            converted["state"].as_str().unwrap(),
            json!({"mode":"converter","converterFrom":"75","converterTo":"74","converterInput":"2000"}),
        );
        let state: Value = serde_json::from_str(reversed["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["converter"]["result"], "2");
        let refreshed = call(
            None,
            reversed["state"].as_str().unwrap(),
            json!({"mode":"converter"}),
        );
        assert_eq!(refreshed["errors"], json!({}));
    }
    #[test]
    fn every_offline_converter_returns_a_roundtrippable_state_and_no_network_effect() {
        for category in converter::CATEGORIES
            .into_iter()
            .filter(|category| *category != "currency")
        {
            let converted = call(
                Some("converter-convert"),
                "",
                json!({"mode":"converter","converterCategory":category,"converterInput":"1","converterFrom":"","converterTo":""}),
            );
            let state: Value = serde_json::from_str(converted["state"].as_str().unwrap()).unwrap();
            assert_eq!(converted["errors"], json!({}), "{category}");
            assert_eq!(converted["effects"], json!([]));
            assert!(state["converter"]["result"].is_string());
            assert_eq!(state["converter"]["category"], category);
            let refreshed = call(
                None,
                converted["state"].as_str().unwrap(),
                json!({"mode":"converter"}),
            );
            assert_eq!(refreshed["errors"], json!({}), "{category}");
        }
    }
    fn currency_call(
        action: &str,
        state: &str,
        fields: Value,
        results: Value,
        granted: bool,
    ) -> Value {
        serde_json::from_str(&process(&json!({"version":2,"action":action,"state":state,"fields":fields,"locale":"en","results":results,"host":{"permissions":if granted {vec!["network:https"]} else {vec![]},"storage":null}}).to_string()).unwrap()).unwrap()
    }
    fn fake_currency_receipt(id: &str, updated_at: u64) -> Value {
        json!([{"id":id,"error":null,"value":{"url":currency::URL,"status":200,"headers":{"content-type":"application/json"},"body":json!({"result":"success","base_code":"USD","time_last_update_unix":updated_at,"time_next_update_unix":updated_at+86_400,"rates":{"USD":1,"EUR":0.8,"CNY":7.2,"JPY":160}}).to_string()}}])
    }
    #[test]
    fn currency_only_fetches_explicitly_and_consumes_the_matching_receipt() {
        let fields = json!({"mode":"converter","converterCategory":"currency","converterFrom":"USD","converterTo":"CNY","converterInput":"12.5"});
        let offline = currency_call("converter-convert", "", fields.clone(), json!([]), false);
        let state: Value = serde_json::from_str(offline["state"].as_str().unwrap()).unwrap();
        assert_eq!(offline["effects"], json!([]));
        assert!(state["converter"]["result"].is_null());
        assert_eq!(state["converter"]["units"].as_array().unwrap().len(), 166);
        let denied = currency_call(
            "currency-refresh",
            offline["state"].as_str().unwrap(),
            fields.clone(),
            json!([]),
            false,
        );
        assert_eq!(denied["effects"], json!([]));
        let pending = currency_call(
            "currency-refresh",
            offline["state"].as_str().unwrap(),
            fields,
            json!([]),
            true,
        );
        assert_eq!(pending["effects"][0]["request"]["url"], currency::URL);
        let id = pending["effects"][0]["id"].as_str().unwrap();
        let confirmed = currency_call(
            "host-result",
            pending["state"].as_str().unwrap(),
            json!({}),
            fake_currency_receipt(id, 1_700_000_000),
            true,
        );
        let state: Value = serde_json::from_str(confirmed["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["converter"]["result"], "90");
        assert_eq!(state["converter"]["rate"], "7.2");
        assert_eq!(state["converter"]["updatedAt"], 1_700_000_000_u64);
        assert!(state["currency_pending"].is_null());
        assert_eq!(confirmed["effects"], json!([]));
        assert!(process(&json!({"version":2,"action":"host-result","state":pending["state"],"fields":{},"locale":"en","results":fake_currency_receipt("currency-rates-other",1_700_000_000)}).to_string()).is_err());
    }
    #[test]
    fn currency_failures_preserve_cache_storage_and_export_never_contains_raw_rates() {
        let pending = currency_call(
            "currency-refresh",
            "",
            json!({"mode":"converter","converterCategory":"currency","converterFrom":"USD","converterTo":"CNY","converterInput":"10"}),
            json!([]),
            true,
        );
        let confirmed = currency_call(
            "host-result",
            pending["state"].as_str().unwrap(),
            json!({}),
            fake_currency_receipt("currency-rates-1", 1_700_000_000),
            true,
        );
        let saved = with_storage(
            "save",
            confirmed["state"].as_str().unwrap(),
            json!({}),
            json!([]),
        )
        .unwrap();
        assert_eq!(
            saved["effects"][0]["changes"][STORAGE_KEY]["currency"]["rates"]["CNY"],
            "7.2"
        );
        let exported = call(
            Some("export"),
            confirmed["state"].as_str().unwrap(),
            json!({"mode":"converter"}),
        );
        assert!(
            !exported["artifacts"][0]["content"]
                .as_str()
                .unwrap()
                .contains("rates")
        );
        let retry = currency_call(
            "currency-refresh",
            confirmed["state"].as_str().unwrap(),
            json!({}),
            json!([]),
            true,
        );
        let failed = currency_call(
            "host-result",
            retry["state"].as_str().unwrap(),
            json!({}),
            json!([{"id":"currency-rates-2","value":null,"error":{"code":"HTTP_TIMEOUT","message":"offline","retryable":false}}]),
            true,
        );
        let state: Value = serde_json::from_str(failed["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["converter"]["result"], "72");
        assert_eq!(state["currency"]["rates"]["CNY"], "7.2");
        let retry = currency_call(
            "currency-refresh",
            failed["state"].as_str().unwrap(),
            json!({}),
            json!([]),
            true,
        );
        let stale = currency_call(
            "host-result",
            retry["state"].as_str().unwrap(),
            json!({}),
            fake_currency_receipt("currency-rates-3", 1_600_000_000),
            true,
        );
        assert!(stale["errors"]["converterInput"].is_string());
        let state: Value = serde_json::from_str(stale["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["currency"]["updatedAt"], 1_700_000_000_u64);
    }
    #[test]
    fn exact_state_literals_are_checked_without_expanding_large_powers() {
        for value in [
            "0",
            "-0.1",
            ".5",
            "1e9999",
            "1e-9999",
            "-123456789012345678901234567890",
        ] {
            assert!(valid_exact(value), "{value}");
        }
        for value in [
            "",
            ".",
            "NaN",
            "inf",
            "1+2",
            "1e10000",
            "9999e9999",
            "2e-10000",
            "1ee2",
            "1.2.3",
        ] {
            assert!(!valid_exact(value), "{value}");
        }
    }
    #[test]
    fn standard_immediate_order_differs_from_scientific_order() {
        let standard = call(
            Some("calculate"),
            "",
            json!({"mode":"standard","expression":"2+3*4"}),
        );
        let scientific = call(
            Some("calculate"),
            "",
            json!({"mode":"scientific","expression":"2+3*4"}),
        );
        assert_eq!(standard["blocks"][0]["items"][0]["value"], "20");
        assert_eq!(scientific["blocks"][0]["items"][0]["value"], "14");
        let percentage = call(
            Some("standard-evaluate"),
            "",
            json!({"mode":"standard","left":"200","operator":"+","right":"10%"}),
        );
        assert_eq!(percentage["blocks"][0]["items"][0]["value"], "220");
        let reciprocal = call(
            Some("standard-unary"),
            "",
            json!({"mode":"standard","operand":"4","operator":"reciprocal"}),
        );
        assert_eq!(reciprocal["blocks"][0]["items"][0]["value"], "0.25");
    }
    #[test]
    fn standard_completed_expressions_keep_their_order_in_followup_operations() {
        let completed = call(
            Some("standard-calculate"),
            "",
            json!({"mode":"standard","expression":"2+3*4"}),
        );
        let completed_state = completed["state"].as_str().unwrap();
        for (operation, expected) in [
            ("sqrt", "4.4721359549995793928183473374626"),
            ("square", "400"),
            ("reciprocal", "0.05"),
            ("negate", "-20"),
            ("percent", "0.2"),
        ] {
            let preview = call(
                Some("standard-unary"),
                completed_state,
                json!({"operand":"2+3*4","operator":operation}),
            );
            assert_eq!(preview["errors"], json!({}));
            let preview: State = serde_json::from_str(preview["state"].as_str().unwrap()).unwrap();
            assert_eq!(preview.result, expected, "{operation}");
            assert_eq!(preview.fields["expression"], "2+3*4");
            assert_eq!(preview.ans_exact, "20");
            assert_eq!(preview.history.len(), 1);
        }
        for (left, operation, right, expected) in [
            ("2+3*4", "+", "5", "25"),
            ("5", "+", "2+3*4", "25"),
            ("2+3*4", "percent", "10", "2"),
            ("2+3*4", "+", "10%", "22"),
            ("1/3", "*", "3", "1"),
        ] {
            let preview = call(
                Some("standard-evaluate"),
                completed_state,
                json!({"left":left,"operator":operation,"right":right}),
            );
            assert_eq!(preview["errors"], json!({}));
            let preview: State = serde_json::from_str(preview["state"].as_str().unwrap()).unwrap();
            assert_eq!(preview.result, expected);
        }
        let mut view = completed;
        for (expression, expected) in [("(2+3*4)+(5)", "25"), ("((2+3*4)+(5))+(5)", "30")] {
            view = call(
                Some("standard-calculate"),
                view["state"].as_str().unwrap(),
                json!({"expression":expression}),
            );
            let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
            assert_eq!(state.result, expected);
            assert_eq!(state.history[0].expression, expression);
        }
    }
    #[test]
    fn scientific_fallback_preserves_decimal_failures_and_rejects_underflow() {
        for expression in [
            "1e-9999+1e-9999",
            "sqrt(1e-9999)",
            "(1e-200)^5000",
            "sin(1e-9999)",
        ] {
            let view = call(
                Some("calculate"),
                "",
                json!({"mode":"scientific","expression":expression}),
            );
            assert!(view["errors"]["expression"].is_string(), "{expression}");
            let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
            assert!(state.history.is_empty(), "{expression}");
            assert_eq!(state.ans_exact, "0");
        }
        for (expression, expected) in [
            ("sin(30)", "0.5"),
            ("2^0.5", "1.414213562373"),
            ("atan2(1,1)", "45"),
            ("2*pi", "6.28318530718"),
            ("0^0", "1"),
        ] {
            let view = call(
                Some("calculate"),
                "",
                json!({"mode":"scientific","expression":expression}),
            );
            assert_eq!(view["errors"], json!({}), "{expression}");
            let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
            assert_eq!(state.result, expected, "{expression}");
        }
    }
    #[test]
    fn formal_history_uses_the_calculation_engine_mode_and_roundtrips() {
        for mode in ["date", "programmer", "graph"] {
            for action in ["calculate", "standard-calculate"] {
                let view = call(Some(action), "", json!({"mode":mode,"expression":"2+3*4"}));
                let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
                assert_eq!(view["errors"], json!({}));
                assert_eq!(state.fields["mode"], mode);
                assert_eq!(state.history.len(), 1);
                assert_eq!(
                    state.history[0].mode,
                    if action == "standard-calculate" {
                        "standard"
                    } else {
                        "scientific"
                    }
                );
                assert_eq!(
                    state.result,
                    if action == "standard-calculate" {
                        "20"
                    } else {
                        "14"
                    }
                );
                assert!(valid_state(&state));
                let refreshed = call(None, view["state"].as_str().unwrap(), json!({"mode":mode}));
                let refreshed: State =
                    serde_json::from_str(refreshed["state"].as_str().unwrap()).unwrap();
                assert_eq!(refreshed.history, state.history);
            }
        }
        let view = call(
            Some("standard-calculate"),
            "",
            json!({"mode":"scientific","expression":"2+3*4"}),
        );
        let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.history[0].mode, "standard");
        assert_eq!(state.result, "20");
    }
    #[test]
    fn standard_precision_survives_history_memory_and_storage() {
        let value = call(
            Some("calculate"),
            "",
            json!({"mode":"standard","expression":"1234567890123456789012345678901234567890+0.1+0.2"}),
        );
        let expected = "1234567890123456789012345678901234567890.3";
        let state: State = serde_json::from_str(value["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.result, expected);
        assert_eq!(state.ans_exact, expected);
        assert_eq!(state.history[0].exact, expected);
        let stored = call(
            Some("memory-store"),
            value["state"].as_str().unwrap(),
            json!({"mode":"standard","operand":expected}),
        );
        let saved = with_storage(
            "save",
            stored["state"].as_str().unwrap(),
            json!({}),
            json!([]),
        )
        .unwrap();
        let loaded = with_storage(
            "load",
            "",
            json!({STORAGE_KEY:saved["effects"][0]["changes"][STORAGE_KEY]}),
            json!([]),
        )
        .unwrap();
        let state: State = serde_json::from_str(loaded["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.memories, vec![expected]);
        assert_eq!(state.result, expected);
        let exact = call(
            Some("standard-evaluate"),
            "",
            json!({"mode":"standard","left":"1/3","operator":"*","right":"3"}),
        );
        assert_eq!(exact["blocks"][0]["items"][0]["value"], "1");
    }
    #[test]
    fn multiple_memories_support_selection_duplicates_and_individual_clear() {
        let first = call(Some("memory-store"), "", json!({"operand":"12"}));
        let second = call(
            Some("memory-store"),
            first["state"].as_str().unwrap(),
            json!({"operand":"12"}),
        );
        let added = call(
            Some("memory-add"),
            second["state"].as_str().unwrap(),
            json!({"operand":"0.1","memoryIndex":"1"}),
        );
        let state: State = serde_json::from_str(added["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.memories, vec!["12", "12.1"]);
        let recalled = call(
            Some("memory-recall"),
            added["state"].as_str().unwrap(),
            json!({"memoryIndex":"1"}),
        );
        assert_eq!(recalled["blocks"][0]["items"][0]["value"], "12.1");
        let removed = call(
            Some("memory-remove"),
            recalled["state"].as_str().unwrap(),
            json!({"memoryIndex":"0"}),
        );
        let state: State = serde_json::from_str(removed["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.memories, vec!["12.1"]);
        assert!(state.memory_set);
        let cleared = call(
            Some("memory-clear"),
            removed["state"].as_str().unwrap(),
            json!({}),
        );
        let state: State = serde_json::from_str(cleared["state"].as_str().unwrap()).unwrap();
        assert!(state.memories.is_empty());
        assert!(!state.memory_set);
    }
    #[test]
    fn programmer_preview_toggle_and_carry_remain_exact() {
        let view = call(
            Some("programmer-preview"),
            "",
            json!({"mode":"programmer","radix":"16","bits":"64","expression":"F+","operand":"F"}),
        );
        let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.fields["expression"], "F+");
        assert_eq!(state.programmer.unwrap().dec, "15");
        assert!(state.history.is_empty());
        let view = call(
            Some("programmer-evaluate"),
            "",
            json!({"mode":"programmer","radix":"16","bits":"64","expression":"FFFFFFFFFFFFFFFF"}),
        );
        let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.programmer.unwrap().dec, "-1");
        let toggled = call(
            Some("programmer-toggle-bit"),
            view["state"].as_str().unwrap(),
            json!({"expression":"0","operand":"63"}),
        );
        let state: State = serde_json::from_str(toggled["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.fields["expression"], "8000000000000000");
        assert_eq!(state.programmer.unwrap().dec, "-9223372036854775808");
    }
    #[test]
    fn programmer_clear_resets_carry_without_clearing_other_work() {
        let calculation = call(Some("calculate"), "", json!({"expression":"6*7"}));
        let stored = call(
            Some("memory-store"),
            calculation["state"].as_str().unwrap(),
            json!({"operand":"7"}),
        );
        let rotated = call(
            Some("programmer-evaluate"),
            stored["state"].as_str().unwrap(),
            json!({"mode":"programmer","radix":"16","bits":"8","expression":"rcl(80,1)"}),
        );
        let before: State = serde_json::from_str(rotated["state"].as_str().unwrap()).unwrap();
        assert!(before.carry);
        let cleared = call(
            Some("programmer-clear"),
            rotated["state"].as_str().unwrap(),
            json!({"mode":"programmer"}),
        );
        let state: State = serde_json::from_str(cleared["state"].as_str().unwrap()).unwrap();
        assert!(!state.carry);
        assert!(!state.programmer.unwrap().carry);
        assert_eq!(state.result, "0");
        assert_eq!(state.fields["expression"], "0");
        assert_eq!(state.fields["mode"], "programmer");
        assert_eq!(state.memories, before.memories);
        assert_eq!(state.history, before.history);
        assert_eq!(state.ans_exact, before.ans_exact);
        let rotated_again = call(
            Some("programmer-evaluate"),
            cleared["state"].as_str().unwrap(),
            json!({"mode":"programmer","expression":"rcl(0,1)"}),
        );
        let state: State = serde_json::from_str(rotated_again["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.programmer.unwrap().value, "0");
        assert!(!state.carry);
    }
    #[test]
    fn date_actions_return_calendar_results_and_validate_offsets() {
        let view = call(
            Some("date-difference"),
            "",
            json!({"mode":"date","dateFrom":"2024-02-28","dateTo":"2024-03-01"}),
        );
        let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.date.unwrap().total_days, 2);
        let view = call(
            Some("date-offset"),
            view["state"].as_str().unwrap(),
            json!({"dateFrom":"2024-01-31","dateOperation":"add","dateMonths":"1","dateDays":"1"}),
        );
        let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.date.unwrap().date, "2024-03-01");
        let invalid = call(
            Some("date-offset"),
            view["state"].as_str().unwrap(),
            json!({"dateYears":"1000"}),
        );
        assert!(invalid["errors"]["dateFrom"].is_string());
        let refreshed = call(None, view["state"].as_str().unwrap(), json!({}));
        let state: State = serde_json::from_str(refreshed["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.date.unwrap().date, "2024-03-01");

        let leap_day = call(
            Some("date-difference"),
            "",
            json!({"mode":"date","dateFrom":"2020-02-29","dateTo":"2024-02-28"}),
        );
        let refreshed: Value = serde_json::from_str(
            &process(
                &json!({"version":2,"action":null,"state":leap_day["state"],"fields":{},"locale":"zh-CN"})
                    .to_string(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(refreshed["display"], "计算器");
        assert_eq!(refreshed["errors"], json!({}));
        let next = call(
            Some("date-difference"),
            refreshed["state"].as_str().unwrap(),
            json!({"mode":"date","dateFrom":"2024-02-28","dateTo":"2024-03-01"}),
        );
        assert_eq!(next["errors"], json!({}));
        let state: State = serde_json::from_str(next["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.date.unwrap().total_days, 2);
    }
    #[test]
    fn eight_graphs_accept_y_prefix_with_a_bounded_sample_budget() {
        let source = (1..=8)
            .map(|index| format!("y = x+{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let view = call(
            Some("plot"),
            "",
            json!({"mode":"graph","functions":source,"curve":"7"}),
        );
        let state: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(view["errors"], json!({}));
        let series = state["plot"]["series"].as_array().unwrap();
        assert_eq!(series.len(), 8);
        assert!(
            series
                .iter()
                .all(|series| series["points"].as_array().unwrap().len() == 241)
        );
        assert!(view["state"].as_str().unwrap().len() <= MAX_STATE_BYTES);
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
        assert!(s.memory_set);
    }
    #[test]
    fn operand_preview_preserves_expression_answer_and_history() {
        let calculated = call(Some("calculate"), "", json!({"expression":"6*7"}));
        let preview = call(
            Some("evaluate"),
            calculated["state"].as_str().unwrap(),
            json!({"expression":"2+sqrt(9)","operand":"sqrt(9)"}),
        );
        let state: State = serde_json::from_str(preview["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.ans, 42.0);
        assert_eq!(state.result, "3");
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.fields["expression"], "2+sqrt(9)");
        assert!(!state.fields.contains_key("operand"));
        assert!(
            preview["fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field["id"] == "operand")
        );
        let refreshed = call(None, preview["state"].as_str().unwrap(), json!({}));
        let refreshed: State = serde_json::from_str(refreshed["state"].as_str().unwrap()).unwrap();
        assert_eq!(refreshed.result, "3");
        let failed = call(
            Some("evaluate"),
            preview["state"].as_str().unwrap(),
            json!({"operand":"sqrt(-1)"}),
        );
        let failed_state: State = serde_json::from_str(failed["state"].as_str().unwrap()).unwrap();
        assert_eq!(failed_state.result, "3");
        assert_eq!(failed_state.fields, state.fields);
        assert_eq!(failed_state.ans, 42.0);
        assert_eq!(failed_state.history, state.history);
        assert!(failed["errors"]["expression"].is_string());
        let full = call(
            Some("evaluate"),
            preview["state"].as_str().unwrap(),
            json!({"operand":""}),
        );
        let full: State = serde_json::from_str(full["state"].as_str().unwrap()).unwrap();
        assert_eq!(full.result, "5");
        assert_eq!(full.ans, 42.0);
    }
    #[test]
    fn memory_uses_current_operand_and_distinguishes_stored_zero() {
        let stored = call(
            Some("memory-store"),
            "",
            json!({"expression":"2+","operand":"5"}),
        );
        let added = call(
            Some("memory-add"),
            stored["state"].as_str().unwrap(),
            json!({"operand":"sqrt(9)"}),
        );
        let subtracted = call(
            Some("memory-subtract"),
            added["state"].as_str().unwrap(),
            json!({"operand":"8"}),
        );
        let state: State = serde_json::from_str(subtracted["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.memory, 0.0);
        assert!(state.memory_set);
        assert_eq!(state.ans, 0.0);
        assert!(state.history.is_empty());
        let recalled = call(
            Some("memory-recall"),
            added["state"].as_str().unwrap(),
            json!({}),
        );
        let recalled: State = serde_json::from_str(recalled["state"].as_str().unwrap()).unwrap();
        assert_eq!(recalled.result, "8");
        assert_eq!(recalled.fields["expression"], "2+");
        assert_eq!(recalled.ans, 0.0);
        let cleared = call(
            Some("memory-clear"),
            subtracted["state"].as_str().unwrap(),
            json!({}),
        );
        let cleared: State = serde_json::from_str(cleared["state"].as_str().unwrap()).unwrap();
        assert_eq!(cleared.memory, 0.0);
        assert!(!cleared.memory_set);
    }
    #[test]
    fn memory_failure_retains_register_and_preview_fallback_works() {
        let preview = call(Some("evaluate"), "", json!({"operand":"7"}));
        let stored = call(
            Some("memory-store"),
            preview["state"].as_str().unwrap(),
            json!({}),
        );
        let state: State = serde_json::from_str(stored["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.memory, 7.0);
        let bad = call(
            Some("memory-subtract"),
            stored["state"].as_str().unwrap(),
            json!({"operand":"1/0"}),
        );
        let bad_state: State = serde_json::from_str(bad["state"].as_str().unwrap()).unwrap();
        assert_eq!(bad_state.memory, 7.0);
        assert!(bad_state.memory_set);
        assert!(bad["errors"]["expression"].is_string());
        let explicit = call(
            Some("memory-store"),
            stored["state"].as_str().unwrap(),
            json!({"expression":"9"}),
        );
        let explicit: State = serde_json::from_str(explicit["state"].as_str().unwrap()).unwrap();
        assert_eq!(explicit.memory, 9.0);
        let large = call(Some("memory-store"), "", json!({"operand":"1e308"}));
        let overflow = call(
            Some("memory-add"),
            large["state"].as_str().unwrap(),
            json!({"operand":"1e308"}),
        );
        let overflow_state: State =
            serde_json::from_str(overflow["state"].as_str().unwrap()).unwrap();
        assert_eq!(
            overflow_state.memories,
            vec![decimal::normalize("2e308").unwrap()]
        );
        assert_eq!(overflow["errors"], json!({}));
    }
    #[test]
    fn operand_field_obeys_request_boundary_and_is_not_saved() {
        for fields in [json!({"operand":"1".repeat(4097)}), json!({"unknown":"0"})] {
            assert!(process(&json!({"version":2,"action":"evaluate","state":"","fields":fields,"locale":"en"}).to_string()).is_err());
        }
        let preview = call(Some("evaluate"), "", json!({"operand":"sqrt(81)"}));
        let saved = with_storage(
            "save",
            preview["state"].as_str().unwrap(),
            json!({}),
            json!([]),
        )
        .unwrap();
        assert!(
            saved["effects"][0]["changes"][STORAGE_KEY]["fields"]
                .get("operand")
                .is_none()
        );
        assert_eq!(saved["effects"][0]["changes"][STORAGE_KEY]["result"], "9");
    }
    #[test]
    fn gradians_and_clear_history_keep_the_formal_answer() {
        let view = call(
            Some("calculate"),
            "",
            json!({"expression":"asin(1)","angle":"grad"}),
        );
        let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.ans, 100.0);
        assert_eq!(state.history[0].angle, "grad");
        let cleared = call(
            Some("history-clear"),
            view["state"].as_str().unwrap(),
            json!({}),
        );
        let cleared: State = serde_json::from_str(cleared["state"].as_str().unwrap()).unwrap();
        assert_eq!(cleared.ans, 100.0);
        assert!(cleared.history.is_empty());
    }
    #[test]
    fn fresh_workspace_starts_with_empty_equations_and_valid_empty_axes() {
        let view = call(None, "", json!({}));
        assert_eq!(view["errors"], json!({}));
        let state: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["fields"]["functions"], "");
        assert_eq!(state["plot"]["series"], json!([]));
        assert_eq!(state["plot"]["xmin"], -10.0);
        assert_eq!(state["plot"]["xmax"], 10.0);
        assert_eq!(state["plot"]["ymin"], -3.0);
        assert_eq!(state["plot"]["ymax"], 3.0);
    }
    #[test]
    fn blank_plot_is_valid_but_numerical_analysis_still_requires_a_function() {
        let view = call(
            Some("plot"),
            "",
            json!({"functions":" \n\t ","xmin":"2","xmax":"6","ymin":"-4","ymax":"8"}),
        );
        assert_eq!(view["errors"], json!({}));
        let output: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(output["plot"]["series"], json!([]));
        assert_eq!(output["plot"]["xmin"], 2.0);
        assert_eq!(output["plot"]["xmax"], 6.0);
        assert_eq!(output["plot"]["ymin"], -4.0);
        assert_eq!(output["plot"]["ymax"], 8.0);
        let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert!(functions(&state).is_err());
        for action in ["root", "integrate"] {
            let analysis = call(Some(action), view["state"].as_str().unwrap(), json!({}));
            assert_eq!(
                analysis["errors"]["a"],
                "Enter between one and eight functions, one per line"
            );
        }
        let invalid_range = call(Some("plot"), "", json!({"xmin":"2","xmax":"1"}));
        assert!(!invalid_range["errors"].as_object().unwrap().is_empty());
    }
    #[test]
    fn plot_has_three_real_series() {
        let v = call(None, "", json!({"functions":"sin(x)\ncos(x)\nx^2 / 10"}));
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
    fn changed_functions_fit_y_once_before_discontinuity_filtering() {
        for (function, low, high) in [
            ("x^2", -10.0, 110.0),
            ("1000000*x", -12000000.0, 12000000.0),
        ] {
            let view = call(Some("plot"), "", json!({"functions":function}));
            assert_eq!(view["errors"], json!({}));
            let state: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
            assert_eq!(state["plot"]["xmin"], -10.0);
            assert_eq!(state["plot"]["xmax"], 10.0);
            assert_eq!(state["plot"]["ymin"], low);
            assert_eq!(state["plot"]["ymax"], high);
            assert_eq!(state["fields"]["ymin"], low.to_string());
            assert_eq!(state["fields"]["ymax"], high.to_string());
            assert!(
                state["plot"]["series"][0]["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|p| p[1].is_number())
            );
        }
    }
    #[test]
    fn graph_navigation_and_refresh_preserve_manual_axes_until_function_changes() {
        let initial = call(Some("plot"), "", json!({"functions":"x^2"}));
        let manual = call(
            Some("plot"),
            initial["state"].as_str().unwrap(),
            json!({"xmin":"-2", "xmax":"2", "ymin":"-1", "ymax":"1"}),
        );
        for action in [None, Some("plot"), Some("save")] {
            let view = call(action, manual["state"].as_str().unwrap(), json!({}));
            let state: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
            assert_eq!(state["plot"]["xmin"], -2.0);
            assert_eq!(state["plot"]["xmax"], 2.0);
            assert_eq!(state["plot"]["ymin"], -1.0);
            assert_eq!(state["plot"]["ymax"], 1.0);
        }
        let changed = call(
            Some("plot"),
            manual["state"].as_str().unwrap(),
            json!({"functions":"x^2+100"}),
        );
        let state: Value = serde_json::from_str(changed["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["plot"]["xmin"], -2.0);
        assert_eq!(state["plot"]["xmax"], 2.0);
        assert_eq!(state["plot"]["ymin"], 99.6);
        assert_eq!(state["plot"]["ymax"], 104.4);
    }
    #[test]
    fn explicit_axis_changes_take_priority_over_new_functions() {
        for fields in [
            json!({"functions":"x^2", "ymin":"-1", "ymax":"1"}),
            json!({"functions":"x^2", "xmin":"2", "xmax":"4"}),
        ] {
            let view = call(Some("plot"), "", fields.clone());
            let state: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
            assert_eq!(
                state["fields"]["ymin"],
                fields.get("ymin").cloned().unwrap_or(json!("-3"))
            );
            assert_eq!(
                state["fields"]["ymax"],
                fields.get("ymax").cloned().unwrap_or(json!("3"))
            );
        }
        let equivalent = call(
            Some("plot"),
            "",
            json!({"functions":"x^2", "ymin":"-3.0", "ymax":"3e0"}),
        );
        let state: Value = serde_json::from_str(equivalent["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["plot"]["ymax"], 110.0);
    }
    #[test]
    fn angle_changes_and_multiple_curves_fit_the_sampled_values() {
        let initial = call(None, "", json!({"functions":"sin(x)", "angle":"deg"}));
        let view = call(
            Some("plot"),
            initial["state"].as_str().unwrap(),
            json!({"angle":"rad"}),
        );
        let state: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert!(state["plot"]["ymin"].as_f64().unwrap() < -1.0);
        assert!(state["plot"]["ymax"].as_f64().unwrap() > 1.0);
        let view = call(Some("plot"), "", json!({"functions":"x^2\n-100\nsqrt(-1)"}));
        let state: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["plot"]["ymin"], -120.0);
        assert_eq!(state["plot"]["ymax"], 120.0);
    }
    #[test]
    fn fit_handles_constant_tiny_and_boundary_values_without_invalid_axes() {
        for function in ["0", "100", "1e-9*x", "1e12", "-1e12"] {
            let view = call(Some("plot"), "", json!({"functions":function}));
            assert_eq!(view["errors"], json!({}));
            let state: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
            let low = state["plot"]["ymin"].as_f64().unwrap();
            let high = state["plot"]["ymax"].as_f64().unwrap();
            assert!(low >= -1e12 && high <= 1e12 && high - low >= 1e-10);
            for point in state["plot"]["series"][0]["points"].as_array().unwrap() {
                let y = point[1].as_f64().unwrap();
                assert!(y >= low && y <= high);
            }
            if function == "1e-9*x" {
                assert!(high - low < 3e-8);
            }
        }
    }
    #[test]
    fn manual_fit_resamples_current_x_and_handles_empty_or_invalid_functions() {
        let initial = call(Some("plot"), "", json!({"functions":"x^2"}));
        let view = call(
            Some("fit-plot"),
            initial["state"].as_str().unwrap(),
            json!({"xmin":"100", "xmax":"110"}),
        );
        let state: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(state["plot"]["ymin"], 9790.0);
        assert_eq!(state["plot"]["ymax"], 12310.0);
        for function in ["", "sqrt(-1)", "1e13", "sin("] {
            let view = call(Some("fit-plot"), "", json!({"functions":function}));
            let state: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
            assert_eq!(state["fields"]["ymin"], "-3");
            assert_eq!(state["fields"]["ymax"], "3");
            if function == "sin(" {
                assert!(view["errors"]["functions"].is_string());
            } else {
                assert!(
                    state["notice"]
                        .as_str()
                        .unwrap()
                        .contains("No drawable samples")
                );
            }
        }
    }
    #[test]
    fn continuous_oscillating_curves_do_not_create_pole_gaps() {
        for function in ["sin(75*x)", "sin(100*x)", "sin(75*x)+1000"] {
            let view = call(
                Some("plot"),
                "",
                json!({"functions": function, "angle": "rad"}),
            );
            assert_eq!(view["errors"], json!({}));
            let state: Value = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
            assert!(
                state["plot"]["series"][0]["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|point| point[1].is_number()),
                "continuous curve {function} contains a gap"
            );
        }
    }
    #[test]
    fn singularities_create_gaps() {
        for function in ["1/x", "1/(x-0.01)", "1/(x-0.01)+100"] {
            let v = call(Some("plot"), "", json!({"functions":function}));
            let state: Value = serde_json::from_str(v["state"].as_str().unwrap()).unwrap();
            assert!(
                state["plot"]["series"][0]["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p[1].is_null())
            );
        }
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
        let calculated = call(
            Some("calculate"),
            "",
            json!({"expression":"10+20","functions":"sin(x)\ncos(x)\nx^2 / 10"}),
        );
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
        assert_eq!(state["fields"]["functions"], "sin(x)\ncos(x)\nx^2 / 10");
        assert_eq!(state["plot"]["series"].as_array().unwrap().len(), 3);
    }
    #[test]
    fn invalid_select_values_never_produce_an_invalid_view() {
        for fields in [json!({"angle":"other"}), json!({"curve":"8"})] {
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
        let mut view = call(
            Some("calculate"),
            "",
            json!({"expression":"6*7","functions":"sin(x)\ncos(x)\nx^2 / 10"}),
        );
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
        for (key, value) in [("result", json!("7")), ("memory_set", json!(true))] {
            let mut changed = workspace.clone();
            changed[key] = value;
            assert!(with_storage("host-result", saved["state"].as_str().unwrap(), json!({}),
                json!([{"id":"save-workspace","value":{"revision":8,"values":{STORAGE_KEY:changed}},"error":null}])).is_err());
        }
        assert!(process(&json!({"version":2,"action":"save","state":"","fields":{},"locale":"en","host":{"permissions":["storage"],"storage":{"revision":i32::MAX,"values":{}}}}).to_string()).is_err());
    }

    #[test]
    fn save_receipt_must_match_carry_programmer_and_date_results() {
        let view = call(
            Some("programmer-evaluate"),
            "",
            json!({"mode":"programmer","expression":"rcl(80)","radix":"16","bits":"8"}),
        );
        let saved = with_storage(
            "save",
            view["state"].as_str().unwrap(),
            json!({}),
            json!([]),
        )
        .unwrap();
        let original: State =
            serde_json::from_value(saved["effects"][0]["changes"][STORAGE_KEY].clone()).unwrap();
        assert!(original.carry);
        for changed in [
            State {
                carry: false,
                ..original.clone()
            },
            State {
                programmer: Some(programmer::evaluate("0", 16, 8).unwrap()),
                ..original.clone()
            },
            State {
                date: Some(date::difference("2024-02-01", "2024-03-01").unwrap()),
                ..original.clone()
            },
        ] {
            assert!(valid_state(&changed));
            let receipt = json!([{"id":"save-workspace","value":{"revision":8,"values":{STORAGE_KEY:changed}},"error":null}]);
            assert!(
                with_storage(
                    "host-result",
                    saved["state"].as_str().unwrap(),
                    json!({}),
                    receipt,
                )
                .is_err()
            );
        }
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
        state
            .fields
            .insert("functions".into(), "sin(x)\ncos(x)\nx^2 / 10".into());
        // Every history entry is individually valid. Escaped backslashes expand
        // on serialization; derived curve samples then exceed the state budget.
        for index in 0..MAX_HISTORY {
            state.history.push(History {
                expression: format!("{index}{}", "\\".repeat(3000)),
                value: index as f64,
                angle: "rad".into(),
                exact: index.to_string(),
                mode: "scientific".into(),
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
