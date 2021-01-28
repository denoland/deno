import "./global.ts";
import { assert, assertStrictEquals } from "../testing/asserts.ts";
import { Buffer as BufferModule } from "./buffer.ts";
import processModule from "./process.ts";
import timers from "./timers.ts";

// Definitions for this are quite delicate
// This ensures modifications to the global namespace don't break on TypeScript

// TODO(bartlomieju):
// Deno lint marks globals defined by this module as undefined
// probably gonna change in the future

Deno.test("global is correctly defined", () => {
  // deno-lint-ignore no-undef
  assertStrictEquals(global, globalThis);
  // deno-lint-ignore no-undef
  assertStrictEquals(global.Buffer, BufferModule);
  // deno-lint-ignore no-undef
  assertStrictEquals(global.process, process);
});

Deno.test("Buffer is correctly defined", () => {
  //Check that Buffer is defined as a type as well
  type x = Buffer;
  // deno-lint-ignore no-undef
  assertStrictEquals(Buffer, BufferModule);
  // deno-lint-ignore no-undef
  assert(Buffer.from);
  // deno-lint-ignore no-undef
  assertStrictEquals(global.Buffer, BufferModule);
  // deno-lint-ignore no-undef
  assert(global.Buffer.from);
  assertStrictEquals(globalThis.Buffer, BufferModule);
  assert(globalThis.Buffer.from);
  assertStrictEquals(window.Buffer, BufferModule);
  assert(window.Buffer.from);
});

Deno.test("process is correctly defined", () => {
  // deno-lint-ignore no-undef
  assertStrictEquals(process, processModule);
  // deno-lint-ignore no-undef
  assert(process.arch);
  // deno-lint-ignore no-undef
  assertStrictEquals(global.process, processModule);
  // deno-lint-ignore no-undef
  assert(global.process.arch);
  assertStrictEquals(globalThis.process, processModule);
  assert(globalThis.process.arch);
  assertStrictEquals(window.process, processModule);
  assert(window.process.arch);
});

Deno.test("setImmediate is correctly defined", () => {
  // deno-lint-ignore no-undef
  assertStrictEquals(setImmediate, timers.setImmediate);
  // deno-lint-ignore no-undef
  assertStrictEquals(global.setImmediate, timers.setImmediate);
  assertStrictEquals(globalThis.setImmediate, timers.setImmediate);
  assertStrictEquals(window.setImmediate, timers.setImmediate);
});

Deno.test("clearImmediate is correctly defined", () => {
  // deno-lint-ignore no-undef
  assertStrictEquals(clearImmediate, timers.clearImmediate);
  // deno-lint-ignore no-undef
  assertStrictEquals(global.clearImmediate, timers.clearImmediate);
  assertStrictEquals(globalThis.clearImmediate, timers.clearImmediate);
  assertStrictEquals(window.clearImmediate, timers.clearImmediate);
});
