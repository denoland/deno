// TypeScript file to test that CPU profiler applies source maps
// and reports original .ts line numbers instead of transpiled .js positions.
function fibonacci(n: number): number {
  if (n <= 1) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}

const result: number = fibonacci(30);
console.log("Computed fibonacci(30):", result);
