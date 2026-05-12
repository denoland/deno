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

// Empty File (regression test for issue #33382: zero-part File round-trip)
{
  const file = new File([], "example.txt");
  const cloned = structuredClone(file);
  console.log("Empty file instanceof File:", cloned instanceof File);
  console.log("Empty file instanceof Blob:", cloned instanceof Blob);
  console.log("Empty file name:", cloned.name);
  console.log("Empty file size:", cloned.size);
  console.log("Empty file type:", JSON.stringify(cloned.type));
  console.log("Empty file is different object:", file !== cloned);
  console.log(
    "Empty file lastModified preserved:",
    cloned.lastModified === file.lastModified,
  );
}

// Empty File and Blob nested in an object (exact PoC from issue #33382)
{
  const clone = structuredClone({
    file: new File([], "example.txt"),
    blob: new Blob([]),
  });
  console.log("Nested empty file instanceof File:", clone.file instanceof File);
  console.log("Nested empty blob instanceof Blob:", clone.blob instanceof Blob);
  console.log("Nested empty file name:", clone.file.name);
  console.log("Nested empty file size:", clone.file.size);
  console.log("Nested empty blob size:", clone.blob.size);
}

// Array of mixed Blobs and Files
{
  const arr = [
    new Blob(["a"]),
    new File(["b"], "b.txt"),
    new Blob([], { type: "image/png" }),
  ];
  const cloned = structuredClone(arr);
  console.log("Array length:", cloned.length);
  console.log("Array[0] instanceof Blob:", cloned[0] instanceof Blob);
  console.log("Array[1] instanceof File:", cloned[1] instanceof File);
  console.log("Array[2] type:", cloned[2].type);
  console.log("Array[0] text:", await cloned[0].text());
  console.log("Array[1] name:", cloned[1].name);
}
