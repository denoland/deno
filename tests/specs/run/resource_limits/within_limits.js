// A small, quick program that stays comfortably within all limits.
const arr = new Array(1000).fill(0).map((_, i) => i);
const sum = arr.reduce((a, b) => a + b, 0);
console.log("sum:", sum);
