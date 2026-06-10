export async function foo() {
  const { default: bar } = await import(
    URL.createObjectURL(
      new Blob(["export default 'bar'"], { type: "application/typescript" }),
    )
  );
  return bar;
}
