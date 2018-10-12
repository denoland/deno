/** jsdoc for foo */
export function foo(a: string, b: string) {
  console.log(a, b);
}

// no jsdoc for bar
export async function bar(promise: Promise<void>): Promise<void> {
  return promise.then(() => {});
}
