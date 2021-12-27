throw new Error("foo", { cause: new Error("bar", { cause: "deno" }) });
