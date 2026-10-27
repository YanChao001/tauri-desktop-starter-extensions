//! 有界的十进制与有理数计算。计算过程中不经过二进制浮点数。

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

const MAX_SOURCE_BYTES: usize = 4_096;
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 2_048;
const MAX_EXPONENT: i32 = 9_999;
const MAX_COMPONENT_BITS: u64 = 66_560;
const MAX_WORK: u64 = 100_000;
const DISPLAY_DIGITS: i32 = 32;

type Number = BigRational;

#[derive(Debug)]
enum EvaluationError {
    Unsupported(String),
    Calculation(String),
}
impl From<String> for EvaluationError {
    fn from(message: String) -> Self {
        Self::Calculation(message)
    }
}
impl From<&str> for EvaluationError {
    fn from(message: &str) -> Self {
        Self::Calculation(message.into())
    }
}
impl From<EvaluationError> for String {
    fn from(error: EvaluationError) -> Self {
        match error {
            EvaluationError::Unsupported(message) | EvaluationError::Calculation(message) => {
                message
            }
        }
    }
}

/// 只有十进制引擎未提供的语法或实数幂才交给科学近似引擎。
/// 计算预算、定义域和范围错误必须保留，不能被低精度重算掩盖。
pub fn scientific(source: &str) -> Result<Option<String>, String> {
    let mut work = Work::default();
    match parse(source, false, &mut work) {
        Ok(value) => format_number(&value.number, work.approximate).map(Some),
        Err(EvaluationError::Unsupported(_)) => Ok(None),
        Err(EvaluationError::Calculation(message)) => Err(message),
    }
}

/// 普通运算优先级；百分号是后缀百分数，加减时采用左操作数为基数。
pub fn evaluate(source: &str) -> Result<String, String> {
    evaluate_mode(source, false)
}

/// 标准模式的四则运算按输入顺序执行；括号内采用同样规则。
pub fn standard(source: &str) -> Result<String, String> {
    evaluate_mode(source, true)
}

/// 只接受十进制或科学计数字面量，供持久状态的数值边界校验使用。
pub fn normalize(source: &str) -> Result<String, String> {
    check_source(source)?;
    let source = source.trim();
    let (negative, digits) = match source.as_bytes().first() {
        Some(b'-') => (true, &source[1..]),
        Some(b'+') => (false, &source[1..]),
        _ => (false, source),
    };
    let mut value = parse_literal(digits)?;
    if negative {
        value = -value;
    }
    format_number(&value, false)
}

/// 右操作数末尾的百分号保留上下文：200 + 10% = 220，200 * 10% = 20。
/// `percent` 返回加减运算使用的百分数操作数，例如 (200, percent, 10) = 20。
/// 两侧源表达式均遵循标准模式的即时顺序，记忆加减的调用方传入规范数值。
pub fn binary(left: &str, operator: &str, right: &str) -> Result<String, String> {
    let mut work = Work::default();
    let left = parse(left, true, &mut work)?.number;
    let right = parse(right, true, &mut work)?;
    let value = if operator == "percent" {
        let product = work.multiply(&left, &right.number)?;
        work.divide(&product, &Number::from_integer(100.into()))?
    } else {
        apply_binary(&left, operator, right, &mut work)?
    };
    format_number(&value, work.approximate)
}

/// 标准模式单目运算保留操作数源表达式的即时顺序及精确有理数。
pub fn unary(source: &str, operation: &str) -> Result<String, String> {
    let mut work = Work::default();
    let value = parse(source, true, &mut work)?.number;
    let value = apply_unary(value, operation, &mut work)?;
    format_number(&value, work.approximate)
}

fn evaluate_mode(source: &str, standard: bool) -> Result<String, String> {
    let mut work = Work::default();
    let value = parse(source, standard, &mut work)?.number;
    format_number(&value, work.approximate)
}

fn check_source(source: &str) -> Result<(), String> {
    if source.len() > MAX_SOURCE_BYTES {
        Err("Expression exceeds 4096 bytes".into())
    } else if source.trim().is_empty() {
        Err("Expression is empty".into())
    } else {
        Ok(())
    }
}

#[derive(Default)]
struct Work {
    nodes: usize,
    cost: u64,
    approximate: bool,
}

impl Work {
    fn node(&mut self, depth: usize) -> Result<(), String> {
        self.nodes += 1;
        if depth > MAX_DEPTH {
            Err("Expression nesting exceeds 64 levels".into())
        } else if self.nodes > MAX_NODES {
            Err("Expression exceeds calculation budget".into())
        } else {
            Ok(())
        }
    }

    fn charge(&mut self, left: &Number, right: &Number) -> Result<(), String> {
        let left_words = (left.numer().bits() + left.denom().bits()).div_ceil(64);
        let right_words = (right.numer().bits() + right.denom().bits()).div_ceil(64);
        self.cost = self.cost.saturating_add(left_words * right_words + 1);
        if self.cost > MAX_WORK {
            Err("Expression exceeds calculation budget".into())
        } else {
            Ok(())
        }
    }

    fn add(&mut self, left: &Number, right: &Number) -> Result<Number, String> {
        self.charge(left, right)?;
        checked(left + right)
    }

    fn subtract(&mut self, left: &Number, right: &Number) -> Result<Number, String> {
        self.charge(left, right)?;
        checked(left - right)
    }

    fn multiply(&mut self, left: &Number, right: &Number) -> Result<Number, String> {
        self.charge(left, right)?;
        checked(left * right)
    }

    fn divide(&mut self, left: &Number, right: &Number) -> Result<Number, String> {
        if right.is_zero() {
            return Err("Division by zero".into());
        }
        if left == right {
            return Ok(Number::one());
        }
        self.charge(left, right)?;
        checked(left / right)
    }
}

fn checked(value: Number) -> Result<Number, String> {
    if value.numer().bits() > MAX_COMPONENT_BITS || value.denom().bits() > MAX_COMPONENT_BITS {
        return Err("Expression exceeds calculation budget".into());
    }
    if !value.is_zero() {
        let exponent = decimal_exponent(&value);
        if !(-MAX_EXPONENT..=MAX_EXPONENT).contains(&exponent) {
            return Err("Decimal result is outside the supported exponent range".into());
        }
    }
    Ok(value)
}

fn power_of_ten(exponent: u32) -> BigInt {
    BigInt::from(10_u8).pow(exponent)
}

fn parse_literal(source: &str) -> Result<Number, String> {
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut coefficient = String::with_capacity(source.len());
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        coefficient.push(bytes[index] as char);
        index += 1;
    }
    let mut fractional_digits = 0_i32;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            coefficient.push(bytes[index] as char);
            fractional_digits += 1;
            index += 1;
        }
    }
    if coefficient.is_empty() {
        return Err("Invalid decimal number".into());
    }
    let mut exponent = 0_i32;
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        let negative = bytes.get(index) == Some(&b'-');
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            exponent = exponent
                .checked_mul(10)
                .and_then(|value| value.checked_add(i32::from(bytes[index] - b'0')))
                .filter(|value| *value <= MAX_EXPONENT)
                .ok_or_else(|| "Decimal exponent must be between -9999 and 9999".to_string())?;
            index += 1;
        }
        if start == index {
            return Err("Invalid decimal number".into());
        }
        if negative {
            exponent = -exponent;
        }
    }
    if index != bytes.len() {
        return Err("Invalid decimal number".into());
    }
    let coefficient = BigInt::parse_bytes(coefficient.as_bytes(), 10)
        .ok_or_else(|| "Invalid decimal number".to_string())?;
    if coefficient.is_zero() {
        return Ok(Number::zero());
    }
    let scale = exponent - fractional_digits;
    let value = if scale >= 0 {
        Number::from_integer(coefficient * power_of_ten(scale as u32))
    } else {
        Number::new(coefficient, power_of_ten(scale.unsigned_abs()))
    };
    checked(value)
}

#[derive(Clone)]
struct Value {
    number: Number,
    percentage: bool,
}

impl Value {
    fn plain(number: Number) -> Self {
        Self {
            number,
            percentage: false,
        }
    }
}

fn parse(source: &str, standard: bool, work: &mut Work) -> Result<Value, EvaluationError> {
    check_source(source)?;
    let mut parser = Parser {
        source: source.as_bytes(),
        index: 0,
        standard,
        work,
    };
    let result = parser.expression(0, 0)?;
    parser.whitespace();
    if parser.index != parser.source.len() {
        return Err(EvaluationError::Unsupported(format!(
            "Unexpected token at byte {}",
            parser.index
        )));
    }
    Ok(result)
}

struct Parser<'source, 'work> {
    source: &'source [u8],
    index: usize,
    standard: bool,
    work: &'work mut Work,
}

impl Parser<'_, '_> {
    fn whitespace(&mut self) {
        while self
            .source
            .get(self.index)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.index += 1;
        }
    }

    fn expression(&mut self, min_binding: u8, depth: usize) -> Result<Value, EvaluationError> {
        self.work.node(depth)?;
        self.whitespace();
        let token = self.source.get(self.index).copied();
        let mut left = match token {
            Some(b'+' | b'-') => {
                self.index += 1;
                let mut value = self.expression(5, depth + 1)?;
                if token == Some(b'-') {
                    value.number = -value.number;
                }
                value
            }
            Some(b'(') => {
                self.index += 1;
                let value = self.expression(0, depth + 1)?;
                self.whitespace();
                if self.source.get(self.index) != Some(&b')') {
                    return Err("Missing closing parenthesis".into());
                }
                self.index += 1;
                value
            }
            Some(b'0'..=b'9' | b'.') => self.number()?,
            Some(b'a'..=b'z' | b'A'..=b'Z') => self.function(depth + 1)?,
            _ => {
                return Err(EvaluationError::Unsupported(format!(
                    "Expected a decimal number at byte {}",
                    self.index
                )));
            }
        };
        loop {
            self.whitespace();
            let token = self.source.get(self.index).copied();
            if token == Some(b'%') {
                if 7 < min_binding {
                    break;
                }
                self.work.node(depth)?;
                self.index += 1;
                left.number = self
                    .work
                    .divide(&left.number, &Number::from_integer(100.into()))?;
                left.percentage = true;
                continue;
            }
            let (operator, left_binding, right_binding) = match token {
                Some(b'+') => ("+", 1, 2),
                Some(b'-') => ("-", 1, 2),
                Some(b'*') if self.standard => ("*", 1, 2),
                Some(b'/') if self.standard => ("/", 1, 2),
                Some(b'*') => ("*", 3, 4),
                Some(b'/') => ("/", 3, 4),
                Some(b'^') => ("^", 6, 5),
                _ => break,
            };
            if left_binding < min_binding {
                break;
            }
            self.index += 1;
            let right = self.expression(right_binding, depth + 1)?;
            left = Value::plain(apply_binary(&left.number, operator, right, self.work)?);
        }
        Ok(left)
    }

    fn number(&mut self) -> Result<Value, EvaluationError> {
        let start = self.index;
        while self.source.get(self.index).is_some_and(u8::is_ascii_digit) {
            self.index += 1;
        }
        if self.source.get(self.index) == Some(&b'.') {
            self.index += 1;
            while self.source.get(self.index).is_some_and(u8::is_ascii_digit) {
                self.index += 1;
            }
        }
        if matches!(self.source.get(self.index), Some(b'e' | b'E')) {
            self.index += 1;
            if matches!(self.source.get(self.index), Some(b'+' | b'-')) {
                self.index += 1;
            }
            while self.source.get(self.index).is_some_and(u8::is_ascii_digit) {
                self.index += 1;
            }
        }
        // 扫描范围仅包含 ASCII 数值字符，因此必然是合法的 UTF-8。
        let literal = std::str::from_utf8(&self.source[start..self.index])
            .map_err(|_| "Invalid decimal number".to_string())?;
        Ok(Value::plain(parse_literal(literal)?))
    }

    fn function(&mut self, depth: usize) -> Result<Value, EvaluationError> {
        let start = self.index;
        while self
            .source
            .get(self.index)
            .is_some_and(u8::is_ascii_alphanumeric)
        {
            self.index += 1;
        }
        let name = std::str::from_utf8(&self.source[start..self.index])
            .map_err(|_| "Unknown decimal function".to_string())?
            .to_ascii_lowercase();
        self.whitespace();
        if self.source.get(self.index) != Some(&b'(') {
            return Err(EvaluationError::Unsupported(
                "Unknown decimal function or variable".into(),
            ));
        }
        self.index += 1;
        let value = self.expression(0, depth)?.number;
        self.whitespace();
        if self.source.get(self.index) != Some(&b')') {
            return Err(EvaluationError::Unsupported(
                "Missing closing parenthesis".into(),
            ));
        }
        self.index += 1;
        Ok(Value::plain(apply_unary(value, &name, self.work)?))
    }
}

fn apply_binary(
    left: &Number,
    operator: &str,
    right: Value,
    work: &mut Work,
) -> Result<Number, EvaluationError> {
    let right = if right.percentage && matches!(operator, "+" | "-" | "add" | "subtract") {
        work.multiply(left, &right.number)?
    } else {
        right.number
    };
    Ok(match operator {
        "+" | "add" => work.add(left, &right)?,
        "-" | "subtract" => work.subtract(left, &right)?,
        "*" | "multiply" => work.multiply(left, &right)?,
        "/" | "divide" => work.divide(left, &right)?,
        "^" | "power" => power(left, &right, work)?,
        _ => return Err("Unknown decimal operator".into()),
    })
}

fn apply_unary(value: Number, operation: &str, work: &mut Work) -> Result<Number, EvaluationError> {
    Ok(match operation {
        "negate" | "neg" => -value,
        "abs" => value.abs(),
        "square" | "sqr" => work.multiply(&value, &value)?,
        "sqrt" => square_root(value, work)?,
        "reciprocal" | "inverse" | "recip" => work.divide(&Number::one(), &value)?,
        "percent" => work.divide(&value, &Number::from_integer(100.into()))?,
        _ => {
            return Err(EvaluationError::Unsupported(
                "Unknown decimal function".into(),
            ));
        }
    })
}

fn power(base: &Number, exponent: &Number, work: &mut Work) -> Result<Number, EvaluationError> {
    if !exponent.is_integer() {
        return Err(EvaluationError::Unsupported(
            "Decimal power requires an integer exponent".into(),
        ));
    }
    let exponent = exponent
        .to_integer()
        .to_i32()
        .filter(|value| (-MAX_EXPONENT..=MAX_EXPONENT).contains(value))
        .ok_or_else(|| "Decimal exponent must be between -9999 and 9999".to_string())?;
    if base.is_zero() && exponent == 0 {
        // Windows scientific mode defines 0^0 as 1; keep that special case in
        // the scientific engine while the standard decimal API rejects it.
        return Err(EvaluationError::Unsupported(
            "Zero to the power of zero is undefined".into(),
        ));
    }
    let mut factor = if exponent < 0 {
        work.divide(&Number::one(), base)?
    } else {
        base.clone()
    };
    let mut remaining = exponent.unsigned_abs();
    let mut result = Number::one();
    while remaining > 0 {
        if remaining & 1 != 0 {
            result = work.multiply(&result, &factor)?;
        }
        remaining >>= 1;
        if remaining > 0 {
            factor = work.multiply(&factor, &factor)?;
        }
    }
    Ok(result)
}

fn integer_sqrt(value: &BigInt) -> BigInt {
    if value.is_zero() {
        return BigInt::zero();
    }
    let mut estimate = BigInt::one() << value.bits().div_ceil(2) as usize;
    loop {
        let next = (&estimate + value / &estimate) >> 1_usize;
        if next >= estimate {
            return estimate;
        }
        estimate = next;
    }
}

fn square_root(value: Number, work: &mut Work) -> Result<Number, String> {
    if value.is_negative() {
        return Err("Square root requires a non-negative value".into());
    }
    if value.is_zero() {
        return Ok(value);
    }
    work.charge(&value, &value)?;
    let numerator_root = integer_sqrt(value.numer());
    let denominator_root = integer_sqrt(value.denom());
    if &numerator_root * &numerator_root == *value.numer()
        && &denominator_root * &denominator_root == *value.denom()
    {
        return checked(Number::new(numerator_root, denominator_root));
    }
    // 后续四则可能产生可终止的小数，仍只展示近似运算所保证的有效位。
    work.approximate = true;
    let exponent = decimal_exponent(&value).div_euclid(2);
    let shift = DISPLAY_DIGITS - 1 - exponent;
    let mut numerator = value.numer().clone();
    let mut denominator = value.denom().clone();
    if shift >= 0 {
        numerator *= power_of_ten((2 * shift) as u32);
    } else {
        denominator *= power_of_ten((-2 * shift) as u32);
    }
    let mut root = integer_sqrt(&(&numerator / &denominator));
    let midpoint = (&root << 1_usize) + 1;
    if (&numerator << 2_usize) >= denominator * &midpoint * midpoint {
        root += 1;
    }
    if shift >= 0 {
        checked(Number::new(root, power_of_ten(shift as u32)))
    } else {
        checked(Number::from_integer(
            root * power_of_ten(shift.unsigned_abs()),
        ))
    }
}

fn decimal_exponent(value: &Number) -> i32 {
    let numerator = value.numer().abs();
    let denominator = value.denom();
    let mut exponent =
        numerator.to_str_radix(10).len() as i32 - denominator.to_str_radix(10).len() as i32;
    let below = if exponent >= 0 {
        numerator < denominator * power_of_ten(exponent as u32)
    } else {
        numerator * power_of_ten(exponent.unsigned_abs()) < *denominator
    };
    if below {
        exponent -= 1;
    }
    exponent
}

fn format_number(value: &Number, approximate: bool) -> Result<String, String> {
    if value.is_zero() {
        return Ok("0".into());
    }
    if !approximate && let Some((coefficient, scale)) = finite_decimal(value) {
        let formatted = format_digits(&coefficient, -scale, value.is_negative());
        if formatted.len() <= MAX_SOURCE_BYTES {
            return Ok(formatted);
        }
    }
    let mut exponent = decimal_exponent(value);
    let shift = DISPLAY_DIGITS - 1 - exponent;
    let mut numerator = value.numer().abs();
    let mut denominator = value.denom().clone();
    if shift >= 0 {
        numerator *= power_of_ten(shift as u32);
    } else {
        denominator *= power_of_ten(shift.unsigned_abs());
    }
    let mut coefficient = &numerator / &denominator;
    let remainder = numerator % &denominator;
    if (remainder << 1_usize) >= denominator {
        coefficient += 1;
    }
    let mut digits = coefficient.to_str_radix(10);
    if digits.len() > DISPLAY_DIGITS as usize {
        digits.pop();
        exponent += 1;
    }
    if !(-MAX_EXPONENT..=MAX_EXPONENT).contains(&exponent) {
        return Err("Decimal result is outside the supported exponent range".into());
    }
    Ok(format_digits(
        &digits,
        exponent - (digits.len() as i32 - 1),
        value.is_negative(),
    ))
}

fn finite_decimal(value: &Number) -> Option<(String, i32)> {
    let mut denominator = value.denom().clone();
    if denominator.is_one() {
        return Some((value.numer().abs().to_str_radix(10), 0));
    }
    let twos = denominator.trailing_zeros().unwrap_or(0) as i32;
    denominator >>= twos as usize;
    let five = BigInt::from(5_u8);
    let mut fives = 0_i32;
    if (&denominator % &five).is_zero() {
        // 二进制拆分指数，避免接近范围边界时逐次执行数万次除以 5。
        let mut factor = five.clone();
        let mut exponent = 1;
        let mut factors = Vec::new();
        while factor.bits() <= denominator.bits() {
            let next = &factor * &factor;
            factors.push((factor, exponent));
            factor = next;
            exponent *= 2;
        }
        for (factor, exponent) in factors.into_iter().rev() {
            if (&denominator % &factor).is_zero() {
                denominator /= factor;
                fives += exponent;
            }
        }
    }
    if !denominator.is_one() {
        return None;
    }
    let scale = twos.max(fives);
    // 既约分母含 2 或 5 时，所需补齐的系数没有尾零；超出额度直接舍入。
    if decimal_exponent(value) + scale + 1 > MAX_SOURCE_BYTES as i32 {
        return None;
    }
    let mut coefficient = value.numer().abs();
    coefficient <<= (scale - twos) as usize;
    coefficient *= five.pow((scale - fives) as u32);
    Some((coefficient.to_str_radix(10), scale))
}

/// coefficient * 10^scale；保留可放入输入额度的全部有限小数位。
fn format_digits(coefficient: &str, mut scale: i32, negative: bool) -> String {
    let digits = coefficient.trim_end_matches('0');
    scale += (coefficient.len() - digits.len()) as i32;
    let exponent = digits.len() as i32 - 1 + scale;
    let sign = if negative { "-" } else { "" };
    let point = digits.len() as i32 + scale;
    let plain_length = if point <= 0 {
        2 + (-point) as usize + digits.len()
    } else {
        (point as usize).max(digits.len()) + usize::from(point < digits.len() as i32)
    } + sign.len();
    if exponent >= -6 && plain_length <= MAX_SOURCE_BYTES {
        if point <= 0 {
            return format!("{sign}0.{}{digits}", "0".repeat((-point) as usize));
        }
        if point >= digits.len() as i32 {
            return format!(
                "{sign}{digits}{}",
                "0".repeat(point as usize - digits.len())
            );
        }
        let point = point as usize;
        return format!("{sign}{}.{}", &digits[..point], &digits[point..]);
    }
    let scientific = if digits.len() == 1 {
        format!("{sign}{digits}e{exponent}")
    } else {
        format!("{sign}{}.{}e{exponent}", &digits[..1], &digits[1..])
    };
    if scientific.len() > MAX_SOURCE_BYTES {
        // 整数系数无需小数点；边界长度的输入可以保持全部有效位。
        let compact = format!("{sign}{digits}e{scale}");
        if compact.len() < scientific.len() {
            return compact;
        }
    }
    scientific
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_decimal_arithmetic_and_long_integers() {
        assert_eq!(evaluate("0.1+0.2").unwrap(), "0.3");
        assert_eq!(evaluate("1/3*3").unwrap(), "1");
        assert_eq!(evaluate("-0+0/3").unwrap(), "0");
        assert_eq!(
            evaluate("1234567890123456789012345678901234567890+1").unwrap(),
            "1234567890123456789012345678901234567891"
        );
        assert_eq!(evaluate("1e9999/1e9999").unwrap(), "1");
        assert_eq!(evaluate("1e-9999/1e-9999").unwrap(), "1");
    }

    #[test]
    fn scientific_routing_only_falls_back_for_unsupported_operations() {
        assert_eq!(scientific("0.1+0.2").unwrap().as_deref(), Some("0.3"));
        for source in ["sin(30)", "atan2(1,1)", "2^0.5", "0^0", "pi", "2×3"] {
            assert!(scientific(source).unwrap().is_none(), "{source}");
        }
        for source in ["1/0", "sqrt(-1)", "1e-9999+1e-9999", "(1e-200)^5000"] {
            assert!(scientific(source).is_err(), "{source}");
        }
    }

    #[test]
    fn standard_uses_immediate_order_and_contextual_percent() {
        assert_eq!(standard("2+3*4").unwrap(), "20");
        assert_eq!(evaluate("2+3*4").unwrap(), "14");
        assert_eq!(standard("2+(3+4*5)").unwrap(), "37");
        for (operator, expected) in [("+", "220"), ("-", "180"), ("*", "20"), ("/", "2000")] {
            assert_eq!(binary("200", operator, "10%").unwrap(), expected);
        }
        assert_eq!(binary("200", "percent", "10").unwrap(), "20");
        assert_eq!(binary("200", "add", "10%").unwrap(), "220");
        assert_eq!(binary("200", "subtract", "10%").unwrap(), "180");
        assert_eq!(standard("200+10%").unwrap(), "220");
        assert_eq!(standard("200+10%+10%").unwrap(), "242");
        assert_eq!(standard("-200+10%").unwrap(), "-220");
        assert_eq!(unary("10", "percent").unwrap(), "0.1");
    }

    #[test]
    fn non_terminating_results_round_to_32_significant_digits() {
        assert_eq!(
            evaluate("1/3").unwrap(),
            "0.33333333333333333333333333333333"
        );
        assert_eq!(
            evaluate("1/6").unwrap(),
            "0.16666666666666666666666666666667"
        );
        assert_eq!(
            evaluate("-1/6").unwrap(),
            "-0.16666666666666666666666666666667"
        );
        assert_eq!(evaluate("1-1/(3e33)").unwrap(), "1");
        assert_eq!(
            evaluate("1/7e50").unwrap(),
            "1.4285714285714285714285714285714e-51"
        );
    }

    #[test]
    fn square_roots_do_not_use_binary_floats() {
        assert_eq!(evaluate("sqrt(0.04)").unwrap(), "0.2");
        assert_eq!(evaluate("sqrt(1/400)").unwrap(), "0.05");
        assert_eq!(
            evaluate("sqrt(2)").unwrap(),
            "1.4142135623730950488016887242097"
        );
        assert_eq!(evaluate("sqrt(2)^2").unwrap(), "2");
        assert_eq!(
            evaluate("sqrt(4)+1234567890123456789012345678901234567890").unwrap(),
            "1234567890123456789012345678901234567892"
        );
        for source in ["sqrt(4e9998)", "sqrt(4e-9998)"] {
            assert_eq!(
                evaluate(source).unwrap_err(),
                "Expression exceeds calculation budget"
            );
        }
        assert!(evaluate("sqrt(-1)").is_err());
    }

    #[test]
    fn supported_unary_and_power_operations() {
        assert_eq!(unary("0.1", "square").unwrap(), "0.01");
        assert_eq!(unary("4", "reciprocal").unwrap(), "0.25");
        assert_eq!(unary("-5", "abs").unwrap(), "5");
        assert_eq!(unary("-5", "negate").unwrap(), "5");
        assert_eq!(evaluate("2^3^2").unwrap(), "512");
        assert_eq!(evaluate("-2^2").unwrap(), "-4");
        assert_eq!(evaluate("2^-3").unwrap(), "0.125");
        assert_eq!(evaluate("abs(-2.5)").unwrap(), "2.5");
        assert!(evaluate("2^0.5").is_err());
    }

    #[test]
    fn numeric_state_normalization_rejects_expressions() {
        for (source, expected) in [
            (".5", "0.5"),
            ("1.", "1"),
            ("1e+2", "100"),
            ("-0.000", "0"),
            ("1e9999", "1e9999"),
            ("1e-9999", "1e-9999"),
        ] {
            assert_eq!(normalize(source).unwrap(), expected);
        }
        for source in [
            ".",
            "1e",
            "1e+",
            "1..2",
            "1 2",
            "NaN",
            "Infinity",
            "1+2",
            "--1",
            "1e99999999999999999999999999999",
        ] {
            assert!(normalize(source).is_err(), "{source}");
        }
    }

    #[test]
    fn failures_are_bounded_and_do_not_panic() {
        for source in [
            "1/0",
            "1/-0",
            "reciprocal(0)",
            "0^-1",
            "0^0",
            "1e9999*10",
            "1e-9999/10",
            "1e10000",
            "1e-10000",
            "2^9999^9999",
        ] {
            assert!(evaluate(source).is_err(), "{source}");
        }
        assert!(evaluate(&"9".repeat(MAX_SOURCE_BYTES + 1)).is_err());
        assert!(evaluate(&format!("{}1{}", "(".repeat(65), ")".repeat(65))).is_err());
        assert!(evaluate(&"1+".repeat(2_048)).is_err());
        assert_eq!(
            evaluate("((1e9999-1)/(1e9999+1))^3").unwrap_err(),
            "Expression exceeds calculation budget"
        );
        assert_eq!(
            evaluate("((4/5)^9999)^2*(4/5)^8664").unwrap_err(),
            "Expression exceeds calculation budget"
        );
    }

    #[test]
    fn outputs_fit_input_limits_and_can_be_reused() {
        let large = "9".repeat(4_096);
        assert_eq!(normalize(&large).unwrap(), large);
        assert_eq!(
            binary("1e9999", "+", "1e9999").unwrap_err(),
            "Expression exceeds calculation budget"
        );
        let result = normalize("2e9999").unwrap();
        assert_eq!(result, "2e9999");
        assert_eq!(binary(&result, "/", "2").unwrap(), "1e9999");
        let result = evaluate("10^4096").unwrap();
        assert_eq!(result, "1e4096");
        assert!(result.len() <= MAX_SOURCE_BYTES);
    }
}
