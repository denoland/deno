try {
  await import(`npm:${"ws:"}`);
} catch (err) {
  console.log("FAILED");
  console.log(err);
}
