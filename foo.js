import swc from "npm:@swc/core";
console.log(await swc.transform("export {}"));
