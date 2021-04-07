const blob = new Blob(
  [
    'export { printHello } from "http://localhost:4545/cli/tests/subdir/mod2.ts"',
  ],
  { type: "application/javascript" },
);
const url = URL.createObjectURL(blob);

const { printHello } = await import(url);

printHello();
