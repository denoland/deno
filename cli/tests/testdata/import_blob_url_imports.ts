const blob = new Blob(
  [
    'export { printHello } from "http://localhost:4545/subdir/mod2.ts"',
  ],
  { type: "application/javascript" },
);
const url = URL.createObjectURL(blob);

const { printHello } = await import(url);

printHello();
