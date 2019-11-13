function assertEquals(actual, expected, msg) {
  if (actual !== expected) {
    throw new Error(msg || "");
  }
}

export function jsFn() {
  state = "WASM JS Function Executed";
  return 42;
}

export let state = "JS Function Executed";

export function jsInitFn() {
  assertEquals(state, "JS Function Executed", "Incorrect state");
  state = "WASM Start Executed";
}
