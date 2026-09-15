//! JSON syntax and explicit container state, independent of Rust call-stack depth.

use num_bigint::BigInt;

use crate::error::Result;
use crate::util::json_numbers::MAX_INTEGER_DIGITS;

use super::{Object, Text, Value, invalid};

enum Frame {
    Array(Vec<Value>),
    Object(Object, Text),
}

pub(super) fn parse(input: &[u32]) -> Result<Value> {
    let mut reader = Reader {
        input,
        position: 0,
    };
    let mut stack = Vec::new();
    loop {
        reader.whitespace();
        let mut value = match reader.ascii() {
            Some(b'[') => {
                reader.position += 1;
                if reader.consume(b']') {
                    Value::Array(Vec::new())
                } else {
                    stack.push(Frame::Array(Vec::new()));
                    continue;
                }
            }
            Some(b'{') => {
                reader.position += 1;
                if reader.consume(b'}') {
                    Value::Object(Object::default())
                } else {
                    let key = reader.string()?;
                    reader.expect(b':')?;
                    stack.push(Frame::Object(Object::default(), key));
                    continue;
                }
            }
            Some(b'"') => Value::String(reader.string()?),
            Some(b'n') => {
                reader.literal("null")?;
                Value::Null
            }
            Some(b't') => {
                reader.literal("true")?;
                Value::Bool(true)
            }
            Some(b'f') => {
                reader.literal("false")?;
                Value::Bool(false)
            }
            Some(b'N') => {
                reader.literal("NaN")?;
                Value::Float(f64::NAN)
            }
            Some(b'I') => {
                reader.literal("Infinity")?;
                Value::Float(f64::INFINITY)
            }
            Some(b'-') if reader.input.get(reader.position + 1) == Some(&u32::from(b'I')) => {
                reader.literal("-Infinity")?;
                Value::Float(f64::NEG_INFINITY)
            }
            Some(b'-' | b'0'..=b'9') => reader.number()?,
            _ => return Err(invalid(format!("expected value at character {}", reader.position))),
        };
        loop {
            match stack.pop() {
                None => {
                    reader.whitespace();
                    return if reader.peek().is_none() {
                        Ok(value)
                    } else {
                        Err(invalid("extra data after document"))
                    };
                }
                Some(Frame::Array(mut values)) => {
                    values.push(value);
                    if reader.consume(b']') {
                        value = Value::Array(values);
                    } else {
                        reader.expect(b',')?;
                        stack.push(Frame::Array(values));
                        break;
                    }
                }
                Some(Frame::Object(mut object, key)) => {
                    object.0.insert(key, value);
                    if reader.consume(b'}') {
                        value = Value::Object(object);
                    } else {
                        reader.expect(b',')?;
                        let key = reader.string()?;
                        reader.expect(b':')?;
                        stack.push(Frame::Object(object, key));
                        break;
                    }
                }
            }
        }
    }
}

struct Reader<'a> {
    input: &'a [u32],
    position: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u32> {
        self.input.get(self.position).copied()
    }

    fn ascii(&self) -> Option<u8> {
        self.peek().and_then(|point| u8::try_from(point).ok()).filter(u8::is_ascii)
    }

    fn whitespace(&mut self) {
        while matches!(self.ascii(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.position += 1;
        }
    }

    fn consume(&mut self, byte: u8) -> bool {
        self.whitespace();
        if self.ascii() == Some(byte) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, byte: u8) -> Result<()> {
        if self.consume(byte) {
            Ok(())
        } else {
            Err(invalid(format!("expected '{}' at character {}", char::from(byte), self.position)))
        }
    }

    fn literal(&mut self, text: &str) -> Result<()> {
        for byte in text.bytes() {
            if self.ascii() != Some(byte) {
                return Err(invalid("invalid literal"));
            }
            self.position += 1;
        }
        Ok(())
    }

    fn number(&mut self) -> Result<Value> {
        let start = self.position;
        if self.ascii() == Some(b'-') {
            self.position += 1;
        }
        if self.ascii() == Some(b'0') {
            self.position += 1;
        } else {
            self.digits()?;
        }
        let integer_end = self.position;
        let mut float = false;
        if self.ascii() == Some(b'.') {
            float = true;
            self.position += 1;
            self.digits()?;
        }
        if matches!(self.ascii(), Some(b'e' | b'E')) {
            float = true;
            self.position += 1;
            if matches!(self.ascii(), Some(b'+' | b'-')) {
                self.position += 1;
            }
            self.digits()?;
        }
        let text: String = self.input[start..self.position]
            .iter()
            .map(|&point| char::from_u32(point).expect("ASCII number grammar"))
            .collect();
        if float {
            return text.parse().map(Value::Float).map_err(invalid);
        }
        let digits = integer_end - start - usize::from(text.starts_with('-'));
        if digits > MAX_INTEGER_DIGITS {
            return Err(invalid("integer exceeds Python's 4300-digit default limit"));
        }
        BigInt::parse_bytes(text.as_bytes(), 10)
            .map(Value::Integer)
            .ok_or_else(|| invalid("invalid integer"))
    }

    fn digits(&mut self) -> Result<()> {
        let start = self.position;
        while matches!(self.ascii(), Some(b'0'..=b'9')) {
            self.position += 1;
        }
        if self.position == start {
            Err(invalid("expected decimal digit"))
        } else {
            Ok(())
        }
    }

    fn string(&mut self) -> Result<Text> {
        self.expect(b'"')?;
        let mut points = Vec::new();
        loop {
            let point = self.peek().ok_or_else(|| invalid("unterminated string"))?;
            self.position += 1;
            match u8::try_from(point).ok() {
                Some(b'"') => return Ok(Text(points)),
                Some(0..=0x1f) => return Err(invalid("unescaped string control character")),
                Some(b'\\') => {
                    let escaped = self.ascii().ok_or_else(|| invalid("invalid string escape"))?;
                    self.position += 1;
                    points.push(match escaped {
                        b'"' | b'\\' | b'/' => u32::from(escaped),
                        b'b' => 8,
                        b'f' => 12,
                        b'n' => u32::from(b'\n'),
                        b'r' => u32::from(b'\r'),
                        b't' => u32::from(b'\t'),
                        b'u' => self.unicode_escape()?,
                        _ => return Err(invalid("invalid string escape")),
                    });
                }
                _ => points.push(point),
            }
        }
    }

    fn unicode_escape(&mut self) -> Result<u32> {
        let high = self.hex_word()?;
        if (0xd800..=0xdbff).contains(&high)
            && self.input.get(self.position..self.position + 2)
                == Some(&[u32::from(b'\\'), u32::from(b'u')])
        {
            let saved = self.position;
            self.position += 2;
            let low = self.hex_word()?;
            if (0xdc00..=0xdfff).contains(&low) {
                return Ok(0x10000 + ((high - 0xd800) << 10) + low - 0xdc00);
            }
            self.position = saved;
        }
        Ok(high)
    }

    fn hex_word(&mut self) -> Result<u32> {
        let mut result = 0;
        for _ in 0..4 {
            let digit = self
                .peek()
                .and_then(char::from_u32)
                .and_then(|character| character.to_digit(16))
                .ok_or_else(|| invalid("invalid Unicode escape"))?;
            self.position += 1;
            result = (result << 4) | digit;
        }
        Ok(result)
    }
}
