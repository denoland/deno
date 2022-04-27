// signal that we started...
console.log("started");

// now loop forever
while (true) {
  await new Promise((resolve) => setTimeout(resolve, 100));
}
