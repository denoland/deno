// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, unitTest } from "./test_util.ts";

unitTest(function globalThisExists() {
  assert(globalThis != null);
});

unitTest(function noInternalGlobals() {
  // globalThis.__bootstrap should not be there.
  for (const key of Object.keys(globalThis)) {
    assert(!key.startsWith("_"));
  }
});

unitTest(function windowExists() {
  assert(window != null);
});

unitTest(function selfExists() {
  assert(self != null);
});

unitTest(function windowWindowExists() {
  assert(window.window === window);
});

unitTest(function windowSelfExists() {
  assert(window.self === window);
});

unitTest(function globalThisEqualsWindow() {
  assert(globalThis === window);
});

unitTest(function globalThisEqualsSelf() {
  assert(globalThis === self);
});

unitTest(function globalThisInstanceofWindow() {
  assert(globalThis instanceof Window);
});

unitTest(function globalThisConstructorLength() {
  assert(globalThis.constructor.length === 0);
});

unitTest(function globalThisInstanceofEventTarget() {
  assert(globalThis instanceof EventTarget);
});

unitTest(function navigatorInstanceofNavigator() {
  // TODO(nayeemrmn): Add `Navigator` to deno_lint globals.
  // deno-lint-ignore no-undef
  assert(navigator instanceof Navigator);
});

unitTest(function DenoNamespaceExists() {
  assert(Deno != null);
});

unitTest(function DenoNamespaceEqualsWindowDeno() {
  assert(Deno === window.Deno);
});

unitTest(function DenoNamespaceIsNotFrozen() {
  assert(!Object.isFrozen(Deno));
});

unitTest(function webAssemblyExists() {
  assert(typeof WebAssembly.compile === "function");
});

declare global {
  namespace Deno {
    // deno-lint-ignore no-explicit-any
    var core: any;
  }
}

unitTest(function DenoNamespaceConfigurable() {
  const desc = Object.getOwnPropertyDescriptor(globalThis, "Deno");
  assert(desc);
  assert(desc.configurable);
  assert(!desc.writable);
});

unitTest(function DenoCoreNamespaceIsImmutable() {
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

unitTest(async function windowQueueMicrotask() {
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
  window.queueMicrotask(resolve1!);
  setTimeout(resolve2!, 0);
  await p1;
  await p2;
});
