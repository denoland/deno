pub fn detect_charset(bytes: &Vec<u8>) -> &str {
  const UTF8_BOM: &'static [u8] = b"\xEF\xBB\xBF";
  const UTF16_LE_BOM: &'static [u8] = b"\xFF\xFE";
  const UTF16_BE_BOM: &'static [u8] = b"\xFE\xFF";

  if bytes.starts_with(UTF8_BOM) {
    "utf-8"
  } else if bytes.starts_with(UTF16_LE_BOM) {
    "utf-16le"
  } else if bytes.starts_with(UTF16_BE_BOM) {
    "utf-16be"
  } else {
    // Assume everything else is utf-8
    "utf-8"
  }
}

pub fn to_utf8(
  bytes: &Vec<u8>,
  charset: &str,
) -> Result<String, std::io::Error> {
  match encoding::label::encoding_from_whatwg_label(charset) {
    Some(coder) => match coder.decode(bytes, encoding::DecoderTrap::Ignore) {
      Ok(text) => Ok(text),
      Err(_e) => Err(std::io::ErrorKind::InvalidData.into()),
    },
    None => Err(std::io::ErrorKind::InvalidData.into()),
  }
}
