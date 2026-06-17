// @ts-expect-error - testing runtime validation
Deno.test.each({ not: "an array" })("name", () => {});
