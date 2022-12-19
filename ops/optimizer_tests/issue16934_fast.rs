async fn send_stdin(state: &mut OpState, v: i32) -> Result<(), anyhow::Error> {
  // @test-attr:fast
  //
  // https://github.com/denoland/deno/issues/16934
  //
  // OpState borrowed across await point is not allowed, as it will likely panic at runtime.
  Ok(())
}
