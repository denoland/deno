try {
  await new Deno.Command("ls").output();
} catch (e) {
  console.log(e);
}

const { success } = await new Deno.Command("curl", {
  args: ["--help"],
  stdout: "null",
  stderr: "inherit",
}).output();
console.log(success);
