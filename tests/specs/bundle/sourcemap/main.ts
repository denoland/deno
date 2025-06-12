function add(a: number, b: number): number {
  return a + b;
}

function multiply(x: number, y: number): number {
  return x * y;
}

const result1 = add(2, 3);
const result2 = multiply(4, 5);

console.log("Addition result:", result1);
console.log("Multiplication result:", result2);
throw new Error("test");
