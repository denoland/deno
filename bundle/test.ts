// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { test } from "../testing/mod.ts";
import {
  assert,
  AssertionError,
  assertEquals,
  assertThrowsAsync
} from "../testing/asserts.ts";
import { instantiate, load, ModuleMetaData } from "./utils.ts";

/* eslint-disable @typescript-eslint/no-namespace */
declare global {
  namespace globalThis {
    // eslint-disable-next-line no-var
    var __results: [string, string] | undefined;
  }
}
/* eslint-disable max-len */
/* eslint-enable @typescript-eslint/no-namespace */
/*
const fixture = `
define("data", [], { "baz": "qat" });
define("modB", ["require", "exports", "data"], function(require, exports, data) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.foo = "bar";
  exports.baz = data.baz;
});
define("modA", ["require", "exports", "modB"], function(require, exports, modB) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  globalThis.__results = [modB.foo, modB.baz];
});
`;
*/
/* eslint-enable max-len */

const fixtureQueue = ["data", "modB", "modA"];
const fixtureModules = new Map<string, ModuleMetaData>();
fixtureModules.set("data", {
  dependencies: [],
  factory: {
    baz: "qat"
  },
  exports: {}
});
fixtureModules.set("modB", {
  dependencies: ["require", "exports", "data"],
  factory(_require, exports, data): void {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    exports.foo = "bar";
    exports.baz = data.baz;
  },
  exports: {}
});
fixtureModules.set("modA", {
  dependencies: ["require", "exports", "modB"],
  factory(_require, exports, modB): void {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    globalThis.__results = [modB.foo, modB.baz];
  },
  exports: {}
});

test(async function loadBundle(): Promise<void> {
  const result = await load(["", "./bundle/testdata/bundle.js", "--foo"]);
  assert(result != null);
  assert(
    result.includes(
      `define("subdir/print_hello", ["require", "exports"], function(`
    )
  );
});

test(async function loadBadArgs(): Promise<void> {
  await assertThrowsAsync(
    async (): Promise<void> => {
      await load(["bundle/test.ts"]);
    },
    AssertionError,
    "Expected at least two arguments."
  );
});

test(async function loadMissingBundle(): Promise<void> {
  await assertThrowsAsync(
    async (): Promise<void> => {
      await load([".", "bad_bundle.js"]);
    },
    AssertionError,
    `Expected "bad_bundle.js" to exist.`
  );
});

/* TODO re-enable test
test(async function evaluateBundle(): Promise<void> {
  assert(globalThis.define == null, "Expected 'define' to be undefined");
  const [queue, modules] = evaluate(fixture);
  assert(globalThis.define == null, "Expected 'define' to be undefined");
  assertEquals(queue, ["data", "modB", "modA"]);
  assert(modules.has("modA"));
  assert(modules.has("modB"));
  assert(modules.has("data"));
  assertStrictEq(modules.size, 3);
});
*/

test(async function instantiateBundle(): Promise<void> {
  assert(globalThis.__results == null);
  instantiate(fixtureQueue, fixtureModules);
  assertEquals(globalThis.__results, ["bar", "qat"]);
  delete globalThis.__results;
});
