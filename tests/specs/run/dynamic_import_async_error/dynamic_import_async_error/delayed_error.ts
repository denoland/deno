await new Promise((r) => setTimeout(r, 100));
throw new Error("foo");
