// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert } from "./test_util.ts";

unitTest(function globalThisExists(): void {
  assert(globalThis != null);
});

unitTest(function windowExists(): void {
  assert(window != null);
});

unitTest(function selfExists(): void {
  assert(self != null);
});

unitTest(function windowWindowExists(): void {
  assert(window.window === window);
});

unitTest(function windowSelfExists(): void {
  assert(window.self === window);
});

unitTest(function globalThisEqualsWindow(): void {
  assert(globalThis === window);
});

unitTest(function globalThisEqualsSelf(): void {
  assert(globalThis === self);
});

unitTest(function DenoNamespaceExists(): void {
  assert(Deno != null);
});

unitTest(function DenoNamespaceEqualsWindowDeno(): void {
  assert(Deno === window.Deno);
});

unitTest(function DenoNamespaceIsFrozen(): void {
  assert(Object.isFrozen(Deno));
});

unitTest(function webAssemblyExists(): void {
  assert(typeof WebAssembly.compile === "function");
});

unitTest(function DenoNamespaceImmutable(): void {
  const denoCopy = window.Deno;
  try {
    // @ts-expect-error
    Deno = 1;
  } catch {}
  assert(denoCopy === Deno);
  try {
    // @ts-expect-error
    window.Deno = 1;
  } catch {}
  assert(denoCopy === Deno);
  try {
    delete window.Deno;
  } catch {}
  assert(denoCopy === Deno);

  const { readFile } = Deno;
  try {
    // @ts-expect-error
    Deno.readFile = 1;
  } catch {}
  assert(readFile === Deno.readFile);
  try {
    delete window.Deno.readFile;
  } catch {}
  assert(readFile === Deno.readFile);

  // @ts-expect-error
  const { print } = Deno.core;
  try {
    // @ts-expect-error
    Deno.core.print = 1;
  } catch {}
  // @ts-expect-error
  assert(print === Deno.core.print);
  try {
    // @ts-expect-error
    delete Deno.core.print;
  } catch {}
  // @ts-expect-error
  assert(print === Deno.core.print);
});

unitTest(async function windowQueueMicrotask(): Promise<void> {
  let resolve1: () => void | undefined;
  let resolve2: () => void | undefined;
  let microtaskDone = false;
  const p1 = new Promise((res): void => {
    resolve1 = (): void => {
      microtaskDone = true;
      res();
    };
  });
  const p2 = new Promise((res): void => {
    resolve2 = (): void => {
      assert(microtaskDone);
      res();
    };
  });
  window.queueMicrotask(resolve1!);
  setTimeout(resolve2!, 0);
  await p1;
  await p2;
});
