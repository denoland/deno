// output the pid so we can check which process to kill
console.log(Deno.pid);

// now loop forever
while (true) {
  await new Promise((resolve) => setTimeout(resolve, 100));
}
