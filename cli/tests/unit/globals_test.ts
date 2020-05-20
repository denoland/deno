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
    // @ts-ignore
    Deno = 1;
  } catch {}
  assert(denoCopy === Deno);
  try {
    // @ts-ignore
    window.Deno = 1;
  } catch {}
  assert(denoCopy === Deno);
  try {
    delete window.Deno;
  } catch {}
  assert(denoCopy === Deno);

  const { readFile } = Deno;
  try {
    // @ts-ignore
    Deno.readFile = 1;
  } catch {}
  assert(readFile === Deno.readFile);
  try {
    delete window.Deno.readFile;
  } catch {}
  assert(readFile === Deno.readFile);

  // @ts-ignore
  const { print } = Deno.core;
  try {
    // @ts-ignore
    Deno.core.print = 1;
  } catch {}
  // @ts-ignore
  assert(print === Deno.core.print);
  try {
    // @ts-ignore
    delete Deno.core.print;
  } catch {}
  // @ts-ignore
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
