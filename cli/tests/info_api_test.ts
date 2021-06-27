// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../../test_util/std/testing/asserts.ts";

Deno.test({
  name: "Deno.info() - local",
  async fn() {
    const actual = await Deno.info("./subdir/mod1.ts");
    assert(actual.root.endsWith("subdir/mod1.ts"));
    assertEquals(actual.modules.length, 3);
    assert(actual.modules[0].specifier.endsWith("subdir/mod1.ts"));
    assertEquals(actual.modules[0].dependencies!.length, 1);
    assert(actual.modules[1].specifier.endsWith("subdir/print_hello.ts"));
    assertEquals(actual.modules[1].dependencies!.length, 0);
    assert(actual.modules[2].specifier.endsWith("subdir/subdir2/mod2.ts"));
    assertEquals(actual.modules[2].dependencies!.length, 1);
  },
});

Deno.test({
  name: "Deno.info() - remote",
  async fn() {
    const actual = await Deno.info(
      "http://localhost:4545/cli/tests/subdir/mod1.ts",
    );
    assertEquals(actual.root, "http://localhost:4545/cli/tests/subdir/mod1.ts");
    assertEquals(actual.modules.length, 3);
    assertEquals(
      actual.modules[0].specifier,
      "http://localhost:4545/cli/tests/subdir/mod1.ts",
    );
    assertEquals(
      actual.modules[1].specifier,
      "http://localhost:4545/cli/tests/subdir/print_hello.ts",
    );
    assertEquals(
      actual.modules[2].specifier,
      "http://localhost:4545/cli/tests/subdir/subdir2/mod2.ts",
    );
  },
});

Deno.test({
  name: "Deno.info() - import map",
  async fn() {
    const actual = await Deno.info("./import_maps/test.ts", {
      importMap: "./import_maps/import_map.json",
    });
    assertEquals(actual.modules.length, 8);
  },
});

Deno.test({
  name: "Deno.info() - import map remote",
  async fn() {
    const actual = await Deno.info(
      "http://localhost:4545/cli/tests/import_maps/test_remote.ts",
      {
        importMap:
          "http://localhost:4545/cli/tests/import_maps/import_map_remote.json",
      },
    );
    assertEquals(actual.modules.length, 6);
  },
});

Deno.test({
  name: "Deno.info() - import map object",
  async fn() {
    const actual = await Deno.info("./import_maps/test.ts", {
      importMap: {
        imports: {
          "moment": "./tests/import_maps/moment/moment.ts",
          "moment/": "./tests/import_maps/moment/",
          "lodash": "./tests/import_maps/lodash/lodash.ts",
          "lodash/": "./tests/import_maps/lodash/",
          "https://www.unpkg.com/vue/dist/vue.runtime.esm.js":
            "./tests/import_maps/vue.ts",
        },
        scopes: {
          "scope/": {
            "moment": "./tests/import_maps/scoped_moment.ts",
          },
        },
      },
    });
    assertEquals(actual.modules.length, 7);
  },
});

Deno.test({
  name: "Deno.info() - checksums",
  async fn() {
    let actual = await Deno.info(
      "http://localhost:4545/cli/tests/subdir/mod1.ts",
    );
    assertEquals(typeof actual.modules[0].checksum, "undefined");
    assertEquals(typeof actual.modules[0].local, "undefined");

    actual = await Deno.info("http://localhost:4545/cli/tests/subdir/mod1.ts", {
      checksums: true,
    });
    assertEquals(typeof actual.modules[0].checksum, "string");
    assertEquals(typeof actual.modules[0].local, "undefined");
  },
});

Deno.test({
  name: "Deno.info() - paths",
  async fn() {
    let actual = await Deno.info(
      "http://localhost:4545/cli/tests/subdir/mod1.ts",
    );
    assertEquals(typeof actual.modules[0].checksum, "undefined");
    assertEquals(typeof actual.modules[0].local, "undefined");

    actual = await Deno.info("http://localhost:4545/cli/tests/subdir/mod1.ts", {
      paths: true,
    });
    assertEquals(typeof actual.modules[0].checksum, "undefined");
    assertEquals(typeof actual.modules[0].local, "string");
  },
});
