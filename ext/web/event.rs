// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(clippy::too_many_arguments)]
#![allow(clippy::extra_unused_lifetimes)]

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::rc::Rc;
use std::rc::Weak;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::WebIDL;
use deno_core::cppgc;
use deno_core::error::JsError;
use deno_core::error::dispatch_exception;
use deno_core::error::to_v8_error;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::Global;
use deno_core::webidl::ContextFn;
use deno_core::webidl::Nullable;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;
use deno_error::JsErrorBox;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum EventError {
  #[class(type)]
  #[error("Argument 2 is not of type 'Object'")]
  InvalidListenerType,
  #[class(type)]
  #[error("Argument 1 is expected Event")]
  ExpectedEvent,
  #[class(type)]
  #[error("Illegal invocation")]
  InvalidCall,
  #[class(type)]
  #[error("Illegal constructor")]
  InvalidConstructor,
  #[class("DOMExceptionInvalidStateError")]
  #[error("Invalid event state")]
  InvalidState,
  #[class(generic)]
  #[error(transparent)]
  DataError(#[from] v8::DataError),
  #[class(inherit)]
  #[error(transparent)]
  WebIDL(#[from] WebIdlError),
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct EventInit {
  #[webidl(default = false)]
  bubbles: bool,
  #[webidl(default = false)]
  cancelable: bool,
  #[webidl(default = false)]
  composed: bool,
}

#[derive(WebIDL, Clone, Debug)]
#[webidl(enum)]
pub enum EventPhase {
  None,
  CapturingPhase,
  AtTarget,
  BubblingPhase,
}

#[derive(Debug)]
struct Path {
  invocation_target: v8::Global<v8::Object>,
  root_of_closed_tree: bool,
  slot_in_closed_tree: bool,
  // item_in_shadow_tree: bool,
  shadow_adjusted_target: Option<v8::Global<v8::Object>>,
  related_target: Option<v8::Global<v8::Object>>,
  // touch_target_list: Vec<v8::Global<v8::Object>>,
}

enum InvokePhase {
  Capturing,
  Bubbling,
}

#[derive(Debug)]
pub struct Event {
  typ: RefCell<String>,
  bubbles: Cell<bool>,
  cancelable: Cell<bool>,
  composed: bool,

  target: RefCell<Option<v8::Global<v8::Object>>>,
  related_target: RefCell<Option<v8::Global<v8::Object>>>,
  current_target: RefCell<Option<v8::Global<v8::Object>>>,
  path: RefCell<Vec<Path>>,
  event_phase: RefCell<EventPhase>,

  // flags
  stop_propagation_flag: Cell<bool>,
  stop_immediate_propagation_flag: Cell<bool>,
  canceled_flag: Cell<bool>,
  in_passive_listener_flag: Cell<bool>,
  // ShadowRoot is not implemented
  // composed_flag: Cell<bool>,
  // document.createEvent is not implemented
  // initialized_flag: Cell<bool>,
  dispatch_flag: Cell<bool>,

  is_trusted: Cell<bool>,
  time_stamp: f64,
}

impl GarbageCollected for Event {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Event"
  }
}

impl Event {
  #[inline]
  fn new(typ: String, init: Option<EventInit>) -> Event {
    let (bubbles, cancelable, composed) = if let Some(init) = init {
      (init.bubbles, init.cancelable, init.composed)
    } else {
      (false, false, false)
    };

    Event {
      typ: RefCell::new(typ),
      bubbles: Cell::new(bubbles),
      cancelable: Cell::new(cancelable),
      composed,

      target: RefCell::new(None),
      related_target: RefCell::new(None),
      current_target: RefCell::new(None),
      path: RefCell::new(Vec::new()),
      event_phase: RefCell::new(EventPhase::None),

      // flags
      stop_propagation_flag: Cell::new(false),
      stop_immediate_propagation_flag: Cell::new(false),
      canceled_flag: Cell::new(false),
      in_passive_listener_flag: Cell::new(false),
      dispatch_flag: Cell::new(false),

      is_trusted: Cell::new(false),
      time_stamp: 0.0,
    }
  }

  // https://dom.spec.whatwg.org/#concept-event-dispatch
  fn dispatch<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    state: &Rc<RefCell<OpState>>,
    event_object: v8::Local<'a, v8::Object>,
    target: &EventTarget,
    target_object: v8::Global<v8::Object>,
    target_override: Option<v8::Global<v8::Object>>,
  ) -> bool {
    // NOTE: Omit unnecessary implementations for Node, MouseEvent, and Slottable

    // 1.
    self.dispatch_flag.set(true);

    // 2.
    let target_override = target_override.or(Some(target_object.clone()));

    // 4.
    let related_target = self.related_target.borrow().clone();

    // 6.3.
    self.append_to_event_path(
      target_object.clone(),
      target_override,
      related_target,
      false,
    );

    // 6.13.
    {
      let event_path = self.path.borrow();
      for (path_index, path) in event_path.iter().enumerate().rev() {
        if path.shadow_adjusted_target.is_none() {
          self.event_phase.replace(EventPhase::CapturingPhase);
          self.invoke(
            scope,
            state,
            event_object,
            target,
            target_object.clone(),
            &event_path,
            path_index,
            InvokePhase::Capturing,
          );
        }
      }

      // 6.14.
      for (path_index, path) in event_path.iter().enumerate() {
        if path.shadow_adjusted_target.is_some() {
          self.event_phase.replace(EventPhase::AtTarget);
        } else {
          if !self.bubbles.get() {
            continue;
          }
          self.event_phase.replace(EventPhase::BubblingPhase);
        }

        self.invoke(
          scope,
          state,
          event_object,
          target,
          target_object.clone(),
          &event_path,
          path_index,
          InvokePhase::Bubbling,
        );
      }
    }

    // 7.
    self.event_phase.replace(EventPhase::None);

    // 8.
    self.current_target.replace(None);

    // 9.
    self.path.borrow_mut().clear();

    // 10.
    self.dispatch_flag.set(false);
    self.stop_propagation_flag.set(false);
    self.stop_immediate_propagation_flag.set(false);

    !self.canceled_flag.get()
  }

  // https://dom.spec.whatwg.org/#concept-event-path-append
  #[inline]
  fn append_to_event_path(
    &self,
    invocation_target: v8::Global<v8::Object>,
    shadow_adjusted_target: Option<v8::Global<v8::Object>>,
    related_target: Option<v8::Global<v8::Object>>,
    slot_in_closed_tree: bool,
  ) {
    let mut path = self.path.borrow_mut();
    path.push(Path {
      invocation_target,
      root_of_closed_tree: false,
      slot_in_closed_tree,
      shadow_adjusted_target,
      related_target,
    });
  }

  // https://dom.spec.whatwg.org/#concept-event-listener-invoke
  #[inline]
  fn invoke<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    state: &Rc<RefCell<OpState>>,
    event_object: v8::Local<'a, v8::Object>,
    target: &EventTarget,
    target_object: v8::Global<v8::Object>,
    path: &[Path],
    path_index: usize,
    phase: InvokePhase,
  ) {
    // 1.
    for (index, current) in path.iter().enumerate().rev() {
      if let Some(target) = &current.shadow_adjusted_target {
        self.target.replace(Some(target.clone()));
        break;
      }
      if index == path_index {
        break;
      }
    }

    // 2.
    let current = &path[path_index];
    self.related_target.replace(current.related_target.clone());

    // 4.
    if self.stop_propagation_flag.get() {
      return;
    }

    // 5.
    self
      .current_target
      .replace(Some(current.invocation_target.clone()));

    // 6.
    let typ = self.typ.borrow();
    let listeners = {
      let listeners = target.listeners.borrow();
      let Some(listeners) = listeners.get(&*typ) else {
        return;
      };
      listeners.clone()
    };

    // 8.
    let _ = self.inner_invoke(
      scope,
      state,
      event_object,
      target_object.clone(),
      &typ,
      target,
      listeners,
      phase,
    );
  }

  // https://dom.spec.whatwg.org/#concept-event-listener-inner-invoke
  #[inline]
  fn inner_invoke<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    state: &Rc<RefCell<OpState>>,
    event_object: v8::Local<'a, v8::Object>,
    target_object: v8::Global<v8::Object>,
    typ: &String,
    target: &EventTarget,
    listeners: Vec<Rc<EventListener>>,
    phase: InvokePhase,
  ) -> bool {
    // NOTE: Omit implementations for window.event (current event)

    // 1.
    let mut found = false;

    // 2.
    for listener in listeners.iter() {
      if listener.removed.get() {
        continue;
      }

      // 2.2.
      found = true;

      // 3.
      // 4.
      if (matches!(phase, InvokePhase::Capturing) && !listener.capture)
        || (matches!(phase, InvokePhase::Bubbling) && listener.capture)
      {
        continue;
      }

      // 5.
      if listener.once {
        let mut listeners = target.listeners.borrow_mut();
        let listeners = listeners.get_mut(typ).unwrap();
        listener.removed.set(true);
        listeners.remove(
          listeners
            .iter()
            .position(|l| Rc::ptr_eq(l, listener))
            .unwrap(),
        );
      }

      // 9.
      if listener.passive {
        self.in_passive_listener_flag.set(true);
      }

      // 11.
      let scope = &mut v8::TryCatch::new(scope);

      let callback = v8::Local::new(scope, listener.callback.clone());
      let key = v8::String::new(scope, "handleEvent").unwrap();
      if let Some(handle_event) = callback.get(scope, key.into())
        && let Ok(handle_event) =
          v8::Local::<v8::Function>::try_from(handle_event)
      {
        let recv = v8::Local::new(scope, &target_object);
        handle_event.call(scope, recv.into(), &[event_object.into()]);
      } else {
        match v8::Local::<v8::Function>::try_from(callback) {
          Ok(callback) => {
            let recv = v8::Local::new(scope, &target_object);
            callback.call(scope, recv.into(), &[event_object.into()]);
          }
          // 11.1.
          Err(error) => {
            let message = v8::String::new(scope, &error.to_string()).unwrap();
            let exception = v8::Exception::type_error(scope, message);
            report_exception(scope, state, exception);
          }
        }
      }

      // 11.1.
      if let Some(exception) = scope.exception() {
        report_exception(scope, state, exception);
      }

      // 12.
      self.in_passive_listener_flag.set(false);

      // 14.
      if self.stop_immediate_propagation_flag.get()
        && !listener.resist_stop_immediate_propagation
      {
        break;
      }
    }

    // 15.
    found
  }
}

#[op2(base)]
impl Event {
  #[constructor]
  #[required(1)]
  #[cppgc]
  fn constructor(
    #[webidl] typ: String,
    #[webidl] init: Nullable<EventInit>,
  ) -> Event {
    Event::new(typ, init.into_option())
  }

  // legacy
  #[required(1)]
  fn init_event<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] typ: String,
    #[webidl] bubbles: Option<bool>,
    #[webidl] cancelable: Option<bool>,
  ) -> v8::Local<'a, v8::Primitive> {
    let undefined = v8::undefined(scope);
    if self.dispatch_flag.get() {
      return undefined;
    }

    self.typ.replace(typ);
    if let Some(bubbles) = bubbles {
      self.bubbles.replace(bubbles);
    }
    if let Some(cancelable) = cancelable {
      self.cancelable.replace(cancelable);
    }
    undefined
  }

  #[getter]
  #[rename("type")]
  #[string]
  fn typ(&self) -> String {
    self.typ.borrow().clone()
  }

  #[getter]
  #[global]
  fn target(&self) -> Option<v8::Global<v8::Object>> {
    self.target.borrow().clone()
  }

  // deprecated: an alias of target
  #[getter]
  #[global]
  fn src_element<'a>(&self) -> Option<v8::Global<v8::Object>> {
    self.target.borrow().clone()
  }

  #[getter]
  #[global]
  fn current_target(&self) -> Option<v8::Global<v8::Object>> {
    self.current_target.borrow().clone()
  }

  fn composed_path<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Array> {
    let path = self.path.borrow();
    if path.is_empty() {
      return v8::Array::new(scope, 0);
    }

    let current_target = self.current_target.borrow();
    let current_target = current_target.as_ref().unwrap();
    let mut composed_path: VecDeque<v8::Local<'a, v8::Value>> = VecDeque::new();
    composed_path.push_back(v8::Local::new(scope, current_target).into());

    let mut current_target_index = 0;
    let mut current_target_hidden_subtree_level = 0;
    for (
      index,
      Path {
        invocation_target,
        root_of_closed_tree,
        slot_in_closed_tree,
        ..
      },
    ) in path.iter().enumerate().rev()
    {
      if *root_of_closed_tree {
        current_target_hidden_subtree_level += 1;
      }

      if *invocation_target == current_target {
        current_target_index = index;
        break;
      }

      if *slot_in_closed_tree {
        current_target_hidden_subtree_level -= 1;
      }
    }

    let mut current_hidden_level = current_target_hidden_subtree_level;
    let mut max_hidden_level = current_target_hidden_subtree_level;
    for Path {
      invocation_target,
      root_of_closed_tree,
      slot_in_closed_tree,
      ..
    } in path[0..current_target_index - 1].iter().rev()
    {
      if *root_of_closed_tree {
        current_hidden_level += 1;
      }

      if current_hidden_level <= max_hidden_level {
        composed_path
          .push_front(v8::Local::new(scope, invocation_target).into());
      }

      if *slot_in_closed_tree {
        current_hidden_level -= 1;
        if current_hidden_level < max_hidden_level {
          max_hidden_level = current_hidden_level;
        }
      }
    }

    current_hidden_level = current_target_hidden_subtree_level;
    max_hidden_level = current_target_hidden_subtree_level;
    for Path {
      invocation_target,
      root_of_closed_tree,
      slot_in_closed_tree,
      ..
    } in path[current_target_index + 1..].iter()
    {
      if *slot_in_closed_tree {
        current_hidden_level += 1;
      }

      if current_hidden_level <= max_hidden_level {
        composed_path
          .push_back(v8::Local::new(scope, invocation_target).into());
      }

      if *root_of_closed_tree {
        current_hidden_level -= 1;
        if current_hidden_level < max_hidden_level {
          max_hidden_level = current_hidden_level;
        }
      }
    }

    v8::Array::new_with_elements(scope, composed_path.make_contiguous())
  }

  #[fast]
  #[getter]
  fn bubbles(&self) -> bool {
    self.bubbles.get()
  }

  #[fast]
  #[getter]
  fn cancelable(&self) -> bool {
    self.cancelable.get()
  }

  #[fast]
  #[getter]
  fn composed(&self) -> bool {
    self.composed
  }

  #[fast]
  #[getter]
  fn event_phase(&self) -> i32 {
    self.event_phase.borrow().clone() as i32
  }

  #[fast]
  fn stop_propagation(&self) {
    self.stop_propagation_flag.set(true);
  }

  // legacy
  #[fast]
  #[getter]
  fn cancel_bubble(&self) -> bool {
    self.stop_propagation_flag.get()
  }

  // legacy
  #[fast]
  #[setter]
  fn cancel_bubble(&self, value: bool) {
    self.stop_propagation_flag.set(value);
  }

  #[fast]
  fn stop_immediate_propagation(&self) {
    self.stop_propagation_flag.set(true);
    self.stop_immediate_propagation_flag.set(true);
  }

  #[fast]
  #[getter]
  fn default_prevented(&self) -> bool {
    self.canceled_flag.get()
  }

  // legacy
  #[fast]
  #[getter]
  fn return_value(&self) -> bool {
    !self.canceled_flag.get()
  }

  // legacy
  #[fast]
  #[setter]
  fn return_value(&self, value: bool) {
    if !value {
      self.canceled_flag.set(true);
    }
  }

  #[fast]
  fn prevent_default(&self) {
    if self.cancelable.get() && !self.in_passive_listener_flag.get() {
      self.canceled_flag.set(true);
    }
  }

  // document.createEvent is not implemented
  #[fast]
  #[getter]
  fn initialized(&self) -> bool {
    true
  }

  // Not spec compliant. The spec defines it as [LegacyUnforgeable]
  // but doing so has a big performance hit
  #[fast]
  #[getter]
  fn is_trusted(&self) -> bool {
    self.is_trusted.get()
  }

  #[fast]
  #[getter]
  fn time_stamp(&self) -> f64 {
    self.time_stamp
  }
}

#[op2(fast)]
pub fn op_event_set_is_trusted(#[cppgc] event: &Event, value: bool) {
  event.is_trusted.set(value);
}

#[op2]
pub fn op_event_set_target(
  #[cppgc] event: &Event,
  #[global] value: v8::Global<v8::Object>,
) {
  event.target.replace(Some(value));
}

#[op2(reentrant)]
pub fn op_event_dispatch<'a>(
  scope: &mut v8::HandleScope<'a>,
  state: Rc<RefCell<OpState>>,
  #[global] target_object: v8::Global<v8::Object>,
  event_object: v8::Local<'a, v8::Object>,
  #[global] target_override: Option<v8::Global<v8::Object>>,
) -> bool {
  let target = v8::Local::new(scope, &target_object);
  let target =
    cppgc::try_unwrap_cppgc_proto_object::<EventTarget>(scope, target.into())
      .unwrap();
  let event =
    cppgc::try_unwrap_cppgc_proto_object::<Event>(scope, event_object.into())
      .unwrap();
  event.dispatch(
    scope,
    &state,
    event_object,
    &target,
    target_object,
    target_override,
  )
}

#[derive(Debug)]
pub struct CustomEvent {
  detail: RefCell<Option<v8::Global<v8::Value>>>,
}

impl GarbageCollected for CustomEvent {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"CustomEvent"
  }
}

impl CustomEvent {
  #[inline]
  fn new(detail: Option<v8::Global<v8::Value>>) -> CustomEvent {
    CustomEvent {
      detail: RefCell::new(detail),
    }
  }
}

#[op2(inherit = Event)]
impl CustomEvent {
  #[constructor]
  #[required(1)]
  #[cppgc]
  fn constructor<'a>(
    scope: &mut v8::HandleScope<'a>,
    #[webidl] typ: String,
    init: v8::Local<'a, v8::Value>,
  ) -> Result<(Event, CustomEvent), EventError> {
    if init.is_null_or_undefined() {
      return Ok((Event::new(typ, None), CustomEvent::new(None)));
    }

    let event_init = Nullable::<EventInit>::convert(
      scope,
      init,
      "Failed to construct 'CustomEvent'".into(),
      (|| "Argument 2".into()).into(),
      &Default::default(),
    )?;
    let event = Event::new(typ, event_init.into_option());

    let detail = if let Ok(init) = init.try_cast::<v8::Object>() {
      get_value(scope, init, "detail")
        .map(|detail| v8::Global::new(scope, detail))
    } else {
      None
    };
    let custom_event = CustomEvent::new(detail);
    Ok((event, custom_event))
  }

  // legacy
  #[required(1)]
  fn init_custom_event<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] typ: String,
    #[webidl] bubbles: Option<bool>,
    #[webidl] cancelable: Option<bool>,
    #[global] detail: Option<v8::Global<v8::Value>>,
    #[proto] event: &Event,
  ) -> v8::Local<'a, v8::Primitive> {
    let undefined = v8::undefined(scope);
    if event.dispatch_flag.get() {
      return undefined;
    }

    event.typ.replace(typ);
    if let Some(bubbles) = bubbles {
      event.bubbles.replace(bubbles);
    }
    if let Some(cancelable) = cancelable {
      event.cancelable.replace(cancelable);
    }
    if detail.is_some() {
      self.detail.replace(detail);
    }
    undefined
  }

  #[getter]
  #[global]
  fn detail(&self) -> Option<v8::Global<v8::Value>> {
    self.detail.borrow().clone()
  }
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct ErrorEventInit {
  #[webidl(default = false)]
  bubbles: bool,
  #[webidl(default = false)]
  cancelable: bool,
  #[webidl(default = false)]
  composed: bool,
  #[webidl(default = String::new())]
  message: String,
  #[webidl(default = String::new())]
  filename: String,
  #[webidl(default = 0)]
  lineno: u32,
  #[webidl(default = 0)]
  colno: u32,
  // #[webidl(default = None)]
  // error: Option<v8::Global<v8::Value>>,
}

#[derive(Debug)]
pub struct ErrorEvent {
  message: String,
  filename: String,
  lineno: u32,
  colno: u32,
  error: Option<v8::Global<v8::Value>>,
}

impl GarbageCollected for ErrorEvent {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ErrorEvent"
  }
}

impl ErrorEvent {
  #[inline]
  fn new(
    init: Option<ErrorEventInit>,
    error: Option<v8::Global<v8::Value>>,
  ) -> ErrorEvent {
    let Some(init) = init else {
      return ErrorEvent {
        message: String::new(),
        filename: String::new(),
        lineno: 0,
        colno: 0,
        error,
      };
    };

    ErrorEvent {
      message: init.message,
      filename: init.filename,
      lineno: init.lineno,
      colno: init.colno,
      error,
    }
  }
}

#[op2(inherit = Event)]
impl ErrorEvent {
  #[constructor]
  #[required(1)]
  #[cppgc]
  fn constructor<'a>(
    scope: &mut v8::HandleScope<'a>,
    #[webidl] typ: String,
    init: v8::Local<'a, v8::Value>,
  ) -> Result<(Event, ErrorEvent), EventError> {
    if init.is_null_or_undefined() {
      return Ok((Event::new(typ, None), ErrorEvent::new(None, None)));
    }

    let error_event_init = Nullable::<ErrorEventInit>::convert(
      scope,
      init,
      "Failed to construct 'ErrorEvent'".into(),
      (|| "Argument 2".into()).into(),
      &Default::default(),
    )?;
    let error_event_init = error_event_init.into_option();
    let event = if let Some(ref error_event_init) = error_event_init {
      let event_init = EventInit {
        bubbles: error_event_init.bubbles,
        cancelable: error_event_init.cancelable,
        composed: error_event_init.composed,
      };
      Event::new(typ, Some(event_init))
    } else {
      Event::new(typ, None)
    };

    let error = if let Ok(init) = init.try_cast::<v8::Object>() {
      get_value(scope, init, "error").map(|error| v8::Global::new(scope, error))
    } else {
      None
    };
    let error_event = ErrorEvent::new(error_event_init, error);
    Ok((event, error_event))
  }

  #[getter]
  #[string]
  fn message(&self) -> String {
    self.message.clone()
  }

  #[getter]
  #[string]
  fn filename(&self) -> String {
    self.filename.clone()
  }

  #[getter]
  fn lineno(&self) -> u32 {
    self.lineno
  }

  #[getter]
  fn colno(&self) -> u32 {
    self.colno
  }

  #[getter]
  fn error<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Value> {
    if let Some(error) = &self.error {
      v8::Local::new(scope, error)
    } else {
      v8::undefined(scope).into()
    }
  }
}

#[derive(Debug)]
pub struct PromiseRejectionEvent {
  promise: v8::Global<v8::Object>,
  reason: Option<v8::Global<v8::Value>>,
}

impl GarbageCollected for PromiseRejectionEvent {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"PromiseRejectionEvent"
  }
}

impl PromiseRejectionEvent {
  #[inline]
  fn new(
    promise: v8::Global<v8::Object>,
    reason: Option<v8::Global<v8::Value>>,
  ) -> PromiseRejectionEvent {
    PromiseRejectionEvent { promise, reason }
  }
}

#[op2(inherit = Event)]
impl PromiseRejectionEvent {
  #[constructor]
  #[required(1)]
  #[cppgc]
  fn constructor<'a>(
    scope: &mut v8::HandleScope<'a>,
    #[webidl] typ: String,
    init: v8::Local<'a, v8::Object>,
  ) -> Result<(Event, PromiseRejectionEvent), EventError> {
    let prefix = "Failed to construct 'PromiseRejectionEvent'";
    let event_init = EventInit::convert(
      scope,
      init.into(),
      prefix.into(),
      (|| "Argument 2".into()).into(),
      &Default::default(),
    )?;
    let event = Event::new(typ, Some(event_init));

    let promise = {
      let promise = get_value(scope, init, "promise");
      if let Some(promise) = promise
        && let Ok(promise) = promise.try_cast::<v8::Object>()
      {
        v8::Global::new(scope, promise)
      } else {
        return Err(EventError::WebIDL(WebIdlError::new(
          prefix.into(),
          (|| "'promise' of 'PromiseRejectionEventInit' (Argument 2)".into())
            .into(),
          WebIdlErrorKind::ConvertToConverterType("object"),
        )));
      }
    };
    let reason = get_value(scope, init, "reason")
      .map(|reason| v8::Global::new(scope, reason));
    let promise_rejection_event = PromiseRejectionEvent::new(promise, reason);
    Ok((event, promise_rejection_event))
  }

  #[getter]
  #[global]
  fn promise(&self) -> v8::Global<v8::Object> {
    self.promise.clone()
  }

  #[getter]
  fn reason<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Value> {
    if let Some(reason) = &self.reason {
      v8::Local::new(scope, reason)
    } else {
      v8::undefined(scope).into()
    }
  }
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct CloseEventInit {
  #[webidl(default = false)]
  bubbles: bool,
  #[webidl(default = false)]
  cancelable: bool,
  #[webidl(default = false)]
  composed: bool,
  #[webidl(default = false)]
  was_clean: bool,
  #[webidl(default = 0)]
  code: u16,
  #[webidl(default = String::new())]
  reason: String,
}

#[derive(Debug)]
pub struct CloseEvent {
  was_clean: bool,
  code: u16,
  reason: String,
}

impl GarbageCollected for CloseEvent {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"CloseEvent"
  }
}

impl CloseEvent {
  #[inline]
  fn new(init: Option<CloseEventInit>) -> CloseEvent {
    let (was_clean, code, reason) = if let Some(init) = init {
      (init.was_clean, init.code, init.reason)
    } else {
      (false, 0, String::new())
    };
    CloseEvent {
      was_clean,
      code,
      reason,
    }
  }
}

#[op2(inherit = Event)]
impl CloseEvent {
  #[constructor]
  #[required(1)]
  #[cppgc]
  fn constructor(
    #[webidl] typ: String,
    #[webidl] init: Nullable<CloseEventInit>,
  ) -> (Event, CloseEvent) {
    let init = init.into_option();
    let event = if let Some(ref init) = init {
      let event_init = EventInit {
        bubbles: init.bubbles,
        cancelable: init.cancelable,
        composed: init.composed,
      };
      Event::new(typ, Some(event_init))
    } else {
      Event::new(typ, None)
    };
    let close_event = CloseEvent::new(init);
    (event, close_event)
  }

  #[getter]
  fn was_clean(&self) -> bool {
    self.was_clean
  }

  #[getter]
  fn code(&self) -> u16 {
    self.code
  }

  #[getter]
  #[string]
  fn reason(&self) -> String {
    self.reason.clone()
  }
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct MessageEventInit {
  #[webidl(default = false)]
  bubbles: bool,
  #[webidl(default = false)]
  cancelable: bool,
  #[webidl(default = false)]
  composed: bool,
  #[webidl(default = String::new())]
  origin: String,
  #[webidl(default = String::new())]
  last_event_id: String,
  // #[webidl(default = None)]
  // data: Option<v8::Global<v8::Value>>,
  // #[webidl(default = None)]
  // source: Option<v8::Global<v8::Object>>,
  // #[webidl(default = None)]
  // ports: Option<v8::Global<v8::Array>>,
}

#[derive(Debug)]
pub struct MessageEvent {
  origin: RefCell<String>,
  last_event_id: RefCell<String>,
  data: RefCell<Option<v8::Global<v8::Value>>>,
  source: RefCell<Option<v8::Global<v8::Object>>>,
  ports: RefCell<v8::Global<v8::Array>>,
}

impl GarbageCollected for MessageEvent {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"MessageEvent"
  }
}

impl MessageEvent {
  #[inline]
  fn new(
    init: Option<MessageEventInit>,
    data: Option<v8::Global<v8::Value>>,
    source: Option<v8::Global<v8::Object>>,
    ports: v8::Global<v8::Array>,
  ) -> MessageEvent {
    let Some(init) = init else {
      return MessageEvent {
        origin: RefCell::new(String::new()),
        last_event_id: RefCell::new(String::new()),
        data: RefCell::new(data),
        source: RefCell::new(source),
        ports: RefCell::new(ports),
      };
    };

    MessageEvent {
      origin: RefCell::new(init.origin),
      last_event_id: RefCell::new(init.last_event_id),
      data: RefCell::new(data),
      source: RefCell::new(source),
      ports: RefCell::new(ports),
    }
  }
}

#[op2(inherit = Event)]
impl MessageEvent {
  #[constructor]
  #[required(1)]
  #[cppgc]
  fn constructor<'a>(
    scope: &mut v8::HandleScope<'a>,
    #[webidl] typ: String,
    init: v8::Local<'a, v8::Value>,
  ) -> Result<(Event, MessageEvent), EventError> {
    if init.is_null_or_undefined() {
      let ports = v8::Array::new(scope, 0);
      return Ok((
        Event::new(typ, None),
        MessageEvent::new(None, None, None, Global::new(scope, ports)),
      ));
    }

    let prefix = "Failed to construct 'MessageEvent'";
    let message_event_init = Nullable::<MessageEventInit>::convert(
      scope,
      init,
      prefix.into(),
      (|| "Argument 2".into()).into(),
      &Default::default(),
    )?;
    let message_event_init = message_event_init.into_option();
    let event = if let Some(ref message_event_init) = message_event_init {
      let event_init = EventInit {
        bubbles: message_event_init.bubbles,
        cancelable: message_event_init.cancelable,
        composed: message_event_init.composed,
      };
      Event::new(typ, Some(event_init))
    } else {
      Event::new(typ, None)
    };

    let (data, source, ports) = if let Ok(init) = init.try_cast::<v8::Object>()
    {
      let data = get_value(scope, init, "data")
        .map(|value| v8::Global::new(scope, value));
      // TODO(petamoriken): Validate Window or MessagePort
      let source = if let Some(source) = get_value(scope, init, "source") {
        if let Ok(source) = source.try_cast::<v8::Object>() {
          Some(v8::Global::new(scope, source))
        } else {
          return Err(EventError::WebIDL(WebIdlError::new(
            prefix.into(),
            (|| "'source' of 'MessageEventInit' (Argument 2)".into()).into(),
            WebIdlErrorKind::ConvertToConverterType("object"),
          )));
        }
      } else {
        None
      };
      // TODO(petamoriken): Validate sequence<MessagePort>
      let ports = if let Some(ports) = get_value(scope, init, "ports") {
        let context = || "'ports' of 'MessageEventInit' (Argument 2)".into();
        let elements = Vec::<v8::Local<'a, v8::Value>>::convert(
          scope,
          ports,
          prefix.into(),
          context.into(),
          &Default::default(),
        )?;
        if elements.iter().any(|element| !element.is_object()) {
          return Err(EventError::WebIDL(WebIdlError::new(
            prefix.into(),
            context.into(),
            WebIdlErrorKind::ConvertToConverterType("sequence"),
          )));
        }
        v8::Array::new_with_elements(scope, &elements)
      } else {
        v8::Array::new(scope, 0)
      };
      ports.set_integrity_level(scope, v8::IntegrityLevel::Frozen);
      let ports = v8::Global::new(scope, ports);
      (data, source, ports)
    } else {
      let ports = v8::Array::new(scope, 0);
      ports.set_integrity_level(scope, v8::IntegrityLevel::Frozen);
      let ports = v8::Global::new(scope, ports);
      (None, None, ports)
    };
    let message_event =
      MessageEvent::new(message_event_init, data, source, ports);
    Ok((event, message_event))
  }

  // legacy
  #[required(1)]
  #[undefined]
  fn init_message_event<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] typ: String,
    #[webidl] bubbles: Option<bool>,
    #[webidl] cancelable: Option<bool>,
    #[global] data: Option<v8::Global<v8::Value>>,
    #[webidl] origin: Option<String>,
    #[webidl] last_event_id: Option<String>,
    #[global] source: Option<v8::Global<v8::Object>>,
    ports: Option<v8::Local<'a, v8::Value>>,
    #[proto] event: &Event,
  ) -> Result<(), EventError> {
    if event.dispatch_flag.get() {
      return Ok(());
    }

    event.typ.replace(typ);
    if let Some(bubbles) = bubbles {
      event.bubbles.replace(bubbles);
    }
    if let Some(cancelable) = cancelable {
      event.cancelable.replace(cancelable);
    }
    if data.is_some() {
      self.data.replace(data);
    }
    if let Some(origin) = origin {
      self.origin.replace(origin);
    }
    if let Some(last_event_id) = last_event_id {
      self.last_event_id.replace(last_event_id);
    }
    // TODO(petamoriken): Validate Window or MessagePort
    if source.is_some() {
      self.source.replace(source);
    }
    // TODO(petamoriken): Validate sequence<MessagePort>
    if let Some(ports) = ports {
      let prefix = "Failed to execute 'initMessageEvent' on 'MessageEvent'";
      let context = || "Argument 8".into();
      let elements = Vec::<v8::Local<'a, v8::Value>>::convert(
        scope,
        ports,
        prefix.into(),
        context.into(),
        &Default::default(),
      )?;
      if elements.iter().any(|element| !element.is_object()) {
        return Err(EventError::WebIDL(WebIdlError::new(
          prefix.into(),
          context.into(),
          WebIdlErrorKind::ConvertToConverterType("sequence"),
        )));
      }
      let ports = v8::Array::new_with_elements(scope, &elements);
      ports.set_integrity_level(scope, v8::IntegrityLevel::Frozen);
      let ports = v8::Global::new(scope, ports);
      self.ports.replace(ports);
    }
    Ok(())
  }

  #[getter]
  #[string]
  fn origin(&self) -> String {
    self.origin.borrow().clone()
  }

  #[getter]
  #[string]
  fn last_event_id(&self) -> String {
    self.last_event_id.borrow().clone()
  }

  #[getter]
  #[global]
  fn data(&self) -> Option<v8::Global<v8::Value>> {
    self.data.borrow().clone()
  }

  #[getter]
  #[global]
  fn source(&self) -> Option<v8::Global<v8::Object>> {
    self.source.borrow().clone()
  }

  #[getter]
  #[global]
  fn ports(&self) -> v8::Global<v8::Array> {
    self.ports.borrow().clone()
  }
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct ProgressEventInit {
  #[webidl(default = false)]
  bubbles: bool,
  #[webidl(default = false)]
  cancelable: bool,
  #[webidl(default = false)]
  composed: bool,
  #[webidl(default = false)]
  length_computable: bool,
  #[webidl(default = 0.0)]
  loaded: f64,
  #[webidl(default = 0.0)]
  total: f64,
}

#[derive(Debug)]
pub struct ProgressEvent {
  length_computable: bool,
  loaded: f64,
  total: f64,
}

impl GarbageCollected for ProgressEvent {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ProgressEvent"
  }
}

impl ProgressEvent {
  #[inline]
  fn new(init: Option<ProgressEventInit>) -> ProgressEvent {
    let Some(init) = init else {
      return ProgressEvent {
        length_computable: false,
        loaded: 0.0,
        total: 0.0,
      };
    };

    ProgressEvent {
      length_computable: init.length_computable,
      loaded: init.loaded,
      total: init.total,
    }
  }
}

#[op2(inherit = Event)]
impl ProgressEvent {
  #[constructor]
  #[required(1)]
  #[cppgc]
  fn constructor<'a>(
    #[webidl] typ: String,
    #[webidl] init: Nullable<ProgressEventInit>,
  ) -> (Event, ProgressEvent) {
    let init = init.into_option();
    let event = if let Some(ref init) = init {
      let event_init = EventInit {
        bubbles: init.bubbles,
        cancelable: init.cancelable,
        composed: init.composed,
      };
      Event::new(typ, Some(event_init))
    } else {
      Event::new(typ, None)
    };
    let progress_event = ProgressEvent::new(init);
    (event, progress_event)
  }

  #[getter]
  fn length_computable(&self) -> bool {
    self.length_computable
  }

  #[getter]
  fn loaded(&self) -> f64 {
    self.loaded
  }

  #[getter]
  fn total(&self) -> f64 {
    self.total
  }
}

#[derive(Default)]
pub(crate) struct ReportExceptionStackedCalls(u32);

// https://html.spec.whatwg.org/#report-the-exception
fn report_exception<'a>(
  scope: &mut v8::HandleScope<'a>,
  state: &Rc<RefCell<OpState>>,
  exception: v8::Local<'a, v8::Value>,
) {
  // Avoid recursing `reportException()` via error handlers more than once.
  let callable = {
    let mut state = state.borrow_mut();
    let stacked_calls = state.borrow_mut::<ReportExceptionStackedCalls>();
    stacked_calls.0 += 1;
    stacked_calls.0 == 1
  };

  let allow_default = if callable {
    let js_error = JsError::from_v8_exception(scope, exception);
    let message = js_error.message;
    let (file_name, line_number, column_number) =
      if let Some(frame) = js_error.frames.first() {
        (
          frame.file_name.clone(),
          frame.line_number,
          frame.column_number,
        )
      } else {
        let message = v8::String::empty(scope);
        let exception = v8::Exception::error(scope, message);
        let js_error = JsError::from_v8_exception(scope, exception);
        if let Some(frame) = js_error.frames.iter().find(|frame| {
          frame
            .file_name
            .as_ref()
            .is_some_and(|file_name| !file_name.starts_with("ext:"))
        }) {
          (
            frame.file_name.clone(),
            frame.line_number,
            frame.column_number,
          )
        } else {
          (None, None, None)
        }
      };
    let event_object = {
      let event = Event::new(
        "error".into(),
        Some(EventInit {
          bubbles: false,
          cancelable: true,
          composed: false,
        }),
      );
      let error_event = ErrorEvent {
        message: message.unwrap_or_default(),
        filename: file_name.unwrap_or_default(),
        lineno: line_number.unwrap_or(0) as u32,
        colno: column_number.unwrap_or(0) as u32,
        error: Some(v8::Global::new(scope, exception)),
      };
      let event_object = cppgc::make_cppgc_empty_object::<ErrorEvent>(scope);
      cppgc::wrap_object2(scope, event_object, (event, error_event))
    };
    let event =
      cppgc::try_unwrap_cppgc_proto_object::<Event>(scope, event_object.into())
        .unwrap();
    let global = scope.get_current_context().global(scope);
    let target =
      cppgc::try_unwrap_cppgc_proto_object::<EventTarget>(scope, global.into())
        .unwrap();
    let global = v8::Global::new(scope, global);
    event.dispatch(scope, state, event_object, &target, global, None)
  } else {
    true
  };

  if allow_default {
    dispatch_exception(scope, exception, false);
  }

  let mut state = state.borrow_mut();
  let stacked_calls = state.borrow_mut::<ReportExceptionStackedCalls>();
  stacked_calls.0 -= 1;
}

#[op2(fast, reentrant, required(1))]
pub fn op_event_report_exception<'a>(
  scope: &mut v8::HandleScope<'a>,
  state: Rc<RefCell<OpState>>,
  exception: v8::Local<'a, v8::Value>,
) {
  report_exception(scope, &state, exception);
}

#[op2(fast, reentrant, required(1))]
pub fn op_event_report_error<'a>(
  #[this] this: v8::Global<v8::Object>,
  scope: &mut v8::HandleScope<'a>,
  state: Rc<RefCell<OpState>>,
  exception: v8::Local<'a, v8::Value>,
) -> Result<(), EventError> {
  let global = scope.get_current_context().global(scope);
  if global != this {
    return Err(EventError::InvalidCall);
  }
  report_exception(scope, &state, exception);
  Ok(())
}

#[inline]
fn get_value<'a>(
  scope: &mut v8::HandleScope<'a>,
  obj: v8::Local<'a, v8::Object>,
  key: &str,
) -> Option<v8::Local<'a, v8::Value>> {
  let key = v8::String::new(scope, key).unwrap();
  if let Some(value) = obj.get(scope, key.into())
    && !value.is_undefined()
  {
    Some(value)
  } else {
    None
  }
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct EventListenerOptions {
  #[webidl(default = false)]
  capture: bool,
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct AddEventListenerOptions {
  #[webidl(default = false)]
  capture: bool,
  #[webidl(default = false)]
  passive: bool,
  #[webidl(default = false)]
  once: bool,
  // #[webidl(default = false)]
  // resist_stop_immediate_propagation: bool,
  // signal: v8::Global<v8::Object>
}

#[derive(Debug)]
struct EventListener {
  callback: v8::Global<v8::Object>,
  capture: bool,
  passive: bool,
  once: bool,
  removed: Cell<bool>,
  // This field exists for simulating Node.js behavior, implemented in https://github.com/nodejs/node/commit/bcd35c334ec75402ee081f1c4da128c339f70c24
  // Some internal event listeners in Node.js can ignore `e.stopImmediatePropagation()` calls　from the earlier event listeners.
  resist_stop_immediate_propagation: bool,
}

#[derive(Debug)]
pub struct EventTarget {
  listeners: Rc<RefCell<HashMap<String, Vec<Rc<EventListener>>>>>,
}

impl GarbageCollected for EventTarget {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"EventTarget"
  }
}

impl EventTarget {
  #[inline]
  fn new() -> EventTarget {
    EventTarget {
      listeners: Rc::new(RefCell::new(HashMap::new())),
    }
  }
}

#[op2(base)]
impl EventTarget {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> EventTarget {
    EventTarget::new()
  }

  #[required(2)]
  #[undefined]
  fn add_event_listener<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] typ: String,
    callback: Option<v8::Local<'a, v8::Value>>,
    options: Option<v8::Local<'a, v8::Value>>,
  ) -> Result<(), EventError> {
    let prefix = "Failed to execute 'addEventListener' on 'EventTarget'";

    let (capture, passive, once, resist_stop_immediate_propagation, signal) =
      match options {
        Some(options) => {
          if let Ok(options) = options.try_cast::<v8::Object>() {
            let key =
              v8::String::new(scope, "Deno.stopImmediatePropagation").unwrap();
            let symbol = v8::Symbol::for_key(scope, key);
            let resist_stop_immediate_propagation =
              match options.get(scope, symbol.into()) {
                Some(value) => value.to_boolean(scope).is_true(),
                None => false,
              };

            let key = v8::String::new(scope, "signal").unwrap();
            let signal = match options.get(scope, key.into()) {
              Some(value) => {
                if value.is_undefined() {
                  None
                } else {
                  match cppgc::try_unwrap_cppgc_proto_object::<AbortSignal>(
                    scope, value,
                  ) {
                    Some(signal) => Some(signal),
                    None => {
                      return Err(EventError::WebIDL(WebIdlError::new(
                        prefix.into(),
                        (|| {
                          "'signal' of 'AddEventListenerOptions' (Argument 3)"
                            .into()
                        })
                        .into(),
                        WebIdlErrorKind::ConvertToConverterType("AbortSignal"),
                      )));
                    }
                  }
                }
              }
              None => None,
            };

            let options = AddEventListenerOptions::convert(
              scope,
              options.into(),
              prefix.into(),
              (|| "Argument 3".into()).into(),
              &Default::default(),
            )?;

            (
              options.capture,
              options.passive,
              options.once,
              resist_stop_immediate_propagation,
              signal,
            )
          } else {
            (
              options.to_boolean(scope).is_true(),
              false,
              false,
              false,
              None,
            )
          }
        }
        None => (false, false, false, false, None),
      };

    let aborted = match signal {
      Some(ref signal) => signal.aborted_inner(),
      None => false,
    };
    if aborted {
      return Ok(());
    }

    let callback = match callback {
      None => {
        return Ok(());
      }
      Some(callback) => {
        if callback.is_null() {
          return Ok(());
        }
        if !callback.is_object() {
          return Err(EventError::InvalidListenerType);
        }
        callback.cast::<v8::Object>()
      }
    };
    let callback = v8::Global::new(scope, callback);

    let mut listeners = self.listeners.borrow_mut();
    let listeners = listeners.entry(typ.clone()).or_default();
    for listener in listeners.iter() {
      if listener.capture == capture && listener.callback == callback {
        return Ok(());
      }
    }

    let listener = Rc::new(EventListener {
      callback,
      capture,
      passive,
      once,
      removed: Cell::new(false),
      resist_stop_immediate_propagation,
    });

    if let Some(ref signal) = signal {
      let abort_callback = |_scope: &mut v8::HandleScope,
                            args: v8::FunctionCallbackArguments,
                            _rv: v8::ReturnValue| {
        let context = v8::Local::<v8::External>::try_from(args.data())
          .expect("Abort algorithm expected external data");
        // SAFETY: `context` is a valid pointer to a EventListener instance
        let listener =
          unsafe { Weak::from_raw(context.value() as *const EventListener) };
        let Some(listener) = listener.upgrade() else {
          return;
        };
        // TODO(petamoriken): remove listener from listeners
        listener.removed.set(true);
      };
      let listener = Rc::downgrade(&listener);
      let external = v8::External::new(scope, Weak::into_raw(listener) as _);
      let abort_algorithm = v8::Function::builder(abort_callback)
        .data(external.into())
        .build(scope)
        .expect("Failed to create abort algorithm");
      let abort_algorithm = v8::Global::new(scope, abort_algorithm);
      signal.add(abort_algorithm);
    }

    listeners.push(listener);
    Ok(())
  }

  #[required(2)]
  #[undefined]
  fn remove_event_listener<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] typ: String,
    callback: Option<v8::Local<'a, v8::Object>>,
    options: Option<v8::Local<'a, v8::Value>>,
  ) -> Result<(), EventError> {
    let capture = match options {
      Some(options) => {
        if options.is_object() {
          let options = EventListenerOptions::convert(
            scope,
            options,
            "Failed to execute 'removeEventListener' on 'EventTarget'".into(),
            (|| "Argument 3".into()).into(),
            &Default::default(),
          )?;
          options.capture
        } else {
          options.to_boolean(scope).is_true()
        }
      }
      None => false,
    };

    let callback = match callback {
      None => {
        return Ok(());
      }
      Some(callback) => {
        if callback.is_null() {
          return Ok(());
        }
        callback
      }
    };

    let mut listeners = self.listeners.borrow_mut();
    let Some(listeners) = listeners.get_mut(&typ) else {
      return Ok(());
    };
    if let Some((index, listener)) =
      listeners.iter().enumerate().find(|(_, listener)| {
        listener.capture == capture && listener.callback == callback
      })
    {
      listener.removed.set(true);
      listeners.remove(index);
    }
    Ok(())
  }

  #[fast]
  #[reentrant]
  #[required(1)]
  fn dispatch_event<'a>(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::HandleScope<'a>,
    state: Rc<RefCell<OpState>>,
    event_object: v8::Local<'a, v8::Value>,
  ) -> Result<bool, EventError> {
    let Some(event) =
      cppgc::try_unwrap_cppgc_proto_object::<Event>(scope, event_object.into())
    else {
      return Err(EventError::ExpectedEvent);
    };

    let typ = event.typ.borrow();

    // This is an optimization to avoid creating an event listener on each startup.
    // Stores the flag for checking whether unload is dispatched or not.
    // This prevents the recursive dispatches of unload events.
    // See https://github.com/denoland/deno/issues/9201.
    let global = scope.get_current_context().global(scope);
    if this == global && *typ == "unload" {
      let key = v8::String::new(scope, "Deno.isUnloadDispatched").unwrap();
      let symbol = v8::Symbol::for_key(scope, key);
      let value = v8::Boolean::new(scope, true);
      global.set(scope, symbol.into(), value.into());
    }

    {
      let listeners = self.listeners.borrow();
      if listeners.get(&*typ).is_none() {
        event.target.replace(Some(this));
        return Ok(true);
      };
    }

    if event.dispatch_flag.get()
      || !matches!(*event.event_phase.borrow(), EventPhase::None)
    {
      return Err(EventError::InvalidState);
    }

    Ok(event.dispatch(scope, &state, event_object, self, this, None))
  }
}

#[op2(fast)]
pub fn op_event_wrap_event_target<'a>(
  scope: &mut v8::HandleScope<'a>,
  obj: v8::Local<'a, v8::Object>,
) {
  cppgc::wrap_object1(scope, obj, EventTarget::new());
}

#[op2]
pub fn op_event_get_target_listeners<'a>(
  scope: &mut v8::HandleScope<'a>,
  #[cppgc] event_target: &EventTarget,
  #[string] typ: String,
) -> v8::Local<'a, v8::Array> {
  let listeners = event_target.listeners.borrow();
  match listeners.get(&typ) {
    Some(listeners) => {
      let elements: Vec<v8::Local<'a, v8::Value>> = listeners
        .iter()
        .map(|listener| v8::Local::new(scope, listener.callback.clone()).into())
        .collect();
      v8::Array::new_with_elements(scope, elements.as_slice())
    }
    None => v8::Array::new(scope, 0),
  }
}

#[op2(fast)]
pub fn op_event_get_target_listener_count<'a>(
  #[cppgc] event_target: &EventTarget,
  #[string] typ: String,
) -> u32 {
  let listeners = event_target.listeners.borrow();
  match listeners.get(&typ) {
    Some(listeners) => listeners.len() as u32,
    None => 0,
  }
}

pub struct AbortSignal {
  reason: RefCell<Option<v8::Global<v8::Value>>>,
  algorithms: RefCell<HashSet<v8::Global<v8::Function>>>,
  dependent: Cell<bool>,
  source_signals: RefCell<Vec<v8::Weak<v8::Value>>>,
  dependent_signals: RefCell<Vec<v8::Weak<v8::Value>>>,
}

impl GarbageCollected for AbortSignal {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"AbortSignal"
  }
}

impl AbortSignal {
  #[inline]
  fn new() -> AbortSignal {
    AbortSignal {
      reason: RefCell::new(None),
      algorithms: RefCell::new(HashSet::new()),
      dependent: Cell::new(false),
      source_signals: RefCell::new(Vec::new()),
      dependent_signals: RefCell::new(Vec::new()),
    }
  }

  // https://dom.spec.whatwg.org/#create-a-dependent-abort-signal
  fn new_with_dependent<'a>(
    scope: &mut v8::HandleScope<'a>,
    result_signal_object: v8::Local<'a, v8::Object>,
    signal_values: Vec<v8::Local<'a, v8::Value>>,
    prefix: Cow<'static, str>,
    context: ContextFn<'_>,
  ) -> Result<AbortSignal, EventError> {
    let result_signal = AbortSignal::new();
    result_signal.dependent.set(true);

    {
      let result_signal_weak =
        v8::Weak::new(scope, result_signal_object.cast::<v8::Value>());
      let mut result_source_signal = result_signal.source_signals.borrow_mut();
      for signal_value in signal_values {
        let Some(signal) = cppgc::try_unwrap_cppgc_proto_object::<AbortSignal>(
          scope,
          signal_value,
        ) else {
          return Err(EventError::WebIDL(WebIdlError::new(
            prefix,
            context,
            WebIdlErrorKind::ConvertToConverterType("AbortSignal"),
          )));
        };
        if !signal.dependent.get() {
          let signal_weak = v8::Weak::new(scope, signal_value);
          result_source_signal.push(signal_weak);
          signal
            .dependent_signals
            .borrow_mut()
            .push(result_signal_weak.clone());
        } else {
          for source_signal_weak in signal.source_signals.borrow().iter() {
            if let Some(source_signal_value) =
              source_signal_weak.to_local(scope)
            {
              let source_signal = cppgc::try_unwrap_cppgc_proto_object::<
                AbortSignal,
              >(scope, source_signal_value)
              .unwrap();
              result_source_signal.push(source_signal_weak.clone());
              source_signal
                .dependent_signals
                .borrow_mut()
                .push(result_signal_weak.clone());
            }
          }
        }
      }
    }

    Ok(result_signal)
  }

  #[inline]
  fn aborted_inner(&self) -> bool {
    self.reason.borrow().is_some()
  }

  // https://dom.spec.whatwg.org/#abortsignal-add
  #[inline]
  fn add(&self, algorithm: v8::Global<v8::Function>) {
    if self.aborted_inner() {
      return;
    }
    self.algorithms.borrow_mut().insert(algorithm);
  }

  // https://dom.spec.whatwg.org/#abortsignal-remove
  #[inline]
  fn remove(&self, algorithm: v8::Global<v8::Function>) {
    self.algorithms.borrow_mut().remove(&algorithm);
  }

  // https://dom.spec.whatwg.org/#abortsignal-signal-abort
  fn signal_abort<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    state: &Rc<RefCell<OpState>>,
    signal_object: v8::Local<'a, v8::Object>,
    reason: Option<v8::Local<'a, v8::Value>>,
  ) {
    if self.aborted_inner() {
      return;
    }

    let reason = if let Some(reason) = reason
      && !reason.is_undefined()
    {
      reason
    } else {
      let error = JsErrorBox::new(
        "DOMExceptionAbortError",
        "The signal has been aborted",
      );
      to_v8_error(scope, &error)
    };
    let reason = v8::Global::new(scope, reason);
    self.reason.replace(Some(reason.clone()));

    let mut dependent_signals_to_abort = Vec::new();
    {
      let dependent_signals = self.dependent_signals.borrow();
      for dependent_signal_weak in &*dependent_signals {
        if let Some(dependent_signal_value) =
          dependent_signal_weak.to_local(scope)
        {
          let dependent_signal = cppgc::try_unwrap_cppgc_proto_object::<
            AbortSignal,
          >(scope, dependent_signal_value)
          .unwrap();
          if !dependent_signal.aborted_inner() {
            dependent_signal.reason.replace(Some(reason.clone()));
            dependent_signals_to_abort
              .push((dependent_signal, dependent_signal_value));
          }
        }
      }
    }

    self.run_abort_steps(scope, state, signal_object);

    for (dependent_signal, dependent_signal_value) in dependent_signals_to_abort
    {
      dependent_signal.run_abort_steps(
        scope,
        state,
        dependent_signal_value.cast(),
      );
    }
  }

  // https://dom.spec.whatwg.org/#run-the-abort-steps
  #[inline]
  fn run_abort_steps<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    state: &Rc<RefCell<OpState>>,
    signal_object: v8::Local<'a, v8::Object>,
  ) {
    {
      let algorithms = self.algorithms.borrow();
      for algorithm in algorithms.iter() {
        let func = v8::Local::new(scope, algorithm);
        func.call(scope, signal_object.into(), &[]);
      }
    }

    self.algorithms.borrow_mut().clear();

    let target = cppgc::try_unwrap_cppgc_proto_object::<EventTarget>(
      scope,
      signal_object.into(),
    )
    .unwrap();
    let event_object = {
      let event = Event::new("abort".to_string(), None);
      event.is_trusted.set(true);
      cppgc::make_cppgc_proto_object(scope, event)
    };
    let event =
      cppgc::try_unwrap_cppgc_proto_object::<Event>(scope, event_object.into())
        .unwrap();
    let signal_object = v8::Global::new(scope, signal_object);
    event.dispatch(scope, state, event_object, &target, signal_object, None);
  }
}

#[op2(inherit = EventTarget)]
impl AbortSignal {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<AbortSignal, EventError> {
    Err(EventError::InvalidConstructor)
  }

  #[static_method]
  fn abort<'a>(
    scope: &mut v8::HandleScope<'a>,
    reason: Option<v8::Local<'a, v8::Value>>,
  ) -> v8::Local<'a, v8::Object> {
    let event_target = EventTarget::new();
    let abort_signal = AbortSignal::new();

    let reason = if let Some(reason) = reason
      && !reason.is_undefined()
    {
      reason
    } else {
      let error = JsErrorBox::new(
        "DOMExceptionAbortError",
        "The signal has been aborted",
      );
      to_v8_error(scope, &error)
    };
    let reason = v8::Global::new(scope, reason);
    abort_signal.reason.replace(Some(reason));

    let obj = cppgc::make_cppgc_empty_object::<AbortSignal>(scope);
    cppgc::wrap_object2(scope, obj, (event_target, abort_signal))
  }

  #[required(1)]
  #[static_method]
  fn any<'a>(
    scope: &mut v8::HandleScope<'a>,
    signals: v8::Local<'a, v8::Array>,
  ) -> Result<v8::Local<'a, v8::Object>, EventError> {
    let prefix = "Failed to execute 'AbortSignal.any'";
    let context = || "Argument 1".into();
    let signals = Vec::<v8::Local<'a, v8::Value>>::convert(
      scope,
      signals.into(),
      prefix.into(),
      context.into(),
      &Default::default(),
    )?;
    let event_target = EventTarget::new();
    let obj = cppgc::make_cppgc_empty_object::<AbortSignal>(scope);
    let abort_signal = AbortSignal::new_with_dependent(
      scope,
      obj,
      signals,
      prefix.into(),
      context.into(),
    )?;

    Ok(cppgc::wrap_object2(
      scope,
      obj,
      (event_target, abort_signal),
    ))
  }

  fn throw_if_aborted<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Value> {
    if let Some(reason) = &*self.reason.borrow() {
      let reason = v8::Local::new(scope, reason);
      scope.throw_exception(reason);
    }
    v8::undefined(scope).into()
  }

  #[getter]
  fn aborted(&self) -> bool {
    self.aborted_inner()
  }

  #[getter]
  fn reason<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Value> {
    if let Some(reason) = &*self.reason.borrow() {
      v8::Local::new(scope, reason)
    } else {
      v8::undefined(scope).into()
    }
  }
}

pub struct AbortController {
  signal: v8::Global<v8::Object>,
}

impl GarbageCollected for AbortController {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"AbortController"
  }
}

#[op2]
impl AbortController {
  #[constructor]
  #[cppgc]
  fn constructor<'a>(scope: &mut v8::HandleScope<'a>) -> AbortController {
    let event_target = EventTarget::new();
    let abort_signal = AbortSignal::new();
    let signal = cppgc::make_cppgc_empty_object::<AbortSignal>(scope);
    cppgc::wrap_object2(scope, signal, (event_target, abort_signal));
    AbortController {
      signal: v8::Global::new(scope, signal),
    }
  }

  #[getter]
  #[global]
  fn signal(&self) -> v8::Global<v8::Object> {
    self.signal.clone()
  }

  fn abort<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    state: Rc<RefCell<OpState>>,
    reason: Option<v8::Local<'a, v8::Value>>,
  ) -> v8::Local<'a, v8::Primitive> {
    let undefined = v8::undefined(scope);
    let signal_object = v8::Local::new(scope, self.signal.clone());
    let signal = cppgc::try_unwrap_cppgc_proto_object::<AbortSignal>(
      scope,
      signal_object.into(),
    )
    .unwrap();
    signal.signal_abort(scope, &state, signal_object, reason);
    undefined
  }
}

#[op2]
pub fn op_event_create_abort_signal<'a>(
  scope: &mut v8::HandleScope<'a>,
) -> v8::Local<'a, v8::Object> {
  let event_target = EventTarget::new();
  let abort_signal = AbortSignal::new();
  let obj = cppgc::make_cppgc_empty_object::<AbortSignal>(scope);
  cppgc::wrap_object2(scope, obj, (event_target, abort_signal))
}

#[op2]
pub fn op_event_create_dependent_abort_signal<'a>(
  scope: &mut v8::HandleScope<'a>,
  signals: v8::Local<'a, v8::Array>,
  #[string] prefix: String,
) -> Result<v8::Local<'a, v8::Object>, EventError> {
  let context = || "Argument 1".into();
  let signals = Vec::<v8::Local<'a, v8::Value>>::convert(
    scope,
    signals.into(),
    prefix.clone().into(),
    context.into(),
    &Default::default(),
  )?;
  let event_target = EventTarget::new();
  let obj = cppgc::make_cppgc_empty_object::<AbortSignal>(scope);
  let abort_signal = AbortSignal::new_with_dependent(
    scope,
    obj,
    signals,
    prefix.into(),
    context.into(),
  )?;
  Ok(cppgc::wrap_object2(
    scope,
    obj,
    (event_target, abort_signal),
  ))
}

#[op2]
pub fn op_event_add_abort_algorithm(
  #[cppgc] signal: &AbortSignal,
  #[global] algorithm: v8::Global<v8::Function>,
) {
  signal.add(algorithm);
}

#[op2]
pub fn op_event_remove_abort_algorithm(
  #[cppgc] signal: &AbortSignal,
  #[global] algorithm: v8::Global<v8::Function>,
) {
  signal.remove(algorithm);
}

#[op2(fast)]
pub fn op_event_signal_abort<'a>(
  scope: &mut v8::HandleScope<'a>,
  state: Rc<RefCell<OpState>>,
  signal_object: v8::Local<'a, v8::Object>,
  reason: Option<v8::Local<'a, v8::Value>>,
) {
  let signal = cppgc::try_unwrap_cppgc_proto_object::<AbortSignal>(
    scope,
    signal_object.into(),
  )
  .unwrap();
  signal.signal_abort(scope, &state, signal_object, reason);
}

#[op2]
pub fn op_event_get_source_signals<'a>(
  scope: &mut v8::HandleScope<'a>,
  #[cppgc] signal: &AbortSignal,
) -> v8::Local<'a, v8::Array> {
  let mut elements = Vec::new();
  let source_signals = signal.source_signals.borrow();
  for source_signal in source_signals.iter() {
    if let Some(source_signal) = source_signal.to_local(scope) {
      elements.push(source_signal);
    }
  }
  v8::Array::new_with_elements(scope, &elements)
}

#[op2]
pub fn op_event_get_dependent_signals<'a>(
  scope: &mut v8::HandleScope<'a>,
  #[cppgc] signal: &AbortSignal,
) -> v8::Local<'a, v8::Array> {
  let mut elements = Vec::new();
  let dependent_signals = signal.dependent_signals.borrow();
  for dependent_signal in dependent_signals.iter() {
    if let Some(source_signal) = dependent_signal.to_local(scope) {
      elements.push(source_signal);
    }
  }
  v8::Array::new_with_elements(scope, &elements)
}
