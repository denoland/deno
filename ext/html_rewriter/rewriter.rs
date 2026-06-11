// Copyright 2018-2026 the Deno authors. MIT license.

//! The streaming core of the `HTMLRewriter` API.
//!
//! Each `transform()` call spawns a dedicated thread that runs a `lol_html`
//! rewriter. The JS side feeds input chunks with `op_html_rewriter_write` /
//! `op_html_rewriter_end` and then pulls messages from the thread with
//! `op_html_rewriter_pump` (async mode, used for `Response` inputs) or
//! `op_html_rewriter_pump_sync` (sync mode, used for string inputs).
//!
//! When a `lol_html` content handler fires on the rewriter thread, it sends
//! `Msg::Dispatch` and parks on the response channel until the JS handler
//! finishes (including resolution of a returned promise) and JS calls
//! `op_html_rewriter_token_done` or `op_html_rewriter_token_error`. While the
//! thread is parked the dispatched token is mutated from the main thread
//! through `TokenPtr` (see `tokens.rs` for the safety invariant).

use std::cell::Cell;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;

use deno_core::GarbageCollected;
use deno_core::ToJsBuffer;
use deno_core::op2;
use lol_html::HtmlRewriter;
use lol_html::Selector;
use lol_html::errors::RewritingError;
use lol_html::send::DocumentContentHandlers;
use lol_html::send::ElementContentHandlers;
use lol_html::send::Settings;

use crate::tokens::TokenPtr;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum HtmlRewriterError {
  #[class(type)]
  #[error("{0}")]
  Selector(String),
  #[class(type)]
  #[error("This content token is no longer valid.")]
  StaleToken,
  #[class(type)]
  #[error("The rewriter transform is no longer usable")]
  TransformFinished,
  #[class(type)]
  #[error("{0}")]
  Mutation(String),
  #[class(type)]
  #[error("Token operation is not supported for this token type")]
  InvalidTokenOperation,
  #[class(generic)]
  #[error("{0}")]
  Rewriting(String),
}

/// Sentinel error returned from a `lol_html` content handler when the
/// corresponding JS handler threw. The original JS exception is rethrown by
/// the JS pump loop; this error only aborts the rewriter.
#[derive(Debug, thiserror::Error)]
#[error("JS handler errored")]
pub(crate) struct HandlerAborted;

pub(crate) enum Input {
  Write(Vec<u8>),
  End,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TokenKind {
  Element,
  Text,
  Comment,
  Doctype,
  DocumentEnd,
  EndTag,
}

pub(crate) enum Msg {
  Dispatch {
    handler_id: u32,
    kind: TokenKind,
    token: TokenPtr,
  },
  WriteDone {
    output: Vec<u8>,
  },
  EndDone {
    output: Vec<u8>,
  },
  Error {
    message: String,
    /// `true` when the rewriting error was caused by a JS handler that threw
    /// (or was aborted). In that case the JS side already has the original
    /// exception and rethrows it instead of this message.
    handler: bool,
  },
  /// Sent by `op_html_rewriter_abort` (not by the rewriter thread) so that a
  /// pending pump resolves; the rewriter thread itself exits silently when
  /// the input sender is dropped.
  Aborted,
}

#[derive(Clone, Copy)]
pub(crate) enum DispatchResult {
  Done,
  Error,
}

pub(crate) enum MsgTx {
  Async(tokio::sync::mpsc::UnboundedSender<Msg>),
  Sync(mpsc::Sender<Msg>),
}

impl MsgTx {
  fn send(&self, msg: Msg) {
    // Errors mean the main thread side is gone; the rewriter thread will
    // notice the dropped input sender and exit on its own.
    match self {
      MsgTx::Async(tx) => {
        let _ = tx.send(msg);
      }
      MsgTx::Sync(tx) => {
        let _ = tx.send(msg);
      }
    }
  }
}

enum MsgRx {
  Async(RefCell<Option<tokio::sync::mpsc::UnboundedReceiver<Msg>>>),
  Sync(mpsc::Receiver<Msg>),
}

/// Shared between the `lol_html` content handler closures (rewriter thread)
/// and `op_html_rewriter_element_on_end_tag` (main thread), which appends an
/// end tag handler closure to a parked element.
pub(crate) struct ThreadCtx {
  msg_tx: Mutex<MsgTx>,
  response_rx: Mutex<mpsc::Receiver<DispatchResult>>,
}

impl ThreadCtx {
  /// Sends a dispatch for `token` to the main thread and parks the rewriter
  /// thread until the JS handler completes.
  pub(crate) fn dispatch(
    &self,
    handler_id: u32,
    kind: TokenKind,
    token: TokenPtr,
  ) -> lol_html::HandlerResult {
    self.msg_tx.lock().unwrap().send(Msg::Dispatch {
      handler_id,
      kind,
      token,
    });
    match self.response_rx.lock().unwrap().recv() {
      Ok(DispatchResult::Done) => Ok(()),
      Ok(DispatchResult::Error) | Err(_) => Err(HandlerAborted.into()),
    }
  }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ElementHandlerSpec {
  selector: String,
  element: Option<u32>,
  comments: Option<u32>,
  text: Option<u32>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentHandlerSpec {
  doctype: Option<u32>,
  comments: Option<u32>,
  text: Option<u32>,
  end: Option<u32>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TransformSpec {
  element_handlers: Vec<ElementHandlerSpec>,
  document_handlers: Vec<DocumentHandlerSpec>,
  sync_mode: bool,
}

pub(crate) struct CurrentToken {
  pub generation: u32,
  pub token: TokenPtr,
}

pub struct HtmlRewriterTransform {
  input_tx: RefCell<Option<mpsc::Sender<Input>>>,
  msg_rx: MsgRx,
  response_tx: mpsc::Sender<DispatchResult>,
  pub(crate) ctx: Arc<ThreadCtx>,
  pub(crate) current_token: RefCell<Option<CurrentToken>>,
  generation: Cell<u32>,
}

impl HtmlRewriterTransform {
  /// Completes the currently dispatched token: invalidates the token pointer
  /// and unparks the rewriter thread. No-op if there is no parked dispatch.
  fn finish_token(&self, result: DispatchResult) {
    if self.current_token.borrow_mut().take().is_some() {
      let _ = self.response_tx.send(result);
    }
  }

  fn process_msg(&self, msg: Option<Msg>) -> PumpMsg {
    match msg {
      Some(Msg::Dispatch {
        handler_id,
        kind,
        token,
      }) => {
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        *self.current_token.borrow_mut() =
          Some(CurrentToken { generation, token });
        PumpMsg::Dispatch {
          handler_id,
          token_kind: kind,
          generation,
        }
      }
      Some(Msg::WriteDone { output }) => PumpMsg::WriteDone {
        output: output.into(),
      },
      Some(Msg::EndDone { output }) => PumpMsg::EndDone {
        output: output.into(),
      },
      Some(Msg::Error { message, handler }) => {
        PumpMsg::Error { message, handler }
      }
      Some(Msg::Aborted) => PumpMsg::Aborted,
      // The rewriter thread always sends `EndDone` or `Error` before exiting,
      // so a closed channel means the transform was already finished.
      None => PumpMsg::Error {
        message: "The rewriter transform is no longer usable".to_string(),
        handler: false,
      },
    }
  }
}

impl Drop for HtmlRewriterTransform {
  fn drop(&mut self) {
    // Drop the input sender so the rewriter thread exits, and unpark it if
    // it is waiting on a dispatch.
    self.input_tx.borrow_mut().take();
    self.finish_token(DispatchResult::Error);
  }
}

// SAFETY: this type is neither Send nor Sync, the cppgc object is only
// accessed from the thread it was created on, and it holds no traced
// references to other GC'd objects.
unsafe impl GarbageCollected for HtmlRewriterTransform {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"HtmlRewriterTransform"
  }
}

#[derive(serde::Serialize)]
#[serde(
  tag = "kind",
  rename_all = "camelCase",
  rename_all_fields = "camelCase"
)]
pub(crate) enum PumpMsg {
  Dispatch {
    handler_id: u32,
    token_kind: TokenKind,
    generation: u32,
  },
  WriteDone {
    output: ToJsBuffer,
  },
  EndDone {
    output: ToJsBuffer,
  },
  Error {
    message: String,
    handler: bool,
  },
  Aborted,
}

fn parse_selector(selector: &str) -> Result<Selector, HtmlRewriterError> {
  selector.parse::<Selector>().map_err(|err| {
    HtmlRewriterError::Selector(format!("Invalid selector: {err}"))
  })
}

#[op2(fast)]
pub fn op_html_rewriter_parse_selector(
  #[string] selector: &str,
) -> Result<(), HtmlRewriterError> {
  parse_selector(selector)?;
  Ok(())
}

#[op2]
#[cppgc]
pub fn op_html_rewriter_start(
  #[serde] spec: TransformSpec,
) -> HtmlRewriterTransform {
  let (input_tx, input_rx) = mpsc::channel::<Input>();
  let (response_tx, response_rx) = mpsc::channel::<DispatchResult>();

  let (msg_tx, msg_rx) = if spec.sync_mode {
    let (tx, rx) = mpsc::channel::<Msg>();
    (MsgTx::Sync(tx), MsgRx::Sync(rx))
  } else {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();
    (MsgTx::Async(tx), MsgRx::Async(RefCell::new(Some(rx))))
  };

  let ctx = Arc::new(ThreadCtx {
    msg_tx: Mutex::new(msg_tx),
    response_rx: Mutex::new(response_rx),
  });

  let thread_ctx = ctx.clone();
  // The thread exits when the input sender is dropped or when rewriting
  // finishes or errors, so it never outlives the transform for long.
  let _ = std::thread::Builder::new()
    .name("html-rewriter".to_string())
    .spawn(move || rewriter_thread(spec, input_rx, thread_ctx));

  HtmlRewriterTransform {
    input_tx: RefCell::new(Some(input_tx)),
    msg_rx,
    response_tx,
    ctx,
    current_token: RefCell::new(None),
    generation: Cell::new(0),
  }
}

fn rewriter_thread(
  spec: TransformSpec,
  input_rx: mpsc::Receiver<Input>,
  ctx: Arc<ThreadCtx>,
) {
  let send_msg = |msg: Msg| ctx.msg_tx.lock().unwrap().send(msg);

  let mut settings = Settings::new_send().with_strict(false);

  for handler in &spec.element_handlers {
    // Selectors are pre-validated on the main thread in `on()`.
    let selector = match parse_selector(&handler.selector) {
      Ok(selector) => selector,
      Err(err) => {
        send_msg(Msg::Error {
          message: err.to_string(),
          handler: false,
        });
        return;
      }
    };

    let mut handlers = ElementContentHandlers::default();
    if let Some(handler_id) = handler.element {
      let ctx = ctx.clone();
      handlers =
        handlers.element(move |el: &mut lol_html::send::Element<'_, '_>| {
          ctx.dispatch(handler_id, TokenKind::Element, TokenPtr::element(el))
        });
    }
    if let Some(handler_id) = handler.comments {
      let ctx = ctx.clone();
      handlers = handlers.comments(
        move |comment: &mut lol_html::html_content::Comment<'_>| {
          ctx.dispatch(
            handler_id,
            TokenKind::Comment,
            TokenPtr::comment(comment),
          )
        },
      );
    }
    if let Some(handler_id) = handler.text {
      let ctx = ctx.clone();
      handlers = handlers.text(
        move |text: &mut lol_html::html_content::TextChunk<'_>| {
          ctx.dispatch(handler_id, TokenKind::Text, TokenPtr::text(text))
        },
      );
    }

    settings = settings.append_element_content_handler((
      std::borrow::Cow::Owned(selector),
      handlers,
    ));
  }

  for handler in &spec.document_handlers {
    let mut handlers = DocumentContentHandlers::default();
    if let Some(handler_id) = handler.doctype {
      let ctx = ctx.clone();
      handlers = handlers.doctype(
        move |doctype: &mut lol_html::html_content::Doctype<'_>| {
          ctx.dispatch(
            handler_id,
            TokenKind::Doctype,
            TokenPtr::doctype(doctype),
          )
        },
      );
    }
    if let Some(handler_id) = handler.comments {
      let ctx = ctx.clone();
      handlers = handlers.comments(
        move |comment: &mut lol_html::html_content::Comment<'_>| {
          ctx.dispatch(
            handler_id,
            TokenKind::Comment,
            TokenPtr::comment(comment),
          )
        },
      );
    }
    if let Some(handler_id) = handler.text {
      let ctx = ctx.clone();
      handlers = handlers.text(
        move |text: &mut lol_html::html_content::TextChunk<'_>| {
          ctx.dispatch(handler_id, TokenKind::Text, TokenPtr::text(text))
        },
      );
    }
    if let Some(handler_id) = handler.end {
      let ctx = ctx.clone();
      handlers = handlers.end(
        move |end: &mut lol_html::html_content::DocumentEnd<'_>| {
          ctx.dispatch(
            handler_id,
            TokenKind::DocumentEnd,
            TokenPtr::document_end(end),
          )
        },
      );
    }

    settings = settings.append_document_content_handler(handlers);
  }

  let output = std::rc::Rc::new(RefCell::new(Vec::<u8>::new()));
  let sink_output = output.clone();
  let mut rewriter = HtmlRewriter::new(settings, move |chunk: &[u8]| {
    sink_output.borrow_mut().extend_from_slice(chunk);
  });

  let send_rewriting_error = |err: RewritingError| {
    let handler = matches!(
      &err,
      RewritingError::ContentHandlerError(inner)
        if inner.downcast_ref::<HandlerAborted>().is_some()
    );
    send_msg(Msg::Error {
      message: format!("HTML rewriting failed: {err}"),
      handler,
    });
  };

  loop {
    match input_rx.recv() {
      Ok(Input::Write(chunk)) => {
        if let Err(err) = rewriter.write(&chunk) {
          send_rewriting_error(err);
          return;
        }
        send_msg(Msg::WriteDone {
          output: std::mem::take(&mut output.borrow_mut()),
        });
      }
      Ok(Input::End) => {
        if let Err(err) = rewriter.end() {
          send_rewriting_error(err);
          return;
        }
        send_msg(Msg::EndDone {
          output: std::mem::take(&mut output.borrow_mut()),
        });
        return;
      }
      // The transform was dropped or aborted.
      Err(_) => return,
    }
  }
}

#[op2]
pub fn op_html_rewriter_write(
  #[cppgc] transform: &HtmlRewriterTransform,
  #[anybuffer] chunk: &[u8],
) -> Result<(), HtmlRewriterError> {
  let input_tx = transform.input_tx.borrow();
  let input_tx = input_tx
    .as_ref()
    .ok_or(HtmlRewriterError::TransformFinished)?;
  input_tx
    .send(Input::Write(chunk.to_vec()))
    .map_err(|_| HtmlRewriterError::TransformFinished)
}

#[op2(fast)]
pub fn op_html_rewriter_end(
  #[cppgc] transform: &HtmlRewriterTransform,
) -> Result<(), HtmlRewriterError> {
  let input_tx = transform
    .input_tx
    .borrow_mut()
    .take()
    .ok_or(HtmlRewriterError::TransformFinished)?;
  input_tx
    .send(Input::End)
    .map_err(|_| HtmlRewriterError::TransformFinished)
}

#[op2]
#[serde]
pub async fn op_html_rewriter_pump(
  #[cppgc] transform: &HtmlRewriterTransform,
) -> Result<PumpMsg, HtmlRewriterError> {
  let MsgRx::Async(rx_cell) = &transform.msg_rx else {
    return Err(HtmlRewriterError::InvalidTokenOperation);
  };
  // Take the receiver out so a misbehaving concurrent pump fails cleanly
  // instead of panicking on a double borrow across the await point.
  let mut rx = rx_cell
    .borrow_mut()
    .take()
    .ok_or(HtmlRewriterError::TransformFinished)?;
  let msg = rx.recv().await;
  *rx_cell.borrow_mut() = Some(rx);
  Ok(transform.process_msg(msg))
}

#[op2]
#[serde]
pub fn op_html_rewriter_pump_sync(
  #[cppgc] transform: &HtmlRewriterTransform,
) -> Result<PumpMsg, HtmlRewriterError> {
  let MsgRx::Sync(rx) = &transform.msg_rx else {
    return Err(HtmlRewriterError::InvalidTokenOperation);
  };
  // This blocks the event loop until the rewriter thread sends the next
  // message. That is safe from deadlocks: the rewriter thread never waits on
  // the event loop, only on dispatch responses provided by this very loop.
  let msg = rx.recv().ok();
  Ok(transform.process_msg(msg))
}

#[op2(fast)]
pub fn op_html_rewriter_token_done(#[cppgc] transform: &HtmlRewriterTransform) {
  transform.finish_token(DispatchResult::Done);
}

#[op2(fast)]
pub fn op_html_rewriter_token_error(
  #[cppgc] transform: &HtmlRewriterTransform,
) {
  transform.finish_token(DispatchResult::Error);
}

#[op2(fast)]
pub fn op_html_rewriter_abort(#[cppgc] transform: &HtmlRewriterTransform) {
  transform.input_tx.borrow_mut().take();
  transform.finish_token(DispatchResult::Error);
  // Resolve a pending pump, if any. The transform object holds the message
  // sender alive through `ctx`, so the channel does not close when the
  // rewriter thread exits.
  transform.ctx.msg_tx.lock().unwrap().send(Msg::Aborted);
}
