for (let i = 0; i < 10; ++i) {
  console.log(Math.random());
}

const arr = new Uint8Array(32);

crypto.getRandomValues(arr);
console.log(arr);

crypto.getRandomValues(arr);
console.log(arr);
