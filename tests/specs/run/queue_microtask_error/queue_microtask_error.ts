queueMicrotask(() => {
  throw new Error("foo");
});
console.log(1);
Promise.resolve().then(() => console.log(2));
