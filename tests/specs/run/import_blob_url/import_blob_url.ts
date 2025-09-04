const blob = new Blob(
  ['export const a = "a";\n\nexport enum A {\n  A,\n  B,\n  C,\n}\n'],
  {
    type: "application/typescript",
  },
);
const url = URL.createObjectURL(blob);

const a = await import(url);

console.log(a.a);
console.log(a.A);
console.log(a.A.A);
