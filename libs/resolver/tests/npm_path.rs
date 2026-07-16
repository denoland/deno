// Copyright 2018-2026 the Deno authors. MIT license.

use deno_resolver::npm::package_name_for_node_modules_path_parts;

#[test]
fn package_name_for_node_modules_path_parts_validates_path_shape() {
  assert_eq!(
    package_name_for_node_modules_path_parts("package").unwrap(),
    vec!["package"]
  );
  assert_eq!(
    package_name_for_node_modules_path_parts("@scope/package").unwrap(),
    vec!["@scope", "package"]
  );

  for valid in [
    "package-name",
    "legacy.Name_1",
    "@scope-name/package.name",
    "@types/node",
    "@jsr/std__bytes",
  ] {
    assert!(
      package_name_for_node_modules_path_parts(valid).is_ok(),
      "{valid:?} should be accepted"
    );
  }

  for invalid in [
    "",
    ".",
    "..",
    "./package",
    "../package",
    "package/.",
    "package/..",
    "package/",
    "package//child",
    "package/subpath",
    "@scope",
    "@/package",
    "@scope/",
    "@scope/package/subpath",
    "/package",
    "C:\\package",
    "C:package",
    "package\\subpath",
    "\\\\server\\share",
    ". ",
    ".. ",
    "...",
    "package.",
    "package ",
    "package. ",
    "@scope./package",
    "@scope /package",
    "@scope/package.",
    "@scope/package ",
  ] {
    assert!(
      package_name_for_node_modules_path_parts(invalid).is_err(),
      "{invalid:?} should be rejected"
    );
  }
}
