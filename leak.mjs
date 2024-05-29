import vm from "node:vm";

const script = new vm.Script("returnValue = 2+2");
console.log("Running script 1000 times");

for (let i = 0; i < 1000; i++) {
  script.runInNewContext({}, { timeout: 10000 });
}
