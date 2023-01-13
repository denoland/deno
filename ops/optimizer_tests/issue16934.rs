async fn send_stdin(
  state: &mut OpState,
  cmd: String,
) -> Result<(), anyhow::Error> {
  // https://github.com/denoland/deno/issues/16934
  //
  // OpState borrowed across await point is not allowed, as it will likely panic at runtime.
  let instance = state.borrow::<MinecraftInstance>().clone();
  instance.send_command(&cmd, CausedBy::Unknown).await?;
  Ok(())
}
