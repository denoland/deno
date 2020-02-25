interface DenoTest {
  (definition: Deno.TestDefinition): void;
  (title: string, body: Deno.TestFunction): void;
  (body: Deno.TestFunction): void;
}

/** Add label as prefix for output text of Deno.test  */
export function testGroup(label: string): DenoTest {
  return (
    arg1: string | Deno.TestDefinition | Deno.TestFunction,
    arg2?: Deno.TestFunction
  ): void => {
    if (typeof arg1 === "string") {
      if (!(typeof arg2 === "function")) {
        throw new Error("invalid arguments");
      }
      Deno.test(`[${label}] ${arg1}`, arg2);
    } else if (typeof arg1 === "function") {
      Deno.test(`[${label}] ${arg1.name ?? ""}`, arg1);
    } else {
      const { name, fn } = arg1;
      Deno.test(`[${label}] ${name}`, fn);
    }
  };
}
