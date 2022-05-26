// output the pid so we can check if this process is still running
console.log(Deno.pid);

// now loop forever
while (true) {
  await new Promise((resolve) => setTimeout(resolve, 100));
}
