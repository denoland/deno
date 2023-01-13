async fn op_read(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: &mut [u8],
) -> Result<u32, Error> {
  // @test-attr:fast
}
