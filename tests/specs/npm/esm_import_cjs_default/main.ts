// @ts-check
import cjsDefault, {
  MyClass as MyCjsClass,
} from "npm:@denotest/cjs-default-export";
import * as cjsNamespace from "npm:@denotest/cjs-default-export";
import esmDefault from "npm:@denotest/esm-import-cjs-default";
import * as esmNamespace from "npm:@denotest/esm-import-cjs-default";

console.log("Deno esm importing node cjs");
console.log("===========================");
console.log(cjsDefault);
console.log(cjsNamespace);
console.log("===========================");

console.log("Deno esm importing node esm");
console.log("===========================");
console.log(esmDefault);
console.log(esmNamespace);
console.log("===========================");

console.log(cjsDefault.default());
console.log(esmDefault());
console.log(MyCjsClass.someStaticMethod());
