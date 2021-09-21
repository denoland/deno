import { assertEquals, assertThrows } from "./test_util.ts";

Deno.test({
  name: "chdirSymlinkDotDotPermissionDenied",
  fn() {
    assertThrows(() => {
      Deno.chdir("cli/tests/unit/testdata/symlink_to_dot_dot");
    }, Deno.errors.PermissionDenied);
  },
  permissions: {
    read: ["cli/tests/unit/testdata"],
  },
});
