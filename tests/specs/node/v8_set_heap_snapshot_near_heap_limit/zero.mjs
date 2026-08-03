import v8 from "node:v8";

try {
  v8.setHeapSnapshotNearHeapLimit(0);
} catch (err) {
  if (err?.code === "ERR_OUT_OF_RANGE") {
    console.log("zero-limit-rejected");
    Deno.exit(0);
  }
  console.error(err);
  Deno.exit(1);
}

console.error("setHeapSnapshotNearHeapLimit(0) did not throw");
Deno.exit(1);
