//! UUID v4 generation shared by the interpreter and native runtime.

use rand::RngCore;

pub const ENTROPY_UNAVAILABLE: &str = "uuid.new: OS entropy unavailable";

pub fn new_v4() -> Result<String, &'static str> {
    let mut bytes = [0_u8; 16];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| ENTROPY_UNAVAILABLE)?;
    Ok(format_v4(bytes))
}

fn format_v4(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v4_format_sets_version_variant_and_canonical_separators() {
        assert_eq!(
            format_v4([0xff; 16]),
            "ffffffff-ffff-4fff-bfff-ffffffffffff"
        );
        assert_eq!(format_v4([0; 16]), "00000000-0000-4000-8000-000000000000");
    }
}
