// A JSON blob imported with an import attribute exercises the
// `requested_module_type` plumbing in the embedded loader's blob branch.
const data = { answer: 42, items: [1, 2, 3] };
const blob = new Blob([JSON.stringify(data)], { type: "application/json" });
const blobUrl = URL.createObjectURL(blob);
const mod = await import(blobUrl, { with: { type: "json" } });
console.log("result:", mod.default.answer, mod.default.items.length);
URL.revokeObjectURL(blobUrl);
