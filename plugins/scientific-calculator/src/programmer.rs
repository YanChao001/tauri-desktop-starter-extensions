//! 有界的二补码整数计算；所有进制、位运算和字长转换均不经过浮点数。

use serde::{Deserialize, Serialize};

const MAX_SOURCE_BYTES: usize = 4_096;
const MAX_TOKENS: usize = 2_048;
const MAX_DEPTH: usize = 64;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ProgrammerOutput {
    pub value: String,
    pub dec: String,
    pub hex: String,
    pub oct: String,
    pub bin: String,
    pub unsigned: String,
    pub radix: u32,
    pub bits: u32,
    pub carry: bool,
}

pub fn evaluate(source: &str, radix: u32, bits: u32) -> Result<ProgrammerOutput, String> {
    evaluate_with_carry(source, radix, bits, false)
}

pub fn evaluate_with_carry(
    source: &str,
    radix: u32,
    bits: u32,
    carry: bool,
) -> Result<ProgrammerOutput, String> {
    let word = Word::new(radix, bits)?;
    if source.len() > MAX_SOURCE_BYTES {
        return Err("Expression exceeds 4096 bytes".into());
    }
    if source.trim().is_empty() {
        return Err("Expression is empty".into());
    }
    let mut parser = Parser {
        tokens: tokenize(source, radix)?,
        index: 0,
        word,
        carry,
    };
    let value = parser.expression(0, 0)?;
    if parser.current() != &TokenKind::End {
        return Err(format!(
            "Unexpected token at byte {}",
            parser.tokens[parser.index].offset
        ));
    }
    Ok(word.output(value, parser.carry))
}

pub fn toggle_bit(
    source: &str,
    radix: u32,
    bits: u32,
    index: u32,
) -> Result<ProgrammerOutput, String> {
    let output = evaluate(source, radix, bits)?;
    let word = Word::new(radix, bits)?;
    if index >= bits {
        return Err("Bit index is outside the selected word size".into());
    }
    let value = output
        .unsigned
        .parse::<u64>()
        .map_err(|_| "Invalid integer result".to_string())?;
    Ok(word.output(value ^ (1_u64 << index), output.carry))
}

#[derive(Clone, Copy)]
struct Word {
    radix: u32,
    bits: u32,
    mask: u64,
}

impl Word {
    fn new(radix: u32, bits: u32) -> Result<Self, String> {
        if !matches!(radix, 2 | 8 | 10 | 16) {
            return Err("Radix must be 2, 8, 10 or 16".into());
        }
        if !matches!(bits, 8 | 16 | 32 | 64) {
            return Err("Word size must be 8, 16, 32 or 64 bits".into());
        }
        Ok(Self {
            radix,
            bits,
            mask: u64::MAX >> (64 - bits),
        })
    }

    fn signed(self, value: u64) -> i128 {
        let value = value & self.mask;
        if value & (1_u64 << (self.bits - 1)) == 0 {
            i128::from(value)
        } else {
            i128::from(value) - (1_i128 << self.bits)
        }
    }

    fn output(self, value: u64, carry: bool) -> ProgrammerOutput {
        let value = value & self.mask;
        let dec = self.signed(value).to_string();
        let hex = format!("{value:X}");
        let oct = format!("{value:o}");
        let bin = format!("{value:b}");
        let display = match self.radix {
            2 => &bin,
            8 => &oct,
            16 => &hex,
            _ => &dec,
        };
        ProgrammerOutput {
            value: display.clone(),
            dec,
            hex,
            oct,
            bin,
            unsigned: value.to_string(),
            radix: self.radix,
            bits: self.bits,
            carry,
        }
    }

    fn bit_index(self, index: u64) -> Result<u32, String> {
        if index >= u64::from(self.bits) {
            Err("Bit index is outside the selected word size".into())
        } else {
            Ok(index as u32)
        }
    }

    fn binary(self, op: Binary, left: u64, right: u64, carry: &mut bool) -> Result<u64, String> {
        let value = match op {
            Binary::Add => left.wrapping_add(right),
            Binary::Subtract => left.wrapping_sub(right),
            Binary::Multiply => left.wrapping_mul(right),
            Binary::Divide | Binary::Modulo => {
                let divisor = self.signed(right);
                if divisor == 0 {
                    return Err("Cannot divide by zero".into());
                }
                let dividend = self.signed(left);
                // i128 容纳最小有符号整数除以 -1，最后按所选字长截断。
                if op == Binary::Divide {
                    (dividend / divisor) as u64
                } else {
                    (dividend % divisor) as u64
                }
            }
            Binary::And => left & right,
            Binary::Or => left | right,
            Binary::Xor => left ^ right,
            Binary::Nand => !(left & right),
            Binary::Nor => !(left | right),
            Binary::ShiftLeft | Binary::ShiftRight | Binary::LogicalRight => {
                // Windows 在位移量达到字长时报告无结果，不使用处理器的取模位移。
                if right >= u64::from(self.bits) {
                    return Err("Shift count must be smaller than the selected word size".into());
                }
                match op {
                    Binary::ShiftLeft => left << right,
                    Binary::ShiftRight => (self.signed(left) >> right) as u64,
                    _ => left >> right,
                }
            }
            Binary::RotateLeft | Binary::RotateRight => {
                let count = (right % u64::from(self.bits)) as u32;
                if count == 0 {
                    left
                } else if op == Binary::RotateLeft {
                    (left << count) | (left >> (self.bits - count))
                } else {
                    (left >> count) | (left << (self.bits - count))
                }
            }
            Binary::RotateCarryLeft | Binary::RotateCarryRight => {
                // 将进位与当前整数视为 bits + 1 位，保留跨按键的进位状态。
                let width = self.bits + 1;
                let count = (right % u64::from(width)) as u32;
                let mask = (1_u128 << width) - 1;
                let extended = u128::from(left) | (u128::from(*carry) << self.bits);
                let rotated = if count == 0 {
                    extended
                } else if op == Binary::RotateCarryLeft {
                    ((extended << count) | (extended >> (width - count))) & mask
                } else {
                    ((extended >> count) | (extended << (width - count))) & mask
                };
                *carry = rotated & (1_u128 << self.bits) != 0;
                rotated as u64
            }
        };
        Ok(value & self.mask)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Binary {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    And,
    Or,
    Xor,
    Nand,
    Nor,
    ShiftLeft,
    ShiftRight,
    LogicalRight,
    RotateLeft,
    RotateRight,
    RotateCarryLeft,
    RotateCarryRight,
}

impl Binary {
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "and" => Self::And,
            "or" => Self::Or,
            "xor" => Self::Xor,
            "nand" => Self::Nand,
            "nor" => Self::Nor,
            "mod" => Self::Modulo,
            "shl" | "lsh" | "lsl" => Self::ShiftLeft,
            "shr" | "rsh" | "asr" => Self::ShiftRight,
            "lsr" | "rshl" => Self::LogicalRight,
            "rol" => Self::RotateLeft,
            "ror" => Self::RotateRight,
            "rcl" | "rolc" => Self::RotateCarryLeft,
            "rcr" | "rorc" => Self::RotateCarryRight,
            _ => return None,
        })
    }

    fn precedence(self) -> u8 {
        match self {
            Self::Or | Self::Xor => 1,
            Self::And | Self::Nand | Self::Nor => 2,
            Self::Add | Self::Subtract => 3,
            Self::ShiftLeft
            | Self::ShiftRight
            | Self::LogicalRight
            | Self::RotateLeft
            | Self::RotateRight
            | Self::RotateCarryLeft
            | Self::RotateCarryRight
            | Self::Multiply
            | Self::Divide
            | Self::Modulo => 4,
        }
    }

    fn is_shift(self) -> bool {
        matches!(
            self,
            Self::ShiftLeft
                | Self::ShiftRight
                | Self::LogicalRight
                | Self::RotateLeft
                | Self::RotateRight
                | Self::RotateCarryLeft
                | Self::RotateCarryRight
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
enum TokenKind {
    Number(u64),
    Operator(Binary),
    Name(String),
    Not,
    Left,
    Right,
    Comma,
    End,
}

struct Token {
    kind: TokenKind,
    offset: usize,
}

fn tokenize(source: &str, radix: u32) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut offset = 0;
    let bytes = source.as_bytes();
    while offset < bytes.len() {
        if bytes[offset].is_ascii_whitespace() {
            offset += 1;
            continue;
        }
        let start = offset;
        let byte = bytes[offset];
        offset += 1;
        let kind = match byte {
            b'+' => TokenKind::Operator(Binary::Add),
            b'-' => TokenKind::Operator(Binary::Subtract),
            b'*' => TokenKind::Operator(Binary::Multiply),
            b'/' => TokenKind::Operator(Binary::Divide),
            b'%' => TokenKind::Operator(Binary::Modulo),
            b'&' => TokenKind::Operator(Binary::And),
            b'|' => TokenKind::Operator(Binary::Or),
            b'^' => TokenKind::Operator(Binary::Xor),
            b'~' => TokenKind::Not,
            b'(' => TokenKind::Left,
            b')' => TokenKind::Right,
            b',' => TokenKind::Comma,
            b'<' | b'>' if bytes.get(offset) == Some(&byte) => {
                offset += 1;
                let op = if byte == b'<' {
                    Binary::ShiftLeft
                } else if bytes.get(offset) == Some(&b'>') {
                    offset += 1;
                    Binary::LogicalRight
                } else {
                    Binary::ShiftRight
                };
                TokenKind::Operator(op)
            }
            byte if byte.is_ascii_alphanumeric() => {
                while bytes.get(offset).is_some_and(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'\'' | b'`')
                }) {
                    offset += 1;
                }
                let word = source[start..offset].to_ascii_lowercase();
                if word == "not" {
                    TokenKind::Not
                } else if let Some(op) = Binary::named(&word) {
                    TokenKind::Operator(op)
                } else if matches!(word.as_str(), "neg" | "bit" | "togglebit") {
                    TokenKind::Name(word)
                } else {
                    TokenKind::Number(parse_number(&word, radix, start)?)
                }
            }
            _ => return Err(format!("Invalid integer expression at byte {start}")),
        };
        if tokens.len() >= MAX_TOKENS {
            return Err("Expression exceeds 2048 tokens".into());
        }
        tokens.push(Token {
            kind,
            offset: start,
        });
    }
    tokens.push(Token {
        kind: TokenKind::End,
        offset,
    });
    Ok(tokens)
}

fn parse_number(source: &str, radix: u32, offset: usize) -> Result<u64, String> {
    let (digits, base) = if source.len() > 2 {
        match &source[..2] {
            "0x" => (&source[2..], 16),
            "0d" => (&source[2..], 10),
            "0o" => (&source[2..], 8),
            "0b" => (&source[2..], 2),
            _ => (source, radix),
        }
    } else {
        (source, radix)
    };
    if !digits.chars().next().is_some_and(|c| c.is_digit(base))
        || !digits.chars().last().is_some_and(|c| c.is_digit(base))
    {
        return Err(format!("Invalid base-{base} integer at byte {offset}"));
    }
    let digits: String = digits
        .chars()
        .filter(|c| !matches!(c, '_' | '\'' | '`'))
        .collect();
    u64::from_str_radix(&digits, base)
        .map_err(|_| format!("Invalid or oversized base-{base} integer at byte {offset}"))
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    word: Word,
    carry: bool,
}

impl Parser {
    fn current(&self) -> &TokenKind {
        &self.tokens[self.index].kind
    }

    fn expression(&mut self, minimum: u8, depth: usize) -> Result<u64, String> {
        if depth > MAX_DEPTH {
            return Err("Expression nesting exceeds 64 levels".into());
        }
        let mut left = self.primary(depth)?;
        while let TokenKind::Operator(op) = self.current() {
            let op = *op;
            if op.precedence() < minimum {
                break;
            }
            self.index += 1;
            let right = self.expression(op.precedence() + 1, depth + 1)?;
            left = self.word.binary(op, left, right, &mut self.carry)?;
        }
        Ok(left)
    }

    fn primary(&mut self, depth: usize) -> Result<u64, String> {
        match self.current() {
            TokenKind::Number(value) => {
                let value = *value & self.word.mask;
                self.index += 1;
                Ok(value)
            }
            TokenKind::Left => {
                self.index += 1;
                let value = self.expression(0, depth + 1)?;
                self.require_right()?;
                Ok(value)
            }
            TokenKind::Not => {
                self.index += 1;
                Ok(!self.expression(7, depth + 1)? & self.word.mask)
            }
            TokenKind::Operator(op @ (Binary::Add | Binary::Subtract)) => {
                let negative = *op == Binary::Subtract;
                self.index += 1;
                let value = self.expression(7, depth + 1)?;
                Ok(if negative {
                    value.wrapping_neg() & self.word.mask
                } else {
                    value
                })
            }
            TokenKind::Operator(op) => {
                let op = *op;
                self.index += 1;
                self.function(op, depth)
            }
            TokenKind::Name(name) => {
                let name = name.clone();
                self.index += 1;
                self.named_function(&name, depth)
            }
            _ => Err(format!(
                "Expected an integer at byte {}",
                self.tokens[self.index].offset
            )),
        }
    }

    fn function(&mut self, op: Binary, depth: usize) -> Result<u64, String> {
        if self.current() != &TokenKind::Left {
            return Err("Integer function requires parentheses".into());
        }
        self.index += 1;
        let value = self.expression(0, depth + 1)?;
        let count = if self.current() == &TokenKind::Comma {
            self.index += 1;
            self.expression(0, depth + 1)?
        } else if op.is_shift() {
            1
        } else {
            return Err("Integer function requires two arguments".into());
        };
        self.require_right()?;
        self.word.binary(op, value, count, &mut self.carry)
    }

    fn named_function(&mut self, name: &str, depth: usize) -> Result<u64, String> {
        if self.current() != &TokenKind::Left {
            return Err("Integer function requires parentheses".into());
        }
        self.index += 1;
        let value = self.expression(0, depth + 1)?;
        if name == "neg" {
            self.require_right()?;
            return Ok(value.wrapping_neg() & self.word.mask);
        }
        if self.current() != &TokenKind::Comma {
            return Err("Bit function requires a value and bit index".into());
        }
        self.index += 1;
        let raw_index = self.expression(0, depth + 1)?;
        let index = self.word.bit_index(raw_index)?;
        self.require_right()?;
        Ok(if name == "bit" {
            (value >> index) & 1
        } else {
            value ^ (1_u64 << index)
        })
    }

    fn require_right(&mut self) -> Result<(), String> {
        if self.current() != &TokenKind::Right {
            return Err(format!(
                "Expected ')' at byte {}",
                self.tokens[self.index].offset
            ));
        }
        self.index += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(source: &str, bits: u32) -> String {
        evaluate(source, 10, bits).unwrap().dec
    }

    #[test]
    fn preserves_all_64_integer_bits_and_signed_display() {
        let value = evaluate("FFFFFFFFFFFFFFFF", 16, 64).unwrap();
        assert_eq!(value.value, "FFFFFFFFFFFFFFFF");
        assert_eq!(value.dec, "-1");
        assert_eq!(value.unsigned, "18446744073709551615");
        assert_eq!(value.bin, "1".repeat(64));
        assert_eq!(dec("9007199254740993 + 2", 64), "9007199254740995");
        assert_eq!(dec("9223372036854775807 + 1", 64), "-9223372036854775808");
    }

    #[test]
    fn wraps_arithmetic_and_literals_at_the_selected_word_size() {
        for bits in [8, 16, 32, 64] {
            assert_eq!(dec("0 - 1", bits), "-1");
            assert_eq!(dec("18446744073709551615 + 1", bits), "0");
            assert_eq!(dec("18446744073709551615 * 2", bits), "-2");
            let signed_minimum = -(1_i128 << (bits - 1));
            assert_eq!(
                dec(&format!("{signed_minimum} / -1"), bits),
                signed_minimum.to_string()
            );
        }
        assert_eq!(dec("256", 8), "0");
        assert_eq!(dec("255", 8), "-1");
        assert_eq!(dec("128", 8), "-128");
        assert_eq!(dec("neg(-128)", 8), "-128");
    }

    #[test]
    fn converts_all_bases_without_floating_point_rounding() {
        let expected = evaluate("1234567890123456789", 10, 64).unwrap();
        for (radix, source) in [
            (2, expected.bin.as_str()),
            (8, expected.oct.as_str()),
            (10, expected.dec.as_str()),
            (16, expected.hex.as_str()),
        ] {
            let value = evaluate(source, radix, 64).unwrap();
            assert_eq!(value.dec, expected.dec);
            assert_eq!(value.hex, expected.hex);
            assert_eq!(value.oct, expected.oct);
            assert_eq!(value.bin, expected.bin);
        }
        assert_eq!(evaluate("ffff'ffff_ffff`ffff", 16, 64).unwrap().dec, "-1");
        assert_eq!(
            evaluate("0d10 + 0x10 + 0b10 + 0o10", 2, 64).unwrap().dec,
            "36"
        );
    }

    #[test]
    fn observes_arithmetic_precedence_and_parentheses() {
        assert_eq!(dec("2+3*4", 64), "14");
        assert_eq!(dec("(2+3)*4", 64), "20");
        assert_eq!(dec("8-3-2", 64), "3");
        assert_eq!(dec("1 << 2 + 1", 64), "5");
        assert_eq!(dec("8 >> 1 * 2", 64), "8");
        assert_eq!(dec("1 or 2 xor 3", 64), "0");
        assert_eq!(dec("1 nor 2 and 3", 8), "0");
        assert_eq!(dec("1 or 2 xor 3 and 4", 64), "3");
        assert_eq!(dec("not 0 and 15", 64), "15");
        assert_eq!(dec("(NOT 0) AND 15", 64), "15");
    }

    #[test]
    fn divides_signed_values_toward_zero_and_remainder_follows_dividend() {
        assert_eq!(dec("-7 / 3", 64), "-2");
        assert_eq!(dec("7 / -3", 64), "-2");
        assert_eq!(dec("-7 / -3", 64), "2");
        assert_eq!(dec("-7 % 3", 64), "-1");
        assert_eq!(dec("7 mod -3", 64), "1");
        assert!(evaluate("3 / 0", 10, 64).is_err());
        assert!(evaluate("3 % 0", 10, 64).is_err());
    }

    #[test]
    fn evaluates_complements_and_all_six_bitwise_operations() {
        assert_eq!(evaluate("AA and CC", 16, 8).unwrap().hex, "88");
        assert_eq!(evaluate("AA or CC", 16, 8).unwrap().hex, "EE");
        assert_eq!(evaluate("AA xor CC", 16, 8).unwrap().hex, "66");
        assert_eq!(evaluate("AA nand CC", 16, 8).unwrap().hex, "77");
        assert_eq!(evaluate("AA nor CC", 16, 8).unwrap().hex, "11");
        assert_eq!(evaluate("NOT AA", 16, 8).unwrap().hex, "55");
        assert_eq!(evaluate("~AA", 16, 8).unwrap().hex, "55");
        assert_eq!(evaluate("xor(AA, CC)", 16, 8).unwrap().hex, "66");
    }

    #[test]
    fn distinguishes_logical_and_arithmetic_shifts() {
        assert_eq!(dec("-128 >> 1", 8), "-64");
        assert_eq!(dec("-128 >>> 1", 8), "64");
        assert_eq!(dec("-1 >> 7", 8), "-1");
        assert_eq!(dec("-1 >>> 7", 8), "1");
        assert_eq!(dec("1 << 7", 8), "-128");
        assert_eq!(dec("shl(128)", 8), "0");
        assert_eq!(dec("shr(128)", 8), "-64");
        assert_eq!(dec("lsr(128)", 8), "64");
        assert!(evaluate("1 << 8", 10, 8).is_err());
        assert!(evaluate("1 >> -1", 10, 8).is_err());
    }

    #[test]
    fn rotates_only_the_selected_word_and_keeps_the_carry_separate() {
        assert_eq!(evaluate("rol(80)", 16, 8).unwrap().hex, "1");
        assert_eq!(evaluate("ror(1)", 16, 8).unwrap().hex, "80");
        assert_eq!(evaluate("rol(81, 0d8)", 16, 8).unwrap().hex, "81");
        assert_eq!(evaluate("ror(1, 0d9)", 16, 8).unwrap().hex, "80");
        assert_eq!(evaluate("rol(8000000000000000)", 16, 64).unwrap().hex, "1");
        let first = evaluate_with_carry("rcl(80)", 16, 8, false).unwrap();
        assert_eq!(first.hex, "0");
        assert!(first.carry);
        let second = evaluate_with_carry("rcl(0)", 16, 8, first.carry).unwrap();
        assert_eq!(second.hex, "1");
        assert!(!second.carry);
        let right = evaluate_with_carry("rcr(1)", 16, 64, true).unwrap();
        assert_eq!(right.hex, "8000000000000000");
        assert!(right.carry);
        let full = evaluate_with_carry("rcl(81,0d9)", 16, 8, true).unwrap();
        assert_eq!(full.hex, "81");
        assert!(full.carry);
        assert!(evaluate_with_carry("rol(80)", 16, 8, true).unwrap().carry);
    }

    #[test]
    fn flips_and_reads_all_bits_with_explicit_decimal_indices() {
        for bits in [8, 16, 32, 64] {
            for index in 0..bits {
                let output = toggle_bit("0", 10, bits, index).unwrap();
                assert_eq!(output.unsigned, (1_u64 << index).to_string());
                assert_eq!(
                    evaluate(&format!("bit(0x{},0d{index})", output.hex), 16, bits)
                        .unwrap()
                        .value,
                    "1"
                );
                assert_eq!(
                    evaluate(&format!("togglebit(0x{},0d{index})", output.hex), 16, bits)
                        .unwrap()
                        .value,
                    "0"
                );
            }
        }
        assert!(toggle_bit("0", 10, 64, 64).is_err());
        assert!(evaluate("bit(0,64)", 10, 64).is_err());
    }

    #[test]
    fn rejects_invalid_integer_inputs_configuration_and_unbounded_nesting() {
        for source in [
            "",
            "1.5",
            "1e3",
            "0x",
            "18446744073709551616",
            "1+",
            "1 2",
            "(1",
            "1)",
            "not()",
            "bit(1)",
            "and(1)",
            "rol(1,2,3)",
            "hello",
            "π",
        ] {
            assert!(evaluate(source, 10, 64).is_err(), "{source}");
        }
        for (source, radix) in [("2", 2), ("8", 8), ("A", 10), ("G", 16)] {
            assert!(evaluate(source, radix, 64).is_err());
        }
        assert!(evaluate("0", 3, 64).is_err());
        assert!(evaluate("0", 10, 128).is_err());
        assert!(evaluate(&format!("{}0{}", "(".repeat(65), ")".repeat(65)), 10, 64).is_err());
        assert!(evaluate(&"1+".repeat(2_048), 10, 64).is_err());
        assert!(evaluate(&" ".repeat(4_097), 10, 64).is_err());
    }
}
