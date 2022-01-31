function a() {
  throw new Error("foo", { cause: new Error("bar", { cause: "deno" }) });
}

function b() {
  a();
}

function c() {
  b();
}

c();
