// This test creates 128 blobs of 128 MB each. This will only work if the blobs
// and their backing data is GCed as expected.
for (let i = 0; i < 128; i++) {
  // Create a 128MB byte array, and then a blob from it.
  const buf = new Uint8Array(128 * 1024 * 1024);
  new Blob([buf]);
  // It is very important that there is a yield here, otherwise the finalizer
  // for the blob is not called and the memory is not freed.
  await new Promise((resolve) => setTimeout(resolve, 0));
}
console.log("GCed all blobs");
