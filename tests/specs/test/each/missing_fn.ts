// @ts-expect-error - testing runtime validation
Deno.test.each([[1], [2]])("name");
