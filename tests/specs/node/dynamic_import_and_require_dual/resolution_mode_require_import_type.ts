// deno-fmt-ignore-file
type Value = typeof import("package", { with: { 'resolution-mode': 'require' } }).kind;

const value: Value = "value";
console.log(value);