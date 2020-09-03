pub fn gen(v: &[&[u8]]) -> String {
  let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
  for src in v {
    ctx.update(src);
  }
  let digest = ctx.finish();
  let out: Vec<String> = digest
    .as_ref()
    .iter()
    .map(|byte| format!("{:02x}", byte))
    .collect();
  out.join("")
}
