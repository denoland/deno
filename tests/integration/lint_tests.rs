// Copyright 2018-2026 the Deno authors. MIT license.

use test_util as util;
use util::TestContextBuilder;

#[test]
fn lint_fix_preserves_permissions() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir().path();
  let file_path = temp_dir.join("a.ts");

  file_path.write("window.foo = 1;\n");

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&file_path, perms).unwrap();
  }

  let output = context.new_command().args("lint --fix a.ts").run();

  output.assert_exit_code(0);

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::metadata(&file_path).unwrap().permissions();
    assert_eq!(perms.mode() & 0o777, 0o755);
  }
}
