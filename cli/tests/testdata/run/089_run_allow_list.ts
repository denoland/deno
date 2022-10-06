try {
  await Deno.spawn("ls");
} catch (e) {
  console.log(e);
}

const { success } = await Deno.spawn("curl", {
  args: ["--help"],
  stdout: "null",
  stderr: "inherit",
});
console.log(success);
