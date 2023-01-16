addEventListener("foo", () => {
  throw new Error("bar");
});
console.log(1);
dispatchEvent(new CustomEvent("foo"));
console.log(2);
