globalThis.addEventListener("unhandledrejection", (e) => {
  console.log("unhandled rejection at:", e.promise, "reason:", e.reason);
  e.preventDefault();
});

class Foo {
  constructor() {
    Promise.reject(new Error("bar not available"));
  }
}

new Foo();
Promise.reject();
