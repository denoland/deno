// Blob structured clone
{
  const blob = new Blob(["hello ", "world"], { type: "text/plain" });
  const cloned = structuredClone(blob);
  console.log("Blob clone instanceof Blob:", cloned instanceof Blob);
  console.log("Blob clone size:", cloned.size);
  console.log("Blob clone type:", cloned.type);
  console.log("Blob clone text:", await cloned.text());
  console.log("Blob clone is different object:", blob !== cloned);
}

// File structured clone
{
  const file = new File(["file content"], "test.txt", {
    type: "text/plain",
    lastModified: 1234567890,
  });
  const cloned = structuredClone(file);
  console.log("File clone instanceof File:", cloned instanceof File);
  console.log("File clone instanceof Blob:", cloned instanceof Blob);
  console.log("File clone name:", cloned.name);
  console.log("File clone lastModified:", cloned.lastModified);
  console.log("File clone size:", cloned.size);
  console.log("File clone type:", cloned.type);
  console.log("File clone text:", await cloned.text());
}

// Empty blob
{
  const blob = new Blob([]);
  const cloned = structuredClone(blob);
  console.log("Empty blob size:", cloned.size);
  console.log("Empty blob type:", JSON.stringify(cloned.type));
}

// Sliced blob
{
  const blob = new Blob(["hello world"]);
  const sliced = blob.slice(0, 5);
  const cloned = structuredClone(sliced);
  console.log("Sliced blob text:", await cloned.text());
  console.log("Sliced blob size:", cloned.size);
}

// Nested blob (blob constructed from other blobs)
{
  const inner = new Blob(["inner"]);
  const outer = new Blob([inner, " outer"]);
  const cloned = structuredClone(outer);
  console.log("Nested blob text:", await cloned.text());
  console.log("Nested blob size:", cloned.size);
}

// Blob inside an object
{
  const obj = { blob: new Blob(["in object"]), value: 42 };
  const cloned = structuredClone(obj);
  console.log("Object blob text:", await cloned.blob.text());
  console.log("Object value:", cloned.value);
}
