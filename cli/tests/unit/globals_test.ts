// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file no-window-prefix
import { assert } from "./test_util.ts";

Deno.test(function globalThisExists() {
  assert(globalThis != null);
});

Deno.test(function noInternalGlobals() {
  // globalThis.__bootstrap should not be there.
  for (const key of Object.keys(globalThis)) {
    assert(!key.startsWith("_"));
  }
});

Deno.test(function windowExists() {
  assert(window != null);
});

Deno.test(function selfExists() {
  assert(self != null);
});

Deno.test(function windowWindowExists() {
  assert(window.window === window);
});

Deno.test(function windowSelfExists() {
  assert(window.self === window);
});

Deno.test(function globalThisEqualsWindow() {
  assert(globalThis === window);
});

Deno.test(function globalThisEqualsSelf() {
  assert(globalThis === self);
});

Deno.test(function globalThisInstanceofWindow() {
  assert(globalThis instanceof Window);
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

Deno.test(function DenoNamespaceEqualsWindowDeno() {
  assert(Deno === window.Deno);
});

Deno.test(function DenoNamespaceIsNotFrozen() {
  assert(!Object.isFrozen(Deno));
});

Deno.test(function webAssemblyExists() {
  assert(typeof WebAssembly.compile === "function");
});

declare global {
  namespace Deno {
    // deno-lint-ignore no-explicit-any, no-var
    var core: any;
  }
}

Deno.test(function DenoNamespaceConfigurable() {
  const desc = Object.getOwnPropertyDescriptor(globalThis, "Deno");
  assert(desc);
  assert(desc.configurable);
  assert(!desc.writable);
});

Deno.test(function DenoCoreNamespaceIsImmutable() {
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
  window.queueMicrotask(resolve1!);
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
