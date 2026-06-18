for (const key of ["A", "B", "C", "D"]) {
  console.log(`${key}=${Deno.env.get(key)}`);
}
