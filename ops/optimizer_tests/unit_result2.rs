pub fn op_set_nodelay(
  state: &mut OpState,
  rid: ResourceId,
  nodelay: bool,
) -> Result<(), AnyError> {
  super::check_unstable(state, "Deno.Conn#setNoDelay");
  let resource: Rc<TcpStreamResource> =
    state.resource_table.get::<TcpStreamResource>(rid)?;
  resource.set_nodelay(nodelay)
}
