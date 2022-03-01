function log(object) {
  try {
    const result = Deno[Deno.internal].inspectArgs(["%o", object], {
      colors: !Deno.noColor,
    });
    console.log("inspectArgs:", result);
    return result;
  } catch (err) {
    return Deno[Deno.internal].inspectArgs(["%o", err]);
  }
}

log(null);
