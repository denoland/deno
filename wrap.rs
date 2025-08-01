struct JsStream {
  onread: Function<Value>,
  state: Rc<RefCell<OpState>>,
}

impl AsyncRead for JsStream {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    let (tx, rx) = oneshot::channel();
    state
      .borrow()
      .borrow::<deno_core::V8TaskSpawner>()
      .spawn(|scope| {
        // call onread
      });
  }
}
