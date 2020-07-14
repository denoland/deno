pub fn source_to_string(data: &Vec<u8>) -> Result<String, std::io::Error> {
  let result = chardet::detect(&data);
  let coder = encoding::label::encoding_from_whatwg_label(
    chardet::charset2encoding(&result.0),
  );
  if coder.is_some() {
    let utf8reader = coder
      .unwrap()
      .decode(&data, encoding::DecoderTrap::Ignore)
      .expect("Error");
    Ok(utf8reader.to_string())
  } else {
    Err(std::io::ErrorKind::InvalidData.into())
  }
}
