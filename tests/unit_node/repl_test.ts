// deno-lint-ignore-file no-undef
// Copyright 2018-2026 the Deno authors. MIT license.

import repl from "node:repl";
import { PassThrough } from "node:stream";
import { Buffer } from "node:buffer";
import { assert, assertEquals } from "@std/assert";

Deno.test({
  name: "repl module exports",
  fn() {
    assert(typeof repl.REPLServer !== "undefined");
    assert(typeof repl.start !== "undefined");
    // @ts-ignore not present in declaration files, but libraries depend on it
    assert(typeof repl.builtinModules !== "undefined");
    // @ts-ignore not present in declaration files, but libraries depend on it
    assert(typeof repl._builtinLibs !== "undefined");
  },
});

// Regression test for https://github.com/denoland/deno/issues/34360.
// A custom `eval` paired with `preview: true` (as used by `@babel/node`)
// previously evaluated the typed input via `vm.Script` on every keystroke,
// so closing a paren around a side-effectful expression would fire the
// side effect mid-type. The preview path now runs through the V8
// inspector's `Runtime.evaluate({ throwOnSideEffect: true })` (see
// `_createPreviewSession` in ext/node/polyfills/repl.ts), which catches
// side-effectful expressions before they execute.
Deno.test({
  name: "node:repl custom eval + preview:true does not run input mid-keystroke",
  async fn() {
    // deno-lint-ignore no-explicit-any
    const input = new PassThrough() as any;
    // deno-lint-ignore no-explicit-any
    const output = new PassThrough() as any;
    input.isTTY = true;
    output.isTTY = true;
    output.columns = 80;
    output.rows = 24;
    output.on("data", () => {});

    // deno-lint-ignore no-explicit-any
    (globalThis as any).__replTestSideEffect = 0;

    let evalCount = 0;
    const server = repl.start({
      input,
      output,
      prompt: "> ",
      terminal: true,
      preview: true,
      useGlobal: true,
      useColors: false,
      // deno-lint-ignore no-explicit-any
      eval: (_code: string, _ctx: any, _file: string, cb: any) => {
        evalCount++;
        cb(null);
      },
    });

    try {
      await new Promise((r) => setTimeout(r, 50));

      // Typing a complete expression that bumps a counter would, with the
      // bug, trip the preview's `vm.Script` evaluation when `)` is typed
      // and increment the counter before Enter.
      input.write("(globalThis.__replTestSideEffect++, 0)");
      await new Promise((r) => setTimeout(r, 150));

      assertEquals(
        // deno-lint-ignore no-explicit-any
        (globalThis as any).__replTestSideEffect,
        0,
        "preview must not execute user input mid-keystroke",
      );
      assertEquals(evalCount, 0, "custom eval must not run before Enter");

      input.write("\r");
      await new Promise((r) => setTimeout(r, 150));

      assertEquals(evalCount, 1, "custom eval runs exactly once per Enter");
      assertEquals(
        // deno-lint-ignore no-explicit-any
        (globalThis as any).__replTestSideEffect,
        0,
        "custom eval is a no-op, so the counter stays at 0",
      );
    } finally {
      server.close();
      // deno-lint-ignore no-explicit-any
      delete (globalThis as any).__replTestSideEffect;
    }
  },
});

// Follow-up to #34360: with the inspector-backed preview probe in place,
// `preview: true` opt-in actually shows a preview for side-effect-free
// expressions (matching Node). Custom-eval REPLs no longer silently
// drop the explicit opt-in.
Deno.test({
  name: "node:repl pure expression preview renders when preview:true",
  async fn() {
    // deno-lint-ignore no-explicit-any
    const input = new PassThrough() as any;
    // deno-lint-ignore no-explicit-any
    const output = new PassThrough() as any;
    input.isTTY = true;
    output.isTTY = true;
    output.columns = 80;
    output.rows = 24;
    let collected = "";
    output.on("data", (chunk: Buffer) => {
      collected += chunk.toString("utf8");
    });

    const server = repl.start({
      input,
      output,
      prompt: "> ",
      terminal: true,
      preview: true,
      useGlobal: true,
      useColors: false,
      // deno-lint-ignore no-explicit-any
      eval: (_code: string, _ctx: any, _file: string, cb: any) => cb(null),
    });

    try {
      await new Promise((r) => setTimeout(r, 50));
      collected = "";
      input.write("'hello'+' world'");
      await new Promise((r) => setTimeout(r, 200));

      assert(
        collected.includes("hello world"),
        `expected preview to render 'hello world', got: ${
          JSON.stringify(collected)
        }`,
      );
    } finally {
      server.close();
    }
  },
});

// Follow-up to #34360: matches Node's default. Custom eval without an
// explicit `preview` opt-in must NOT show a preview, even though the
// inspector probe is wired up.
Deno.test({
  name: "node:repl custom eval defaults to no preview",
  async fn() {
    // deno-lint-ignore no-explicit-any
    const input = new PassThrough() as any;
    // deno-lint-ignore no-explicit-any
    const output = new PassThrough() as any;
    input.isTTY = true;
    output.isTTY = true;
    output.columns = 80;
    output.rows = 24;
    let collected = "";
    output.on("data", (chunk: Buffer) => {
      collected += chunk.toString("utf8");
    });

    const server = repl.start({
      input,
      output,
      prompt: "> ",
      terminal: true,
      useGlobal: true,
      useColors: false,
      // deno-lint-ignore no-explicit-any
      eval: (_code: string, _ctx: any, _file: string, cb: any) => cb(null),
    });

    try {
      await new Promise((r) => setTimeout(r, 50));
      collected = "";
      input.write("'hello'+' world'");
      await new Promise((r) => setTimeout(r, 200));

      assert(
        !collected.includes("hello world"),
        `expected no preview, but output contained 'hello world': ${
          JSON.stringify(collected)
        }`,
      );
    } finally {
      server.close();
    }
  },
});
