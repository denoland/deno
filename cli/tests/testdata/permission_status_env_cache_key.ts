if (
  await Deno.permissions.query({ name: "env", variable: "A" }) ==
    await Deno.permissions.query({ name: "env", variable: "B" })
) {
  throw new Error("Status objects should be different");
}
