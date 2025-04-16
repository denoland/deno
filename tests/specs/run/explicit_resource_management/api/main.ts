import console from "node:console";

console.log(new SuppressedError("asd", new Error("foo")) instanceof Error);
console.log(new DisposableStack());
console.log(new AsyncDisposableStack());
console.log(Iterator.from([])[Symbol.dispose]);
