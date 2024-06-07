import vm from "node:vm";
const script = new vm.Script("import('node:process')");
console.log(await script.runInNewContext());
