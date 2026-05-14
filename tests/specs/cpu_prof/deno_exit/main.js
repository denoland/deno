// Regression test for https://github.com/denoland/deno/issues/32590
// CPU profile should be written even when Deno.exit() is called.
function fibonacci(n) {
  if (n <= 1) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}

fibonacci(30);
console.log("calling Deno.exit()");
Deno.exit(0);
