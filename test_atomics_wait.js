setTimeout(() => console.log("timeout", new Date()), 5000);

const sab = new SharedArrayBuffer(4);
const int32 = new Int32Array(sab);
Atomics.waitAsync(int32, 0, 0, 50).value.then(() =>
  console.log("waitAsync", new Date())
);

console.log(new Date());
