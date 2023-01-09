import pl from "npm:nodejs-polars@0.5.4"; // latest version that works with deno
df = pl.DataFrame(
  {
    "A": [1, 2, 3, 4, 5],
    "fruits": ["banana", "banana", "apple", "apple", "banana"],
    "B": [5, 4, 3, 2, 1],
    "cars": ["beetle", "audi", "beetle", "beetle", "beetle"],
  },
);
