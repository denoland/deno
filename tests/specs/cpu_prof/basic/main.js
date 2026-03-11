// Simple workload to generate CPU profile data
function fibonacci(n) {
  if (n <= 1) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}

// Run some computation to generate profile data
const result = fibonacci(30);
console.log("Computed fibonacci(30):", result);
