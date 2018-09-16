setTimeout(() => {
  console.log("World");
}, 10);

console.log("Hello");

const id = setTimeout(() => {
  console.log("Not printed");
}, 10000);

clearTimeout(id);

const id2 = setTimeout(() => {
  console.log("Clearing timeout");
  // Should silently fail (no panic)
  clearTimeout(id2);
}, 500);

// Should silently fail (no panic)
clearTimeout(2147483647);
