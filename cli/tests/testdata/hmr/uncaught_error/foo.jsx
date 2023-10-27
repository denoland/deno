export function foo() {
  setTimeout(() => {
    throw new Error("fail");
  });
  return `<h1>asd1</h1>`;
}
