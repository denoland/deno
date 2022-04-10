function a() {
  // deno-lint-ignore no-explicit-any
  throw new Error("foo", { cause: new Error("bar", { cause: "deno" as any }) });
}

function b() {
  a();
}

function c() {
  b();
}

c();
