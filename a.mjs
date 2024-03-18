import vm from "node:vm";

globalThis.globalVar = 3;

const context = { globalVar: 1 };
vm.createContext(context);

vm.runInContext("globalVar *= 2;", context);

console.log(context);

console.log(global.globalVar);
