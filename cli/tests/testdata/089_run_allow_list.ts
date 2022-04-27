try {
  await Deno.spawn("ls");
} catch (e) {
  console.log(e);
}

const { status } = await Deno.spawn("curl", {
  args: ["--help"],
  stdout: "null",
});
console.log(status.success);
