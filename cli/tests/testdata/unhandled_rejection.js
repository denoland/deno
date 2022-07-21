globalThis.addEventListener("unhandledrejection", (e) => {
  console.log("unhandled rejection at:", e.promise, "reason:", e.reason);
  e.preventDefault();
});

function Foo() {
  this.bar = Promise.reject(new Error("bar not available"));
}

new Foo();
Promise.reject();
