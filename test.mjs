import vm from "node:vm";
const result = vm.runInThisContext(`global.foo = 1`);
console.log(result);
