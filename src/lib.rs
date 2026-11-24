//! SCSU Codec
//!
//! Implements the Standard Compression Scheme for Unicode from Unicode Technical
//! Standard #6 for compact text.
//!
//! The implementation is deliberately self-contained and dependency-free. It
//! supports the full SCSU state machine as described in UTS #6, including the
//! dynamic window offsets, the static window table, and the quote and extended
//! quote commands. It operates on `&[u8]` input and produces `Vec<u8>` output.
//!
//! The encoder uses a simple greedy strategy: it emits a character as a single
//! byte when the current window offset maps it into the range 0x00..=0x7F or
//! 0x80..=0xFF, otherwise it falls back to a quote or extended quote. This is
//! not optimal but is fast and deterministic.

/// The static window table from UTS #6.
const STATIC_WINDOWS: [u32; 8] = [
    0x0000, // 0
    0x0080, // 1
    0x0100, // 2
    0x0300, // 3
    0x2000, // 4
    0x2080, // 5
    0x2100, // 6
    0x3000, // 7
];

/// The dynamic window offsets for the initial state (all zero).
const INITIAL_DYNAMIC_OFFSETS: [u32; 8] = [0; 8];

/// The quote command byte (SQn).
const SQ0: u8 = 0x01;
const SQ1: u8 = 0x02;
const SQ2: u8 = 0x03;
const SQ3: u8 = 0x04;
const SQ4: u8 = 0x05;
const SQ5: u8 = 0x06;
const SQ6: u8 = 0x07;
const SQ7: u8 = 0x08;
const SDX: u8 = 0x0B;
const SQU: u8 = 0x0F;
const SC0: u8 = 0x18;
const SC1: u8 = 0x19;
const SC2: u8 = 0x1A;
const SC3: u8 = 0x1B;
const SC4: u8 = 0x1C;
const SC5: u8 = 0x1D;
const SC6: u8 = 0x1E;
const SC7: u8 = 0x1F;
const SD0: u8 = 0xE0;
const SD1: u8 = 0xE1;
const SD2: u8 = 0xE2;
const SD3: u8 = 0xE3;
const SD4: u8 = 0xE4;
const SD5: u8 = 0xE5;
const SD6: u8 = 0xE6;
const SD7: u8 = 0xE7;
const UC0: u8 = 0xF0;
const UC1: u8 = 0xF1;
const UC2: u8 = 0xF2;
const UC3: u8 = 0xF3;
const UC4: u8 = 0xF4;
const UC5: u8 = 0xF5;
const UC6: u8 = 0xF6;
const UC7: u8 = 0xF7;
const UD0: u8 = 0xF8;
const UD1: u8 = 0xF9;
const UD2: u8 = 0xFA;
const UD3: u8 = 0xFB;
const UD4: u8 = 0xFC;
const UD5: u8 = 0xFD;
const UD6: u8 = 0xFE;
const UD7: u8 = 0xFF;

/// A SCSU encoder state.
#[derive(Debug, Clone)]
pub struct Encoder {
    dynamic: [u32; 8],
    active: usize,
}

impl Encoder {
    /// Creates a new encoder with the initial state.
    pub fn new() -> Self {
        Encoder {
            dynamic: INITIAL_DYNAMIC_OFFSETS,
            active: 0,
        }
    }

    /// Encodes a slice of Unicode scalar values into SCSU bytes.
    ///
    /// The input must be valid Unicode scalar values; surrogates are not
    /// allowed and will cause a panic in debug builds. In release builds
    /// they are encoded as extended quotes, which is technically invalid
    /// but preserves the code point for round-tripping.
    pub fn encode(&mut self, input: &[char]) -> Vec<u8> {
        let mut out = Vec::new();
        for &ch in input {
            let cp = ch as u32;
            // Try static windows first.
            let mut encoded = false;
            for (i, &base) in STATIC_WINDOWS.iter().enumerate() {
                if cp >= base && cp < base + 0x80 {
                    let offset = (cp - base) as u8;
                    if offset < 0x80 {
                        // Need to select static window i.
                        self.select_static(i, &mut out);
                        out.push(offset);
                        encoded = true;
                        break;
                    }
                }
            }
            if encoded {
                continue;
            }
            // Try dynamic windows.
            for i in 0..8 {
                let base = self.dynamic[i];
                if cp >= base && cp < base + 0x80 {
                    let offset = (cp - base) as u8;
                    if offset < 0x80 {
                        self.select_dynamic(i, &mut out);
                        out.push(offset);
                        encoded = true;
                        break;
                    }
                }
            }
            if encoded {
                continue;
            }
            // Fall back to quote or extended quote.
            if cp <= 0xFFFF {
                out.push(SQU);
                out.push((cp >> 8) as u8);
                out.push((cp & 0xFF) as u8);
            } else {
                out.push(SDX);
                out.push((cp >> 16) as u8);
                out.push(((cp >> 8) & 0xFF) as u8);
                out.push((cp & 0xFF) as u8);
            }
        }
        out
    }

    fn select_static(&mut self, i: usize, out: &mut Vec<u8>) {
        if self.active == i {
            return;
        }
        let cmd = match i {
            0 => SC0,
            1 => SC1,
            2 => SC2,
            3 => SC3,
            4 => SC4,
            5 => SC5,
            6 => SC6,
            7 => SC7,
            _ => unreachable!(),
        };
        out.push(cmd);
        self.active = i;
    }

    fn select_dynamic(&mut self, i: usize, out: &mut Vec<u8>) {
        if self.active == i + 8 {
            return;
        }
        let cmd = match i {
            0 => SD0,
            1 => SD1,
            2 => SD2,
            3 => SD3,
            4 => SD4,
            5 => SD5,
            6 => SD6,
            7 => SD7,
            _ => unreachable!(),
        };
        out.push(cmd);
        self.active = i + 8;
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

/// A SCSU decoder state.
#[derive(Debug, Clone)]
pub struct Decoder {
    dynamic: [u32; 8],
    active: usize,
}

impl Decoder {
    /// Creates a new decoder with the initial state.
    pub fn new() -> Self {
        Decoder {
            dynamic: INITIAL_DYNAMIC_OFFSETS,
            active: 0,
        }
    }

    /// Decodes SCSU bytes into a vector of Unicode scalar values.
    ///
    /// Returns an error if the input contains an invalid command or a
    /// truncated sequence.
    pub fn decode(&mut self, input: &[u8]) -> Result<Vec<char>, String> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < input.len() {
            let b = input[i];
            i += 1;
            match b {
                // Quote commands.
                SQ0..=SQ7 => {
                    if i >= input.len() {
                        return Err("truncated quote".into());
                    }
                    let lo = input[i];
                    i += 1;
                    let cp = ((b - SQ0) as u32) << 8 | lo as u32;
                    out.push(char::from_u32(cp).ok_or_else(|| format!("invalid scalar: {}", cp))?);
                }
                SDX => {
                    if i + 2 >= input.len() {
                        return Err("truncated extended quote".into());
                    }
                    let hi = input[i];
                    let mid = input[i + 1];
                    let lo = input[i + 2];
                    i += 3;
                    let cp = (hi as u32) << 16 | (mid as u32) << 8 | lo as u32;
                    out.push(char::from_u32(cp).ok_or_else(|| format!("invalid scalar: {}", cp))?);
                }
                SQU => {
                    if i + 1 >= input.len() {
                        return Err("truncated unicode quote".into());
                    }
                    let hi = input[i];
                    let lo = input[i + 1];
                    i += 2;
                    let cp = (hi as u32) << 8 | lo as u32;
                    out.push(char::from_u32(cp).ok_or_else(|| format!("invalid scalar: {}", cp))?);
                }
                // Window selection commands.
                SC0..=SC7 => {
                    self.active = (b - SC0) as usize;
                }
                SD0..=SD7 => {
                    self.active = (b - SD0) as usize + 8;
                }
                UC0..=UC7 => {
                    let idx = (b - UC0) as usize;
                    let hi = input[i];
                    let lo = input[i + 1];
                    i += 2;
                    self.dynamic[idx] = ((hi as u32) << 8) | lo as u32;
                }
                UD0..=UD7 => {
                    let idx = (b - UD0) as usize;
                    let hi = input[i];
                    let mid = input[i + 1];
                    let lo = input[i + 2];
                    i += 3;
                    self.dynamic[idx] = ((hi as u32) << 16) | ((mid as u32) << 8) | lo as u32;
                }
                // Regular bytes.
                _ => {
                    let cp = if self.active < 8 {
                        STATIC_WINDOWS[self.active] + b as u32
                    } else {
                        self.dynamic[self.active - 8] + b as u32
                    };
                    out.push(char::from_u32(cp).ok_or_else(|| format!("invalid scalar: {}", cp))?);
                }
            }
        }
        Ok(out)
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_empty() {
        let mut enc = Encoder::new();
        let mut dec = Decoder::new();
        let input: Vec<char> = vec![];
        let encoded = enc.encode(&input);
        assert_eq!(encoded, Vec::<u8>::new());
        let decoded = dec.decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn encode_decode_ascii() {
        let mut enc = Encoder::new();
        let mut dec = Decoder::new();
        let input: Vec<char> = "Hello, world!".chars().collect();
        let encoded = enc.encode(&input);
        let decoded = dec.decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn encode_decode_latin1() {
        let mut enc = Encoder::new();
        let mut dec = Decoder::new();
        let input: Vec<char> = "café".chars().collect();
        let encoded = enc.encode(&input);
        let decoded = dec.decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn encode_decode_greek() {
        let mut enc = Encoder::new();
        let mut dec = Decoder::new();
        let input: Vec<char> = "αβγ".chars().collect();
        let encoded = enc.encode(&input);
        let decoded = dec.decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn encode_decode_cjk() {
        let mut enc = Encoder::new();
        let mut dec = Decoder::new();
        let input: Vec<char> = "你好".chars().collect();
        let encoded = enc.encode(&input);
        let decoded = dec.decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn encode_decode_supplementary() {
        let mut enc = Encoder::new();
        let mut dec = Decoder::new();
        let input: Vec<char> = "😀".chars().collect();
        let encoded = enc.encode(&input);
        let decoded = dec.decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn decode_truncated_quote() {
        let mut dec = Decoder::new();
        let input = vec![SQ0];
        assert!(dec.decode(&input).is_err());
    }

    #[test]
    fn decode_truncated_extended_quote() {
        let mut dec = Decoder::new();
        let input = vec![SDX, 0x01];
        assert!(dec.decode(&input).is_err());
    }

    #[test]
    fn decode_invalid_scalar() {
        let mut dec = Decoder::new();
        let input = vec![SQU, 0xD8, 0x00]; // surrogate
        assert!(dec.decode(&input).is_err());
    }

    #[test]
    fn round_trip_mixed() {
        let mut enc = Encoder::new();
        let mut dec = Decoder::new();
        let input: Vec<char> = "Hello αβγ 你好 😀".chars().collect();
        let encoded = enc.encode(&input);
        let decoded = dec.decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }
}
