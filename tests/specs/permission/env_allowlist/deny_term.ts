// TERM is normally allowlisted, but --deny-env=TERM removes it from the grant
// because deny always wins, so the read fails naming --allow-env.
try {
  Deno.env.get("TERM");
  console.log("TERM: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-env") ? "--allow-env" : "?";
  console.log(`TERM: ${err.name} ${named}`);
}
