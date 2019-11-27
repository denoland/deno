use deno::*;

#[test]
fn basic() {
  let src = include_str!("test.js");

  let mut isolate = Isolate::new(StartupData::None, false);
  js_check(isolate.execute("test.js", src));
}
