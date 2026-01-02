// Copyright 2018-2025 the Deno authors. MIT license.

import { join } from "@std/path";
import { assert, assertEquals, assertRejects } from "./test_util.ts";

Deno.test(function globalThisExists() {
  assert(globalThis != null);
});

Deno.test(function noInternalGlobals() {
  // globalThis.__bootstrap should not be there.
  for (const key of Object.keys(globalThis)) {
    assert(!key.startsWith("_"));
  }
});

Deno.test(function selfExists() {
  assert(self != null);
});

Deno.test(function globalThisWindowEqualsUndefined() {
  assert(globalThis.window === undefined);
});

Deno.test(function globalThisEqualsSelf() {
  assert(globalThis === self);
});

Deno.test(function globalThisConstructorLength() {
  assert(globalThis.constructor.length === 0);
});

Deno.test(function globalThisInstanceofEventTarget() {
  assert(globalThis instanceof EventTarget);
});

Deno.test(function navigatorInstanceofNavigator() {
  // TODO(nayeemrmn): Add `Navigator` to deno_lint globals.
  // deno-lint-ignore no-undef
  assert(navigator instanceof Navigator);
});

Deno.test(function DenoNamespaceExists() {
  assert(Deno != null);
});

Deno.test(function DenoNamespaceIsNotFrozen() {
  assert(!Object.isFrozen(Deno));
});

Deno.test(function webAssemblyExists() {
  assert(typeof WebAssembly.compile === "function");
});

// @ts-ignore This is not publicly typed namespace, but it's there for sure.
const core = Deno[Deno.internal].core;

Deno.test(function DenoNamespaceConfigurable() {
  const desc = Object.getOwnPropertyDescriptor(globalThis, "Deno");
  assert(desc);
  assert(desc.configurable);
  assert(!desc.writable);
});

Deno.test(function DenoCoreNamespaceIsImmutable() {
  const { print } = core;
  try {
    core.print = 1;
  } catch {
    // pass
  }
  assert(print === core.print);
  try {
    delete core.print;
  } catch {
    // pass
  }
  assert(print === core.print);
});

Deno.test(async function windowQueueMicrotask() {
  let resolve1: () => void | undefined;
  let resolve2: () => void | undefined;
  let microtaskDone = false;
  const p1 = new Promise<void>((res) => {
    resolve1 = () => {
      microtaskDone = true;
      res();
    };
  });
  const p2 = new Promise<void>((res) => {
    resolve2 = () => {
      assert(microtaskDone);
      res();
    };
  });
  globalThis.queueMicrotask(resolve1!);
  setTimeout(resolve2!, 0);
  await p1;
  await p2;
});

Deno.test(function webApiGlobalThis() {
  assert(globalThis.FormData !== null);
  assert(globalThis.TextEncoder !== null);
  assert(globalThis.TextEncoderStream !== null);
  assert(globalThis.TextDecoder !== null);
  assert(globalThis.TextDecoderStream !== null);
  assert(globalThis.CountQueuingStrategy !== null);
  assert(globalThis.ByteLengthQueuingStrategy !== null);
});

Deno.test(function windowNameIsDefined() {
  assertEquals(typeof globalThis.name, "string");
  assertEquals(name, "");
  name = "foobar";
  assertEquals(name, "foobar");
  name = "";
  assertEquals(name, "");
});

Deno.test(async function promiseWithResolvers() {
  {
    const { promise, resolve } = Promise.withResolvers();
    resolve(true);
    assert(await promise);
  }
  {
    const { promise, reject } = Promise.withResolvers();
    reject(new Error("boom!"));
    await assertRejects(() => promise, Error, "boom!");
  }
});

Deno.test(async function arrayFromAsync() {
  // Taken from https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/fromAsync#examples
  // Thank you.
  const asyncIterable = (async function* () {
    for (let i = 0; i < 5; i++) {
      await new Promise((resolve) => setTimeout(resolve, 10 * i));
      yield i;
    }
  })();

  const a = await Array.fromAsync(asyncIterable);
  assertEquals(a, [0, 1, 2, 3, 4]);

  const b = await Array.fromAsync(new Map([[1, 2], [3, 4]]));
  assertEquals(b, [[1, 2], [3, 4]]);
});

// Taken from https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Object/groupBy#examples
Deno.test(function objectGroupBy() {
  const inventory = [
    { name: "asparagus", type: "vegetables", quantity: 5 },
    { name: "bananas", type: "fruit", quantity: 0 },
    { name: "goat", type: "meat", quantity: 23 },
    { name: "cherries", type: "fruit", quantity: 5 },
    { name: "fish", type: "meat", quantity: 22 },
  ];
  const result = Object.groupBy(inventory, ({ type }) => type);
  assertEquals(result, {
    vegetables: [
      { name: "asparagus", type: "vegetables", quantity: 5 },
    ],
    fruit: [
      { name: "bananas", type: "fruit", quantity: 0 },
      { name: "cherries", type: "fruit", quantity: 5 },
    ],
    meat: [
      { name: "goat", type: "meat", quantity: 23 },
      { name: "fish", type: "meat", quantity: 22 },
    ],
  });
});

Deno.test(function objectGroupByEmpty() {
  const empty: string[] = [];
  const result = Object.groupBy(empty, () => "abc");
  assertEquals(result.abc, undefined);
});

// Taken from https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Map/groupBy#examples
Deno.test(function mapGroupBy() {
  const inventory = [
    { name: "asparagus", type: "vegetables", quantity: 9 },
    { name: "bananas", type: "fruit", quantity: 5 },
    { name: "goat", type: "meat", quantity: 23 },
    { name: "cherries", type: "fruit", quantity: 12 },
    { name: "fish", type: "meat", quantity: 22 },
  ];
  const restock = { restock: true };
  const sufficient = { restock: false };
  const result = Map.groupBy(
    inventory,
    ({ quantity }) => quantity < 6 ? restock : sufficient,
  );
  assertEquals(result.get(restock), [{
    name: "bananas",
    type: "fruit",
    quantity: 5,
  }]);
});

// Regression test for https://github.com/denoland/deno/issues/30012
Deno.test(function globalGlobalIsWritable() {
  // @ts-ignore the typings here are wrong
  globalThis.global = "can write to `global`";
  // @ts-ignore the typings here are wrong
  globalThis.global = globalThis;
});

Deno.test(async function overwriteEventOnExternalModuleShouldNotCrash() {
  const tmpDir = await Deno.makeTempDir();

  const externalModulePath = join(tmpDir, "overwrite_event.ts");
  const externalModuleContent =
    `globalThis.Event = class {}; export default {};`;
  await Deno.writeTextFile(externalModulePath, externalModuleContent);

  const entrypointPath = join(tmpDir, "index.ts");
  const entrypointContent = `import("./overwrite_event.ts");`;
  await Deno.writeTextFile(entrypointPath, entrypointContent);

  const command = new Deno.Command(Deno.execPath(), {
    args: ["run", "-A", entrypointPath],
    stdout: "null",
    stderr: "null",
  });
  const child = command.spawn();

  const status = await child.status;
  assert(status.success);
  await Deno.remove(tmpDir, { recursive: true });
});
