globalThis.addEventListener("unhandledrejection", () => {
  console.log("hey");
});
console.log("---");
Promise.reject();
