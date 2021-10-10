if (global != window) {
  throw new Error("global is not equal to window");
}

console.log(process);
console.log(Buffer);
console.log(setImmediate);
console.log(clearImmediate);
