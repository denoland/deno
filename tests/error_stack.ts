function foo(): never {
  throw new Error("foo");
}

try {
  foo();
} catch (e) {
  console.log(e);
  throw e;
}
