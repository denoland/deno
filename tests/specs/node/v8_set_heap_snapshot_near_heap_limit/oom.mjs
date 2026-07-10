import v8 from "node:v8";

// Ask V8 to write a single heap snapshot right before the process OOMs.
v8.setHeapSnapshotNearHeapLimit(1);

// Allocate in an infinite loop until the (tiny) old-space limit is hit.
const sink = [];
let i = 0;
while (true) {
  sink.push(new Array(100000).fill(i++));
}
