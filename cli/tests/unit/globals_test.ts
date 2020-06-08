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

/* eslint-disable @typescript-eslint/no-namespace, @typescript-eslint/no-explicit-any,no-var */
declare global {
  namespace Deno {
    var core: any;
  }
}
/* eslint-enable */

unitTest(function DenoNamespaceImmutable(): void {
  const denoCopy = window.Deno;
  try {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (Deno as any) = 1;
  } catch {}
  assert(denoCopy === Deno);
  try {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).Deno = 1;
  } catch {}
  assert(denoCopy === Deno);
  try {
    delete window.Deno;
  } catch {}
  assert(denoCopy === Deno);

  const { readFile } = Deno;
  try {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (Deno as any).readFile = 1;
  } catch {}
  assert(readFile === Deno.readFile);
  try {
    delete window.Deno.readFile;
  } catch {}
  assert(readFile === Deno.readFile);

  const { print } = Deno.core;
  try {
    Deno.core.print = 1;
  } catch {}
  assert(print === Deno.core.print);
  try {
    delete Deno.core.print;
  } catch {}
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
