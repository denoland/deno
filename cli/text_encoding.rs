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

#[cfg(test)]
mod tests {
  use super::*;

  fn test_detection(test_data: &Vec<u8>, expected_charset: &str) {
    let detected_charset = detect_charset(&test_data);
    assert_eq!(
      expected_charset.to_lowercase(),
      detected_charset.to_lowercase()
    );
  }

  #[test]
  fn test_detection_utf8_no_bom() {
    let test_data = "Hello UTF-8 it is \u{23F0} for Deno!"
      .to_owned()
      .into_bytes();
    test_detection(&test_data, "utf-8");
  }

  #[test]
  fn test_detection_utf16_little_endian() {
    let test_data = b"\xFF\xFEHello UTF-16LE".to_owned().to_vec();
    test_detection(&test_data, "utf-16le");
  }

  #[test]
  fn test_detection_utf16_big_endian() {
    let test_data = b"\xFE\xFFHello UTF-16BE".to_owned().to_vec();
    test_detection(&test_data, "utf-16be");
  }
}
