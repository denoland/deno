use std::fmt::Write;

pub fn gen(v: Vec<&[u8]>) -> String {
  let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
  for src in v.iter() {
    ctx.update(src);
  }
  let digest = ctx.finish();
  let mut out = String::new();
  // TODO There must be a better way to do this...
  for byte in digest.as_ref() {
    write!(&mut out, "{:02x}", byte).unwrap();
  }
  out
}
