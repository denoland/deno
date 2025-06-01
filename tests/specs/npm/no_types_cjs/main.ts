import mod from "npm:@denotest/no-types-cjs";

// it actually returns a `number` and has that in its
// jsdocs, but the jsdocs should not have been resolved so
// this should type check just fine
const value: string = mod();
console.log(value);
