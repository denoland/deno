try {
  await Deno.spawn("ls");
} catch (e) {
  console.log(e);
}

const { status } = await Deno.spawn("curl", {
  args: ["--help"],
  stdout: "null",
  stderr: "inherit",
});
console.log(status.success);
