import { assertEquals } from "../../../../test_util/std/testing/asserts.ts";

const state: string[] = [];

Deno.test.before(function () {
  assertEquals(state, []);
  state.push("before 1");
});

Deno.test.before(function () {
  assertEquals(state, ["before 1"]);
  state.push("before 2");
});

Deno.test.before(function () {
  assertEquals(state, ["before 1", "before 2"]);
  state.push("before 3");
});

Deno.test("test 1", function () {
  assertEquals(state, ["before 1", "before 2", "before 3"]);
  state.push("test 1");
});

Deno.test("test 2", function () {
  assertEquals(state, ["before 1", "before 2", "before 3", "test 1"]);
  state.push("test 2");
});

Deno.test("test 3", function () {
  assertEquals(state, ["before 1", "before 2", "before 3", "test 1", "test 2"]);
  state.push("test 3");
});

Deno.test.after(function () {
  assertEquals(state, [
    "before 1",
    "before 2",
    "before 3",
    "test 1",
    "test 2",
    "test 3",
  ]);
  state.push("after 1");
});

Deno.test.after(function () {
  assertEquals(state, [
    "before 1",
    "before 2",
    "before 3",
    "test 1",
    "test 2",
    "test 3",
    "after 1",
  ]);
  state.push("after 2");
});

Deno.test.after(function () {
  assertEquals(state, [
    "before 1",
    "before 2",
    "before 3",
    "test 1",
    "test 2",
    "test 3",
    "after 1",
    "after 2",
  ]);
  state.push("after 3");
});
