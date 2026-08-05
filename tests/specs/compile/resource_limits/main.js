// The `--max-memory` limit is baked into the compiled binary; allocating
// past it should terminate with a clear error.
console.log("start");
const arrays = [];
while (true) {
  arrays.push(new Array(1_000_000).fill(Math.random()));
}
