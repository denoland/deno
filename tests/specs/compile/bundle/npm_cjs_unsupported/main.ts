import cjsDefault, { named } from "npm:@denotest/cjs-default-export";

console.log("cjs default", cjsDefault());
console.log("cjs named", named());
