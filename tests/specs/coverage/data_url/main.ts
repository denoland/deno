export async function foo() {
  const { default: bar } = await import(
    "data:application/typescript,export default 'bar'"
  );
  return bar;
}
