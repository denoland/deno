const i = setImmediate(() => {});
console.log(i);
clearImmediate(i);
const t = setTimeout(() => {}, 1_000);
console.log(t);
clearTimeout(t);
