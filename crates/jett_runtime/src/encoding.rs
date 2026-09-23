//! Shared checked encoding kernels for interpreted and native execution.

pub fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() {
            data[i + 1] as u32
        } else {
            0
        };
        let b2 = if i + 2 < data.len() {
            data[i + 2] as u32
        } else {
            0
        };
        let combined = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((combined >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((combined >> 12) & 63) as usize] as char);
        if i + 1 < data.len() {
            out.push(ALPHABET[((combined >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < data.len() {
            out.push(ALPHABET[(combined & 63) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

pub fn base64_decode(s: &str) -> Result<Vec<u8>, &'static str> {
    fn char_val(c: u8) -> u32 {
        match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            b'=' => 0,
            _ => unreachable!("alphabet was validated before decoding"),
        }
    }

    let bytes = s.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err("invalid length");
    }
    if bytes.iter().any(|byte| {
        !matches!(
            byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'='
        )
    }) {
        return Err("invalid character");
    }

    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if padding > 2
        || bytes[..bytes.len().saturating_sub(padding)].contains(&b'=')
        || (padding > 0 && bytes.len() < 4)
    {
        return Err("invalid padding");
    }

    let mut out = Vec::new();
    for chunk in bytes.chunks_exact(4) {
        let chunk_padding = usize::from(chunk[3] == b'=') + usize::from(chunk[2] == b'=');
        let v0 = char_val(chunk[0]);
        let v1 = char_val(chunk[1]);
        let v2 = char_val(chunk[2]);
        let v3 = char_val(chunk[3]);
        if (chunk_padding == 2 && v1 & 0x0F != 0) || (chunk_padding == 1 && v2 & 0x03 != 0) {
            return Err("non-zero trailing bits");
        }
        let combined = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
        out.push(((combined >> 16) & 0xFF) as u8);
        if chunk_padding < 2 {
            out.push(((combined >> 8) & 0xFF) as u8);
        }
        if chunk_padding == 0 {
            out.push((combined & 0xFF) as u8);
        }
    }
    Ok(out)
}

pub fn encoding_hex_decode(s: &str) -> Result<Vec<u8>, &'static str> {
    fn nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let raw = s.as_bytes();
    if raw.len() % 2 != 0 {
        return Err("odd-length hex string");
    }
    raw.chunks_exact(2)
        .map(|pair| {
            let high = nibble(pair[0]).ok_or("invalid hex characters")?;
            let low = nibble(pair[1]).ok_or("invalid hex characters")?;
            Ok((high << 4) | low)
        })
        .collect()
}

pub fn percent_encode(value: &str, form: bool) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        let literal = byte.is_ascii_alphanumeric()
            || if form {
                matches!(byte, b'*' | b'-' | b'.' | b'_')
            } else {
                matches!(byte, b'-' | b'.' | b'_' | b'~')
            };
        if literal {
            output.push(byte as char);
        } else if form && byte == b' ' {
            output.push('+');
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

pub fn percent_decode(value: &str, form: bool) -> Result<String, &'static str> {
    fn nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let malformed = "malformed percent escape";
    let invalid_utf8 = "decoded bytes are not valid UTF-8";
    let input = value.as_bytes();
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%' {
            if index + 2 >= input.len() {
                return Err(malformed);
            }
            let high = nibble(input[index + 1]).ok_or(malformed)?;
            let low = nibble(input[index + 2]).ok_or(malformed)?;
            output.push((high << 4) | low);
            index += 3;
        } else {
            output.push(if form && input[index] == b'+' {
                b' '
            } else {
                input[index]
            });
            index += 1;
        }
    }
    String::from_utf8(output).map_err(|_| invalid_utf8)
}
