// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert } from "./test_util.ts";

Deno.test("globalThisExists", function (): void {
  assert(globalThis != null);
});

Deno.test("noInternalGlobals", function (): void {
  // globalThis.__bootstrap should not be there.
  for (const key of Object.keys(globalThis)) {
    assert(!key.startsWith("_"));
  }
});

Deno.test("windowExists", function (): void {
  assert(window != null);
});

Deno.test("selfExists", function (): void {
  assert(self != null);
});

Deno.test("windowWindowExists", function (): void {
  assert(window.window === window);
});

Deno.test("windowSelfExists", function (): void {
  assert(window.self === window);
});

Deno.test("globalThisEqualsWindow", function (): void {
  assert(globalThis === window);
});

Deno.test("globalThisEqualsSelf", function (): void {
  assert(globalThis === self);
});

Deno.test("globalThisInstanceofWindow", function (): void {
  assert(globalThis instanceof Window);
});

Deno.test("globalThisConstructorLength", function (): void {
  assert(globalThis.constructor.length === 0);
});

Deno.test("globalThisInstanceofEventTarget", function (): void {
  assert(globalThis instanceof EventTarget);
});

Deno.test("navigatorInstanceofNavigator", function (): void {
  // TODO(nayeemrmn): Add `Navigator` to deno_lint globals.
  // deno-lint-ignore no-undef
  assert(navigator instanceof Navigator);
});

Deno.test("DenoNamespaceExists", function (): void {
  assert(Deno != null);
});

Deno.test("DenoNamespaceEqualsWindowDeno", function (): void {
  assert(Deno === window.Deno);
});

Deno.test("DenoNamespaceIsFrozen", function (): void {
  assert(Object.isFrozen(Deno));
});

Deno.test("webAssemblyExists", function (): void {
  assert(typeof WebAssembly.compile === "function");
});

declare global {
  namespace Deno {
    // deno-lint-ignore no-explicit-any
    var core: any;
  }
}

Deno.test("DenoNamespaceImmutable", function (): void {
  const denoCopy = window.Deno;
  try {
    // deno-lint-ignore no-explicit-any
    (Deno as any) = 1;
  } catch {
    // pass
  }
  assert(denoCopy === Deno);
  try {
    // deno-lint-ignore no-explicit-any
    (window as any).Deno = 1;
  } catch {
    // pass
  }
  assert(denoCopy === Deno);
  try {
    // deno-lint-ignore no-explicit-any
    delete (window as any).Deno;
  } catch {
    // pass
  }
  assert(denoCopy === Deno);

  const { readFile } = Deno;
  try {
    // deno-lint-ignore no-explicit-any
    (Deno as any).readFile = 1;
  } catch {
    // pass
  }
  assert(readFile === Deno.readFile);
  try {
    // deno-lint-ignore no-explicit-any
    delete (window as any).Deno.readFile;
  } catch {
    // pass
  }
  assert(readFile === Deno.readFile);

  const { print } = Deno.core;
  try {
    Deno.core.print = 1;
  } catch {
    // pass
  }
  assert(print === Deno.core.print);
  try {
    delete Deno.core.print;
  } catch {
    // pass
  }
  assert(print === Deno.core.print);
});

Deno.test("windowQueueMicrotask", async function (): Promise<void> {
  let resolve1: () => void | undefined;
  let resolve2: () => void | undefined;
  let microtaskDone = false;
  const p1 = new Promise<void>((res): void => {
    resolve1 = (): void => {
      microtaskDone = true;
      res();
    };
  });
  const p2 = new Promise<void>((res): void => {
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
