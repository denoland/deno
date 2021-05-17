const blob = new Blob(
  [
    "enum A {\n  A,\n  B,\n  C,\n }\n \n export function a() {\n   throw new Error(`Hello ${A.C}`);\n }\n ",
  ],
  {
    type: "application/typescript",
  },
);
const url = URL.createObjectURL(blob);

const { a } = await import(url);

a();
