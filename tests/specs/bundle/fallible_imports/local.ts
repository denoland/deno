try {
  await import("bad");
} catch (_e) {
  console.log("import failed");
}

console.log("after import");
