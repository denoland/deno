console.table({ a: "test", b: 1 });
console.table({ a: { b: 10 }, b: { b: 20, c: 30 } }, ["c"]);
console.table([1, 2, [3, [4]], [5, 6], [[7], [8]]]);
console.table(new Set([1, 2, 3, "test"]));
console.table(new Map([[1, "one"], [2, "two"]]));
console.table({
  a: true,
  b: { c: { d: 10 }, e: [1, 2, [5, 6]] },
  f: "test",
  g: new Set([1, 2, 3, "test"]),
  h: new Map([[1, "one"]])
});
console.table([1, "test", false, { a: 10 }, ["test", { b: 20, c: "test" }]]);
console.table([]);
console.table({});
console.table(new Set());
console.table(new Map());
console.table("test");
