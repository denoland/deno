// Simple workload to generate CPU profile data
function fibonacci(n) {
  if (n <= 1) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}

const result = fibonacci(25);
console.log("Done:", result);
