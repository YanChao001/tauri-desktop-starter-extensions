//! 货币汇率只从受控 HTTP effect 读取；持久缓存保留来源的实际更新时间。
//!
//! ExchangeRate-API Open Access 允许本机缓存及个人 / 商业换算，要求在
//! 使用汇率的页面提供署名链接，不允许提供原始汇率的再分发接口。

use crate::decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const URL: &str = "https://open.er-api.com/v6/latest/USD";
const MAX_BODY_BYTES: usize = 65_536;
const MAX_RATES: usize = 256;
const INVALID_RESPONSE: &str = "Invalid currency rate response";
// 仅包含公开币种代码，不打包提供商的汇率数据。取得缓存后以响应内支持项为准。
const CODES: &str = "AED AFN ALL AMD ANG AOA ARS AUD AWG AZN BAM BBD BDT BGN BHD BIF BMD BND BOB BRL BSD BTN BWP BYN BZD CAD CDF CHF CLF CLP CNH CNY COP CRC CUP CVE CZK DJF DKK DOP DZD EGP ERN ETB EUR FJD FKP FOK GBP GEL GGP GHS GIP GMD GNF GTQ GYD HKD HNL HRK HTG HUF IDR ILS IMP INR IQD IRR ISK JEP JMD JOD JPY KES KGS KHR KID KMF KRW KWD KYD KZT LAK LBP LKR LRD LSL LYD MAD MDL MGA MKD MMK MNT MOP MRU MUR MVR MWK MXN MYR MZN NAD NGN NIO NOK NPR NZD OMR PAB PEN PGK PHP PKR PLN PYG QAR RON RSD RUB RWF SAR SBD SCR SDG SEK SGD SHP SLE SLL SOS SRD SSP STN SYP SZL THB TJS TMT TND TOP TRY TTD TVD TWD TZS UAH UGX USD UYU UZS VES VND VUV WST XAF XCD XCG XDR XOF XPF YER ZAR ZMW ZWG ZWL";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub base: String,
    pub updated_at: u64,
    pub next_update_at: u64,
    pub rates: BTreeMap<String, String>,
}

impl Snapshot {
    /// 保存和恢复都校验缓存的结构、日期及数值边界。
    pub fn valid(&self) -> bool {
        self.base == "USD"
            && valid_times(self.updated_at, self.next_update_at)
            && (2..=MAX_RATES).contains(&self.rates.len())
            && self.rates.get("USD").is_some_and(|rate| rate == "1")
            && self.rates.iter().all(|(code, rate)| {
                valid_code(code) && normalize_rate(rate).is_ok_and(|normalized| normalized == *rate)
            })
    }
}

/// 无首次缓存时仍允许选择币种；此列表不代表已有任何可使用的汇率。
/// 由 HTML 的 Intl.DisplayNames 按当前语言显示货币名称，协议保留稳定代码。
pub fn codes(snapshot: Option<&Snapshot>) -> Vec<String> {
    if let Some(snapshot) = snapshot {
        snapshot.rates.keys().cloned().collect()
    } else {
        CODES.split_whitespace().map(str::to_owned).collect()
    }
}

fn valid_times(updated_at: u64, next_update_at: u64) -> bool {
    // Unix 秒必须对应可显示的公历日期；开放端点每日更新。
    (946_684_800..=253_401_695_999).contains(&updated_at)
        && next_update_at > updated_at
        && next_update_at - updated_at <= 7 * 24 * 60 * 60
        && next_update_at <= 253_402_300_799
}

pub fn valid_code(code: &str) -> bool {
    code.len() == 3 && code.bytes().all(|byte| byte.is_ascii_uppercase())
}

fn normalize_rate(source: &str) -> Result<String, String> {
    if source.len() > 48
        || !source
            .parse::<f64>()
            .is_ok_and(|number| number.is_finite() && (1e-18..=1e18).contains(&number))
    {
        return Err(INVALID_RESPONSE.into());
    }
    decimal::normalize(source).map_err(|_| INVALID_RESPONSE.into())
}

/// 仅接受匹配固定 URL 的宿主 HTTP 回执。任意失败都由调用方保留旧缓存。
pub fn from_http(value: &Value) -> Result<Snapshot, String> {
    let response = value.as_object().ok_or(INVALID_RESPONSE)?;
    if response.len() != 4
        || response.get("url").and_then(Value::as_str) != Some(URL)
        || response
            .get("headers")
            .and_then(Value::as_object)
            .is_none_or(|headers| headers.values().any(|value| !value.is_string()))
    {
        return Err(INVALID_RESPONSE.into());
    }
    match response.get("status").and_then(Value::as_u64) {
        Some(200) => {}
        Some(429) => return Err("Currency source is rate limited".into()),
        _ => return Err("Currency source request failed".into()),
    }
    let body = response
        .get("body")
        .and_then(Value::as_str)
        .filter(|body| body.len() <= MAX_BODY_BYTES)
        .ok_or(INVALID_RESPONSE)?;
    let payload: Value = serde_json::from_str(body).map_err(|_| INVALID_RESPONSE)?;
    if payload.get("result").and_then(Value::as_str) != Some("success")
        || payload.get("base_code").and_then(Value::as_str) != Some("USD")
    {
        return Err(INVALID_RESPONSE.into());
    }
    let updated_at = payload
        .get("time_last_update_unix")
        .and_then(Value::as_u64)
        .ok_or(INVALID_RESPONSE)?;
    let next_update_at = payload
        .get("time_next_update_unix")
        .and_then(Value::as_u64)
        .ok_or(INVALID_RESPONSE)?;
    let values = payload
        .get("rates")
        .and_then(Value::as_object)
        .filter(|values| (2..=MAX_RATES).contains(&values.len()))
        .ok_or(INVALID_RESPONSE)?;
    let mut rates = BTreeMap::new();
    for (code, value) in values {
        if !valid_code(code) || !value.is_number() {
            return Err(INVALID_RESPONSE.into());
        }
        rates.insert(code.clone(), normalize_rate(&value.to_string())?);
    }
    let snapshot = Snapshot {
        base: "USD".into(),
        updated_at,
        next_update_at,
        rates,
    };
    if !snapshot.valid() {
        return Err(INVALID_RESPONSE.into());
    }
    Ok(snapshot)
}

/// 交叉汇率换算保持十进制输入，不经过二进制浮点运算。
pub fn convert(snapshot: &Snapshot, from: &str, to: &str, input: &str) -> Result<String, String> {
    if !snapshot.valid() {
        return Err("Currency rates are unavailable".into());
    }
    let from_rate = snapshot
        .rates
        .get(from)
        .ok_or("Choose a supported currency")?;
    let to_rate = snapshot
        .rates
        .get(to)
        .ok_or("Choose a supported currency")?;
    if input.len() > 256 {
        return Err("Currency amount exceeds 256 bytes".into());
    }
    // normalize 校验它是单个十进制字面量；原始数字进入整式，避免先显示舍入。
    decimal::normalize(input)?;
    decimal::evaluate(&format!("({input})*({to_rate})/({from_rate})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // 人工构造的测试数字，不包含服务提供商的真实汇率数据。
    fn body() -> Value {
        json!({
            "result":"success", "base_code":"USD",
            "time_last_update_unix":1_700_000_000_u64,
            "time_next_update_unix":1_700_086_400_u64,
            "rates":{"USD":1,"EUR":0.8,"CNY":7.2,"JPY":160}
        })
    }

    fn response(body: Value) -> Value {
        json!({"url":URL,"status":200,"headers":{"content-type":"application/json"},"body":body.to_string()})
    }

    fn snapshot() -> Snapshot {
        from_http(&response(body())).unwrap()
    }

    #[test]
    fn source_dates_and_decimal_rates_are_preserved() {
        let snapshot = snapshot();
        assert_eq!(snapshot.base, "USD");
        assert_eq!(snapshot.updated_at, 1_700_000_000);
        assert_eq!(snapshot.next_update_at, 1_700_086_400);
        assert_eq!(snapshot.rates["EUR"], "0.8");
        let restored: Snapshot =
            serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap();
        assert!(restored.valid());
        assert_eq!(restored, snapshot);
    }

    #[test]
    fn raw_json_exchange_rates_preserve_all_decimal_digits() {
        // 原始正文必须保留数字 token，不能先经过 json!(f64) 构造测试输入。
        let body = r#"{
            "result":"success","base_code":"USD",
            "time_last_update_unix":1700000000,
            "time_next_update_unix":1700086400,
            "rates":{"USD":1,"EUR":0.123456789012345678901}
        }"#;
        let receipt = json!({
            "url":URL,"status":200,"headers":{"content-type":"application/json"},
            "body":body
        });
        let snapshot = from_http(&receipt).unwrap();
        assert_eq!(snapshot.rates["EUR"], "0.123456789012345678901");
        assert_eq!(
            convert(&snapshot, "USD", "EUR", "1").unwrap(),
            "0.123456789012345678901"
        );
    }

    #[test]
    fn initial_codes_have_no_implied_rates_and_cached_codes_follow_the_source() {
        let codes = codes(None);
        assert_eq!(codes.len(), 166);
        assert!(codes.iter().all(|code| valid_code(code)));
        assert!(codes.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            super::codes(Some(&snapshot())),
            ["CNY", "EUR", "JPY", "USD"]
        );
    }

    #[test]
    fn cross_rates_and_reverse_conversions_use_exact_decimal_arithmetic() {
        let snapshot = snapshot();
        for (from, to, input, expected) in [
            ("USD", "CNY", "12.5", "90"),
            ("EUR", "CNY", "10", "90"),
            ("CNY", "EUR", "90", "10"),
            ("USD", "EUR", "0.1", "0.08"),
            ("CNY", "CNY", "1.23E3", "1230"),
            ("USD", "CNY", "-12.5", "-90"),
            ("JPY", "EUR", "0", "0"),
        ] {
            assert_eq!(convert(&snapshot, from, to, input).unwrap(), expected);
        }
    }

    #[test]
    fn invalid_amounts_and_unknown_currencies_fail_without_fabricating_results() {
        let snapshot = snapshot();
        for amount in ["", "1+2", "NaN", "Infinity", "1.2.3"] {
            assert!(convert(&snapshot, "USD", "EUR", amount).is_err());
        }
        assert!(convert(&snapshot, "usd", "EUR", "1").is_err());
        assert!(convert(&snapshot, "USD", "XXX", "1").is_err());
        assert!(convert(&snapshot, "USD", "EUR", &"1".repeat(257)).is_err());
    }

    #[test]
    fn finite_amounts_preserve_more_than_32_digits_without_intermediate_rounding() {
        let input = format!("1{}4", "0".repeat(31));
        let expected = format!("8{}3.2", "0".repeat(30));
        assert_eq!(
            convert(&snapshot(), "USD", "EUR", &input).unwrap(),
            expected
        );
    }

    #[test]
    fn http_identity_status_and_shape_are_checked() {
        let original = response(body());
        for (key, replacement) in [
            (
                "url",
                json!("https://open.er-api.com.evil.test/v6/latest/USD"),
            ),
            ("url", json!("https://open.er-api.com/v6/latest/EUR")),
            ("status", json!("200")),
            ("status", json!(500)),
            ("headers", json!({"content-type":1})),
            ("body", json!(null)),
        ] {
            let mut changed = original.clone();
            changed[key] = replacement;
            assert!(from_http(&changed).is_err(), "{key}");
        }
        let mut limited = original.clone();
        limited["status"] = json!(429);
        assert_eq!(
            from_http(&limited).unwrap_err(),
            "Currency source is rate limited"
        );
        let mut extra = original.clone();
        extra["untrusted"] = json!(true);
        assert!(from_http(&extra).is_err());
        let mut large = original;
        large["body"] = json!(" ".repeat(MAX_BODY_BYTES + 1));
        assert!(from_http(&large).is_err());
    }

    #[test]
    fn malformed_provider_data_and_dates_do_not_replace_cache() {
        let original = body();
        for (key, replacement) in [
            ("result", json!("error")),
            ("base_code", json!("EUR")),
            ("time_last_update_unix", json!(-1)),
            ("time_last_update_unix", json!(1_700_086_400_u64)),
            ("time_next_update_unix", json!(1_710_086_400_u64)),
            ("rates", json!({"USD":1})),
            ("rates", json!({"USD":2,"CNY":7.2})),
            ("rates", json!({"USD":1,"CNY":0})),
            ("rates", json!({"USD":1,"CNY":-1})),
            ("rates", json!({"USD":1,"CNY":"7.2"})),
            ("rates", json!({"USD":1,"CNY":null})),
            ("rates", json!({"USD":1,"cny":7.2})),
            ("rates", json!({"USD":1,"CNY":1e19})),
            ("rates", json!({"USD":1,"CNY":1e-19})),
        ] {
            let mut changed = original.clone();
            changed[key] = replacement;
            assert!(from_http(&response(changed)).is_err(), "{key}");
        }
    }

    #[test]
    fn corrupted_snapshot_cannot_be_used_for_conversion() {
        let mut snapshot = snapshot();
        snapshot.rates.insert("EUR".into(), "1+2".into());
        assert!(!snapshot.valid());
        assert_eq!(
            convert(&snapshot, "USD", "EUR", "1").unwrap_err(),
            "Currency rates are unavailable"
        );
    }
}
