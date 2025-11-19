try {
  Deno.readTextFileSync("doesnt-exist");
} catch (e) {
  if (e instanceof Deno.errors.NotFound) {
    // @ts-ignore: Property 'code' does not exist on type 'NotFound'.
    console.log("Runtime code:", (e as any).code);

    // This line should fail type checking currently
    console.log((e as Deno.errors.NotFound).code);
  }
}
