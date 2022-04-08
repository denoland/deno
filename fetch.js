// Test GC.
for (let i = 0; i < 1000; i++) await fetch("http://localhost:8000/README.md");
gc();

// Test that fetch ops work.
const response = await fetch("http://localhost:8000/README.md");
const text = await response.text();
console.log(text.length);
gc();
