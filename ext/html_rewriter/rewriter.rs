// Copyright 2018-2026 the Deno authors. MIT license.

//! The streaming core of the `HTMLRewriter` API.
//!
//! Each `transform()` call builds a `lol_html` rewriter that is stored on the
//! `HtmlRewriterTransform` cppgc object. Input is fed one chunk at a time:
//! `op_html_rewriter_write` / `op_html_rewriter_end` hand the rewriter to a
//! blocking task (`tokio::task::spawn_blocking`, i.e. the shared blocking
//! thread pool) for the duration of that single `write`/`end` call and then
//! take it back. The rewriter therefore only occupies a thread while it is
//! actively parsing a chunk, not for the whole lifetime of the transform: an
//! idle streaming transform (for example one waiting for the next `Response`
//! body chunk to arrive) holds no thread at all, and many concurrent
//! transforms share the bounded blocking pool instead of each spawning a
//! dedicated OS thread.
//!
//! The JS side pulls messages produced by those tasks with
//! `op_html_rewriter_pump` (async mode, used for `Response` inputs) or
//! `op_html_rewriter_pump_sync` (sync mode, used for string inputs).
//!
//! When a `lol_html` content handler fires on the blocking task, it sends
//! `Msg::Dispatch` and parks the task on the response channel until the JS
//! handler finishes (including resolution of a returned promise) and JS calls
//! `op_html_rewriter_token_done` or `op_html_rewriter_token_error`. While the
//! task is parked the dispatched token is mutated from the main thread through
//! `TokenPtr` (see `tokens.rs` for the safety invariant). Because the rewriter
//! is moved out of its storage slot for exactly the duration of one parked
//! `write`/`end`, only a single dispatch is ever outstanding at a time.

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

/// Output sink type for the rewriter. Boxed and `Send` so the rewriter can be
/// moved onto a blocking task for each `write`/`end`.
type BoxedSink = Box<dyn FnMut(&[u8]) + Send>;
/// The `Send`-able `lol_html` rewriter stored on the transform between chunks.
type Rewriter = lol_html::send::HtmlRewriter<'static, BoxedSink>;

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
  /// Sent by `op_html_rewriter_abort` (not by a rewriter task) so that a
  /// pending pump resolves.
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
    // Errors mean the main thread side is gone; the rewriter task will notice
    // (the response channel is closed) and unwind on its own.
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

/// Shared between the `lol_html` content handler closures (rewriter task) and
/// `op_html_rewriter_element_on_end_tag` (main thread), which appends an end
/// tag handler closure to a parked element.
pub(crate) struct ThreadCtx {
  msg_tx: Mutex<MsgTx>,
  response_rx: Mutex<mpsc::Receiver<DispatchResult>>,
}

impl ThreadCtx {
  fn send(&self, msg: Msg) {
    self.msg_tx.lock().unwrap().send(msg);
  }

  /// Sends a dispatch for `token` to the main thread and parks the rewriter
  /// task until the JS handler completes.
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
  /// The rewriter lives here between chunks. It is `take`n while a `write` or
  /// `end` task is running on a blocking thread, and is `None` once the
  /// transform has finished, errored, or been aborted.
  rewriter: Arc<Mutex<Option<Rewriter>>>,
  /// Bytes emitted by the rewriter's output sink. Drained after each chunk.
  output: Arc<Mutex<Vec<u8>>>,
  msg_rx: MsgRx,
  response_tx: mpsc::Sender<DispatchResult>,
  pub(crate) ctx: Arc<ThreadCtx>,
  pub(crate) current_token: RefCell<Option<CurrentToken>>,
  generation: Cell<u32>,
  /// Set once `end` has been requested (or the transform aborted) so further
  /// writes fail synchronously.
  finished: Cell<bool>,
}

impl HtmlRewriterTransform {
  /// Completes the currently dispatched token: invalidates the token pointer
  /// and unparks the rewriter task. No-op if there is no parked dispatch.
  fn finish_token(&self, result: DispatchResult) {
    if self.current_token.borrow_mut().take().is_some() {
      let _ = self.response_tx.send(result);
    }
  }

  /// Runs one `write` (`Some(chunk)`) or the final `end` (`None`) on the
  /// shared blocking pool. The rewriter is moved onto the task and, for a
  /// `write`, restored to its slot before completion is signalled so the next
  /// write can use it; `end` consumes it.
  fn spawn_chunk(
    &self,
    input: Option<Vec<u8>>,
  ) -> Result<(), HtmlRewriterError> {
    if self.finished.get() {
      return Err(HtmlRewriterError::TransformFinished);
    }
    if input.is_none() {
      self.finished.set(true);
    }
    let rewriter = self.rewriter.clone();
    let output = self.output.clone();
    let ctx = self.ctx.clone();
    tokio::task::spawn_blocking(move || {
      run_chunk(rewriter, output, ctx, input)
    });
    Ok(())
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
      // A rewriter task always sends `WriteDone`, `EndDone`, or `Error`, so a
      // closed channel means the transform was already finished.
      None => PumpMsg::Error {
        message: "The rewriter transform is no longer usable".to_string(),
        handler: false,
      },
    }
  }
}

/// Body of the blocking task spawned for each chunk. Takes the rewriter out of
/// the shared slot, runs the chunk (parking on dispatches), and reports the
/// result over the message channel.
fn run_chunk(
  rewriter_slot: Arc<Mutex<Option<Rewriter>>>,
  output: Arc<Mutex<Vec<u8>>>,
  ctx: Arc<ThreadCtx>,
  input: Option<Vec<u8>>,
) {
  let Some(mut rewriter) = rewriter_slot.lock().unwrap().take() else {
    // The rewriter is gone: the transform was finished, dropped, or aborted
    // before this task ran.
    ctx.send(Msg::Error {
      message: "The rewriter transform is no longer usable".to_string(),
      handler: false,
    });
    return;
  };

  // `write` keeps the rewriter alive (returns it for the next chunk); `end`
  // consumes it.
  let result = match input {
    Some(chunk) => rewriter.write(&chunk).map(|()| Some(rewriter)),
    None => rewriter.end().map(|()| None),
  };

  match result {
    Ok(maybe_rewriter) => {
      let bytes = std::mem::take(&mut *output.lock().unwrap());
      if let Some(rewriter) = maybe_rewriter {
        // Restore the rewriter before signalling completion so the next write
        // finds it available.
        *rewriter_slot.lock().unwrap() = Some(rewriter);
        ctx.send(Msg::WriteDone { output: bytes });
      } else {
        ctx.send(Msg::EndDone { output: bytes });
      }
    }
    Err(err) => {
      // The rewriter is dropped here, leaving the slot empty; the transform is
      // now unusable.
      let handler = matches!(
        &err,
        RewritingError::ContentHandlerError(inner)
          if inner.downcast_ref::<HandlerAborted>().is_some()
      );
      ctx.send(Msg::Error {
        message: format!("HTML rewriting failed: {err}"),
        handler,
      });
    }
  }
}

impl Drop for HtmlRewriterTransform {
  fn drop(&mut self) {
    self.finished.set(true);
    // Unpark an in-flight task if it is waiting on a dispatch. The task holds
    // its own `Arc` clones, so it can finish and drop the rewriter even after
    // the transform object is gone.
    self.finish_token(DispatchResult::Error);
    // Drop the rewriter if it is currently idle in its slot. If a task holds
    // it, the slot is empty here and the task drops it when it unwinds.
    if let Ok(mut slot) = self.rewriter.lock() {
      slot.take();
    }
  }
}

// SAFETY: this type is only accessed from the thread it was created on, and it
// holds no traced references to other GC'd objects.
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

/// Builds the `lol_html` rewriter for `spec`, wiring each handler to dispatch
/// through `ctx` and appending emitted bytes to `output`.
fn build_rewriter(
  spec: &TransformSpec,
  ctx: &Arc<ThreadCtx>,
  output: Arc<Mutex<Vec<u8>>>,
) -> Result<Rewriter, HtmlRewriterError> {
  let mut settings = Settings::new_send().with_strict(false);

  for handler in &spec.element_handlers {
    // Selectors are pre-validated on the main thread in `on()`, but parse
    // again here so a malformed selector surfaces as a clean error.
    let selector = parse_selector(&handler.selector)?;

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

  let sink_output = output;
  let sink: BoxedSink = Box::new(move |chunk: &[u8]| {
    sink_output.lock().unwrap().extend_from_slice(chunk);
  });
  Ok(HtmlRewriter::new(settings, sink))
}

#[op2]
#[cppgc]
pub fn op_html_rewriter_start(
  #[serde] spec: TransformSpec,
) -> Result<HtmlRewriterTransform, HtmlRewriterError> {
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

  let output = Arc::new(Mutex::new(Vec::<u8>::new()));
  let rewriter = build_rewriter(&spec, &ctx, output.clone())?;

  Ok(HtmlRewriterTransform {
    rewriter: Arc::new(Mutex::new(Some(rewriter))),
    output,
    msg_rx,
    response_tx,
    ctx,
    current_token: RefCell::new(None),
    generation: Cell::new(0),
    finished: Cell::new(false),
  })
}

#[op2]
pub fn op_html_rewriter_write(
  #[cppgc] transform: &HtmlRewriterTransform,
  #[anybuffer] chunk: &[u8],
) -> Result<(), HtmlRewriterError> {
  transform.spawn_chunk(Some(chunk.to_vec()))
}

#[op2(fast)]
pub fn op_html_rewriter_end(
  #[cppgc] transform: &HtmlRewriterTransform,
) -> Result<(), HtmlRewriterError> {
  transform.spawn_chunk(None)
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
  // This blocks the event loop until the rewriter task sends the next message.
  // That is safe from deadlocks: the task runs on a separate blocking thread
  // and only waits on dispatch responses provided by this very loop.
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
  transform.finished.set(true);
  // Unpark an in-flight task, dropping its rewriter on the way out.
  transform.finish_token(DispatchResult::Error);
  if let Ok(mut slot) = transform.rewriter.lock() {
    slot.take();
  }
  // Resolve a pending pump, if any. The transform object holds the message
  // sender alive through `ctx`, so the channel does not close just because a
  // rewriter task exited.
  transform.ctx.send(Msg::Aborted);
}
