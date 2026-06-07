// Importing a blob URL that was already revoked locks down the negative path
// and the "Blob URL not found" error surfaced by the embedded loader.
const blob = new Blob(["export const x = 1;"], {
  type: "application/javascript",
});
const blobUrl = URL.createObjectURL(blob);
URL.revokeObjectURL(blobUrl);

try {
  await import(blobUrl);
  console.log("no error");
} catch (err) {
  console.log("caught:", (err as Error).message);
}
