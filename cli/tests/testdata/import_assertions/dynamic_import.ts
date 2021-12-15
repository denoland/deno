const data = await import("./data.json", { assert: { type: "json" } });

console.log(data);
