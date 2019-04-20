// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function globalThisExists() {
  assert(globalThis != null);
});

test(function windowExists() {
  assert(window != null);
});

test(function windowWindowExists() {
  assert(window.window === window);
});

test(function globalThisEqualsWindow() {
  // @ts-ignore (TypeScript thinks globalThis and window don't match)
  assert(globalThis === window);
});

test(function DenoNamespaceExists() {
  assert(Deno != null);
});

test(function DenoNamespaceEqualsWindowDeno() {
  assert(Deno === window.Deno);
});

test(function DenoNamespaceIsFrozen() {
  assert(Object.isFrozen(Deno));
});

test(function webAssemblyExists() {
  assert(typeof WebAssembly.compile === "function");
});

test(function DenoNamespaceImmutable() {
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
