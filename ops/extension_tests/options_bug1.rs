extension! {
  deno_cache,
  deps = [ deno_webidl, deno_web, deno_url, deno_fetch ],
  parameters = [CA: Cache],
  ops = [
    op_cache_storage_open::<CA>,
    op_cache_storage_has::<CA>,
    op_cache_storage_delete::<CA>,
    op_cache_put::<CA>,
    op_cache_match::<CA>,
    op_cache_delete::<CA>,
  ],
  esm = [ "01_cache.js" ],
  options = {
    maybe_create_cache: Option<CreateCache<CA>>,
  },
  state = |state, options| {
    if let Some(create_cache) = options.maybe_create_cache {
      state.put(create_cache);
    }
  },
}
