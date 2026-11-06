const sea = await import("node:sea").catch(() => null);
console.log(sea === null ? "not a SEA" : "SEA");
