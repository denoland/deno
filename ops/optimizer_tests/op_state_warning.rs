fn op_listen(state: &mut OpState) -> Result<ResourceId, Error> {
  log::debug!("listen");
  let addr = "127.0.0.1:4570".parse::<SocketAddr>().unwrap();
  let std_listener = std::net::TcpListener::bind(&addr)?;
  std_listener.set_nonblocking(true)?;
  let listener = TcpListener::try_from(std_listener)?;
  let rid = state.resource_table.add(listener);
  Ok(rid)
}
