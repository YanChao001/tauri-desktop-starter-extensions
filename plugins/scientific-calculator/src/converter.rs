//! Windows 单位目录与离线换算。系数保留上游十进制文本，不经过 f64。

use crate::decimal;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

pub const CATEGORIES: [&str; 13] = [
    "currency",
    "volume",
    "length",
    "weight",
    "temperature",
    "energy",
    "area",
    "speed",
    "time",
    "power",
    "data",
    "pressure",
    "angle",
];
const MAX_INPUT_BYTES: usize = 256;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnitOption {
    pub id: String,
    pub label: String,
    pub abbreviation: String,
    pub whimsical: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Equivalent {
    pub id: String,
    pub label: String,
    pub abbreviation: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Output {
    pub category: String,
    pub from: String,
    pub to: String,
    pub input: String,
    pub result: Option<String>,
    pub units: Vec<UnitOption>,
    pub equivalents: Vec<Equivalent>,
    pub supports_negative: bool,
    pub updated_at: Option<u64>,
    pub next_update_at: Option<u64>,
    pub rate: Option<String>,
    pub source: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Catalog {
    source: String,
    commit: String,
    license: String,
    categories: Vec<Category>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Category {
    id: String,
    label: Localized,
    supports_negative: bool,
    default_from: Regional,
    default_to: Regional,
    units: Vec<Unit>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Regional {
    #[serde(rename = "enUS")]
    en_us: u16,
    #[serde(rename = "zhSI")]
    zh_si: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Localized {
    en: String,
    zh: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Unit {
    id: u16,
    label: Localized,
    abbr: Localized,
    factor: Option<String>,
    whimsical: bool,
    available_when: String,
}

static CATALOG: LazyLock<Result<Catalog, serde_json::Error>> =
    LazyLock::new(|| serde_json::from_str(include_str!("converter-catalog.json")));

fn category(id: &str) -> Result<&'static Category, String> {
    let catalog = CATALOG.as_ref().map_err(|_| "Invalid unit catalogue")?;
    if catalog.source.is_empty() || catalog.commit.len() != 40 || !catalog.license.contains("MIT") {
        return Err("Invalid unit catalogue".into());
    }
    catalog
        .categories
        .iter()
        .find(|category| category.id == id)
        .ok_or_else(|| "Choose a supported converter category".into())
}

fn visible(unit: &Unit) -> bool {
    unit.available_when == "always"
}
fn text<'a>(value: &'a Localized, locale: &str) -> &'a str {
    if locale == "en" { &value.en } else { &value.zh }
}

pub fn defaults(id: &str, locale: &str) -> Result<(String, String), String> {
    let category = category(id)?;
    let from = if locale == "en" {
        category.default_from.en_us
    } else {
        category.default_from.zh_si
    };
    let to = if locale == "en" {
        category.default_to.en_us
    } else {
        category.default_to.zh_si
    };
    Ok((from.to_string(), to.to_string()))
}

pub fn units(id: &str, locale: &str) -> Result<Vec<UnitOption>, String> {
    Ok(category(id)?
        .units
        .iter()
        .filter(|unit| visible(unit))
        .map(|unit| UnitOption {
            id: unit.id.to_string(),
            label: text(&unit.label, locale).into(),
            abbreviation: text(&unit.abbr, locale).into(),
            whimsical: unit.whimsical,
        })
        .collect())
}

pub fn category_label(id: &str, locale: &str) -> Result<String, String> {
    Ok(text(&category(id)?.label, locale).into())
}

pub fn describe(
    id: &str,
    from: &str,
    to: &str,
    input: &str,
    locale: &str,
) -> Result<Output, String> {
    let category = category(id)?;
    Ok(Output {
        category: id.into(),
        from: from.into(),
        to: to.into(),
        input: input.into(),
        result: None,
        units: units(id, locale)?,
        equivalents: Vec::new(),
        supports_negative: category.supports_negative,
        updated_at: None,
        next_update_at: None,
        rate: None,
        source: None,
    })
}

fn find_unit<'a>(category: &'a Category, id: &str) -> Result<&'a Unit, String> {
    let number = id.parse::<u16>().map_err(|_| "Choose a supported unit")?;
    category
        .units
        .iter()
        .find(|unit| unit.id == number && visible(unit))
        .ok_or_else(|| "Choose a supported unit".into())
}

fn value(category: &Category, from: &Unit, to: &Unit, input: &str) -> Result<String, String> {
    if from.id == to.id {
        return Ok(input.into());
    }
    let expression = if category.id == "temperature" {
        // 华氏与摄氏的 5/9 保持为分数，避免把上游 double 展开当精确常数。
        let celsius = match from.id {
            46 => format!("({input})"),
            47 => format!("(({input})-32)*5/9"),
            48 => format!("({input})-273.15"),
            _ => return Err("Choose a supported unit".into()),
        };
        match to.id {
            46 => celsius,
            47 => format!("({celsius})*9/5+32"),
            48 => format!("({celsius})+273.15"),
            _ => return Err("Choose a supported unit".into()),
        }
    } else {
        let source = from
            .factor
            .as_deref()
            .ok_or("Missing unit conversion factor")?;
        let target = to
            .factor
            .as_deref()
            .ok_or("Missing unit conversion factor")?;
        format!("({input})*({source})/({target})")
    };
    decimal::evaluate(&expression)
}

struct Suggested<'a> {
    unit: &'a Unit,
    value: f64,
    magnitude: f64,
}

fn suggested_order(first: &Suggested<'_>, second: &Suggested<'_>) -> std::cmp::Ordering {
    first
        .magnitude
        .abs()
        .total_cmp(&second.magnitude.abs())
        .then_with(|| second.magnitude.total_cmp(&first.magnitude))
}

fn format_suggestion(value: f64) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    let precision = if value.abs() < 100.0 {
        2
    } else if value.abs() < 1000.0 {
        1
    } else {
        0
    };
    let mut rounded = format!("{value:.precision$}");
    if rounded.contains('.') {
        rounded.truncate(rounded.trim_end_matches('0').trim_end_matches('.').len());
    }
    Some(rounded)
}

fn equivalents(
    category: &Category,
    source: &Unit,
    target: &Unit,
    input: &str,
    locale: &str,
) -> Vec<Equivalent> {
    // Windows ranks the approximate suggestions by proximity to one, separately
    // for ordinary and whimsical units. The primary conversion remains decimal.
    let Ok(approximate_input) = input.parse::<f64>() else {
        return Vec::new();
    };
    if !approximate_input.is_finite() {
        return Vec::new();
    }
    let input = if approximate_input == 0.0 { "0" } else { input };
    let (mut whimsical, mut ordinary): (Vec<_>, Vec<_>) = category
        .units
        .iter()
        .filter(|unit| visible(unit) && unit.id != source.id && unit.id != target.id)
        .filter_map(|unit| {
            let converted = value(category, source, unit, input)
                .ok()?
                .parse::<f64>()
                .ok()?;
            converted.is_finite().then_some(Suggested {
                unit,
                value: converted,
                // Zero sorts last; abs also gives negative temperatures a stable
                // ordering instead of the upstream log10(negative) NaN.
                magnitude: converted.abs().log10(),
            })
        })
        .partition(|suggested| suggested.unit.whimsical);
    ordinary.sort_by(suggested_order);
    whimsical.sort_by(suggested_order);
    let make_equivalent = |entry: Suggested<'_>, keep_zero: bool| {
        let value = format_suggestion(entry.value)?;
        if !keep_zero && value.parse::<f64>().ok()? == 0.0 {
            return None;
        }
        Some(Equivalent {
            id: entry.unit.id.to_string(),
            label: text(&entry.unit.label, locale).into(),
            abbreviation: text(&entry.unit.abbr, locale).into(),
            value,
        })
    };
    ordinary
        .into_iter()
        .filter_map(|entry| make_equivalent(entry, category.supports_negative))
        .chain(
            whimsical
                .into_iter()
                .filter_map(|entry| make_equivalent(entry, false))
                .take(1),
        )
        .collect()
}

pub fn convert(
    id: &str,
    from: &str,
    to: &str,
    input: &str,
    locale: &str,
) -> Result<Output, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err("Converter amount exceeds 256 bytes".into());
    }
    let category = category(id)?;
    let original_input = input.to_owned();
    let input = decimal::normalize(input)?;
    if input.starts_with('-') && !category.supports_negative {
        return Err("Negative values are not supported in this category".into());
    }
    let source = find_unit(category, from)?;
    let target = find_unit(category, to)?;
    if source.whimsical || target.whimsical {
        return Err("Choose a supported unit".into());
    }
    let result = value(category, source, target, &input)?;
    let equivalents = equivalents(category, source, target, &input, locale);
    Ok(Output {
        category: id.into(),
        from: from.into(),
        to: to.into(),
        input: original_input,
        result: Some(result),
        units: units(id, locale)?,
        equivalents,
        supports_negative: category.supports_negative,
        updated_at: None,
        next_update_at: None,
        rate: None,
        source: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogue_is_complete_localized_and_uses_real_default_units() {
        let catalogue = CATALOG.as_ref().unwrap();
        assert!(
            catalogue
                .source
                .starts_with("https://github.com/microsoft/calculator")
        );
        assert_eq!(catalogue.commit.len(), 40);
        assert!(catalogue.license.contains("MIT"));
        assert_eq!(catalogue.categories.len(), 12);
        for id in CATEGORIES.into_iter().filter(|id| *id != "currency") {
            for locale in ["zh-CN", "en"] {
                let (from, to) = defaults(id, locale).unwrap();
                let units = units(id, locale).unwrap();
                assert!(units.iter().any(|unit| unit.id == from && !unit.whimsical));
                assert!(units.iter().any(|unit| unit.id == to && !unit.whimsical));
                assert!(units.iter().all(|unit| !unit.label.trim().is_empty()));
                assert!(!category_label(id, locale).unwrap().is_empty());
            }
        }
    }
    #[test]
    fn exact_metric_imperial_and_affine_conversions() {
        for (category, from, to, input, expected) in [
            ("length", "34", "33", "1234.5", "1.2345"),
            ("length", "32", "30", "1", "2.54"),
            ("volume", "74", "75", "1.25", "1250"),
            ("weight", "92", "86", "1", "453.59237"),
            ("temperature", "46", "47", "-40", "-40"),
            ("temperature", "47", "46", "212", "100"),
            ("temperature", "48", "46", "273.15", "0"),
            ("temperature", "46", "48", "-273.15", "0"),
            ("time", "50", "54", "1.5", "5400"),
            ("power", "44", "45", "-2.5", "-2500"),
            ("area", "7", "3", "1", "10000"),
            ("energy", "166", "27", "1", "3600000"),
            ("data", "16", "12", "1", "1000"),
            ("data", "150", "12", "1", "1024"),
            ("data", "11", "12", "8", "1"),
            ("angle", "136", "134", "100", "90"),
        ] {
            assert_eq!(
                convert(category, from, to, input, "en")
                    .unwrap()
                    .result
                    .as_deref(),
                Some(expected),
                "{category} {from}->{to}"
            );
        }
    }

    #[test]
    fn suggestions_rank_near_one_and_keep_all_ordinary_candidates() {
        let output = convert("length", "34", "30", "1.", "en").unwrap();
        assert_eq!(output.input, "1.");
        assert_eq!(output.result.as_deref(), Some("100"));
        assert_eq!(
            output
                .equivalents
                .iter()
                .take(3)
                .map(|entry| (entry.id.as_str(), entry.value.as_str()))
                .collect::<Vec<_>>(),
            [("40", "1.09"), ("31", "3.28"), ("32", "39.37")]
        );
        assert!(output.equivalents.len() > 4);
        assert!(
            output
                .equivalents
                .iter()
                .any(|entry| entry.id == "37" && entry.value == "1000")
        );
        assert!(
            output
                .equivalents
                .iter()
                .all(|entry| entry.id != "34" && entry.id != "30" && entry.value != "0")
        );
        let whimsical = output
            .equivalents
            .iter()
            .filter(|entry| {
                output
                    .units
                    .iter()
                    .any(|unit| unit.id == entry.id && unit.whimsical)
            })
            .collect::<Vec<_>>();
        assert_eq!(whimsical.len(), 1);
        assert_eq!(whimsical[0].id, "131");
        assert_eq!(whimsical[0].value, "5.36");
        assert_eq!(output.equivalents.last(), whimsical.first().copied());
    }

    #[test]
    fn suggestions_use_windows_decimal_places_and_trim_trailing_zeros() {
        for (value, expected) in [
            (1.0 / 0.035052, "28.53"),
            (-28.529, "-28.53"),
            (1.0, "1"),
            (1.2, "1.2"),
            (99.999, "100"),
            (100.06, "100.1"),
            (999.96, "1000"),
            (1000.6, "1001"),
            (10000.0, "10000"),
        ] {
            assert_eq!(format_suggestion(value).as_deref(), Some(expected));
        }
        assert!(format_suggestion(f64::INFINITY).is_none());
        assert!(format_suggestion(f64::NAN).is_none());
    }

    #[test]
    fn suggestion_ties_prefer_above_one_and_zero_follows_finite_magnitudes() {
        let unit = find_unit(category("length").unwrap(), "34").unwrap();
        let above = Suggested {
            unit,
            value: 10.0,
            magnitude: 1.0,
        };
        let below = Suggested {
            unit,
            value: 0.1,
            magnitude: -1.0,
        };
        let zero = Suggested {
            unit,
            value: 0.0,
            magnitude: f64::NEG_INFINITY,
        };
        assert!(suggested_order(&above, &below).is_lt());
        assert!(suggested_order(&below, &zero).is_lt());
    }

    #[test]
    fn suggestions_filter_rounded_zero_except_in_signed_categories() {
        let unsigned = convert("length", "34", "30", "0", "en").unwrap();
        assert!(unsigned.equivalents.is_empty());
        let signed = convert("power", "44", "45", "0", "en").unwrap();
        assert!(!signed.equivalents.is_empty());
        assert!(signed.equivalents.iter().all(|entry| entry.value == "0"));
        assert!(signed.equivalents.iter().all(|entry| {
            !signed
                .units
                .iter()
                .any(|unit| unit.id == entry.id && unit.whimsical)
        }));
    }

    #[test]
    fn overflowing_suggestions_leave_the_exact_primary_result_intact() {
        let output = convert("length", "34", "34", "1e999", "en").unwrap();
        assert_eq!(output.result.unwrap(), format!("1{}", "0".repeat(999)));
        assert!(output.equivalents.is_empty());
    }

    #[test]
    fn invalid_categories_units_amounts_and_negative_domains_are_rejected() {
        for (category, from, to, input) in [
            ("unknown", "34", "33", "1"),
            ("length", "34", "74", "1"),
            ("length", "34", "33", "-1"),
            ("length", "34", "33", "1+2"),
            ("length", "34", "33", "NaN"),
        ] {
            assert!(convert(category, from, to, input, "en").is_err());
        }
        assert!(convert("length", "34", "33", &"1".repeat(257), "en").is_err());
        assert_eq!(
            convert("temperature", "46", "47", "0", "en")
                .unwrap()
                .result
                .as_deref(),
            Some("32")
        );
    }
}
