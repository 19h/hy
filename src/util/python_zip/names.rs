//! ZIP's historical CP437 encoding and Python's filename sanitation.

use zip::result::{ZipError, ZipResult};

static CP437: std::sync::LazyLock<Vec<char>> =
    std::sync::LazyLock::new(|| CP437_HIGH.chars().collect());

const CP437_HIGH: &str = concat!(
    "ÇüéâäàåçêëèïîìÄÅ",
    "ÉæÆôöòûùÿÖÜ¢£¥₧ƒ",
    "áíóúñÑªº¿⌐¬½¼¡«»",
    "░▒▓│┤╡╢╖╕╣║╗╝╜╛┐",
    "└┴┬├─┼╞╟╚╔╩╦╠═╬╧",
    "╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀",
    "αßΓπΣσµτΦΘΩδ∞φε∩",
    "≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}",
);

pub(super) fn decode(bytes: &[u8], flags: u16) -> ZipResult<String> {
    if flags & 0x800 != 0 {
        return String::from_utf8(bytes.to_vec()).map_err(|error| {
            // A filename UnicodeDecodeError is terminal, not Python's BadZipFile.
            ZipError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
        });
    }
    Ok(bytes
        .iter()
        .map(|&byte| {
            if byte < 128 {
                char::from(byte)
            } else {
                CP437[usize::from(byte - 128)]
            }
        })
        .collect())
}

pub(super) fn sanitize(name: &str) -> String {
    let name = name.split('\0').next().expect("split yields one item");
    if cfg!(windows) {
        name.replace('\\', "/")
    } else {
        name.to_owned()
    }
}
