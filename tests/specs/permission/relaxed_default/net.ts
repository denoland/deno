// net stays gated under the relaxed profile: fetch is denied
// non-interactively naming --allow-net.
try {
  await fetch("http://localhost/");
  console.log("fetch: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-net") ? "--allow-net" : "?";
  console.log(`fetch: ${err.name} ${named}`);
}
