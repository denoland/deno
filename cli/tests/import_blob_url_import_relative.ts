const blob = new Blob(['export { a } from "./a.ts";'], {
  type: "application/javascript",
});
const url = URL.createObjectURL(blob);

const a = await import(url);

console.log(a);
