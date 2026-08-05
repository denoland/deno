// Allocate until the V8 heap limit set by `--max-memory` is hit.
const arrays = [];
while (true) {
  arrays.push(new Array(1_000_000).fill(Math.random()));
}
