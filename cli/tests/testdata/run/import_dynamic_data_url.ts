// export const a = "a";

// export enum A {
//   A,
//   B,
//   C,
// }
const a = await import(
  "data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo="
);

console.log(a.a);
console.log(a.A);
console.log(a.A.A);
