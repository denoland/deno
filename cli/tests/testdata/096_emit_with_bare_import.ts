console.log(
  (await Deno.emit(new URL("095_cache_with_bare_import.ts", import.meta.url)))
    .modules,
);
