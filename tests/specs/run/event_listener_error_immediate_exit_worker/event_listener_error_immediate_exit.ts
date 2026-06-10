addEventListener("foo", () => {
  queueMicrotask(() => console.log("queueMicrotask"));
  setTimeout(() => console.log("timer"), 0);
  throw new Error("bar");
});
console.log(1);
process.nextTick(() => console.log("nextTick"));
dispatchEvent(new CustomEvent("foo"));
console.log(2);
