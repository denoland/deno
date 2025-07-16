// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::rc::Rc;

use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::cppgc;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::Global;
use deno_core::webidl::Nullable;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum EventError {
  #[class(type)]
  #[error("parameter 2 is not of type 'Object'")]
  InvalidListenerType,
  #[class(type)]
  #[error("parameter 1 is expected Event")]
  ExpectedEvent,
  #[class("DOMExceptionInvalidStateError")]
  #[error("Invalid event state")]
  InvalidState,
  #[class(generic)]
  #[error(transparent)]
  DataError(#[from] v8::DataError),
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
  // https://dom.spec.whatwg.org/#concept-event-dispatch
  fn dispatch<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
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
    for (path_index, path) in self.path.borrow().iter().enumerate().rev() {
      if path.shadow_adjusted_target.is_none() {
        self.event_phase.replace(EventPhase::CapturingPhase);
        self.invoke(
          scope,
          event_object,
          target,
          target_object.clone(),
          path_index,
          InvokePhase::Capturing,
        );
      }
    }

    // 6.14.
    for (path_index, path) in self.path.borrow().iter().enumerate() {
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
        event_object,
        target,
        target_object.clone(),
        path_index,
        InvokePhase::Bubbling,
      );
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
    event_object: v8::Local<'a, v8::Object>,
    target: &EventTarget,
    target_object: v8::Global<v8::Object>,
    path_index: usize,
    phase: InvokePhase,
  ) {
    let path = self.path.borrow();

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
    // Against the spec, clone event listeners in inner_invoke
    let typ = self.typ.borrow();
    let mut listeners = target.listeners.borrow_mut();
    let Some(listeners) = listeners.get_mut(&*typ) else {
      return;
    };

    // 8.
    let _ = self.inner_invoke(
      scope,
      event_object,
      target_object.clone(),
      listeners,
      phase,
    );
  }

  // https://dom.spec.whatwg.org/#concept-event-listener-inner-invoke
  #[inline]
  fn inner_invoke<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    event_object: v8::Local<'a, v8::Object>,
    target_object: v8::Global<v8::Object>,
    listeners: &mut Vec<Rc<EventListener>>,
    phase: InvokePhase,
  ) -> bool {
    // NOTE: Omit implementations for window.event (current event)

    // 1.
    let mut found = false;

    // 2.
    // Clone event listeners before iterating since the list can be modified during the iteration.
    for listener in listeners.clone().iter() {
      // Check if the event listener has been removed since the listeners has been cloned.
      if !listeners.iter().any(|l| Rc::ptr_eq(l, listener)) {
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
            // TODO(petamoriken): report exception
          }
        }
      }

      // 11.1.
      if let Some(exception) = scope.exception() {
        // TODO(petamoriken): report exception
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

#[op2]
impl Event {
  #[constructor]
  #[required(1)]
  #[cppgc]
  fn new<'a>(
    #[string] typ: String,
    #[webidl] init: Nullable<EventInit>,
  ) -> Event {
    let (bubbles, cancelable, composed) = if let Some(init) = init.into_option()
    {
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

  // legacy
  #[required(1)]
  fn init_event<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[string] typ: String,
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
  #[global] target_object: v8::Global<v8::Object>,
  event_object: v8::Local<'a, v8::Object>,
  #[global] target_override: Option<v8::Global<v8::Object>>,
) -> bool {
  let target = v8::Local::new(scope, &target_object);
  let target =
    cppgc::try_unwrap_cppgc_object::<EventTarget>(scope, target.into())
      .unwrap();
  let event =
    cppgc::try_unwrap_cppgc_object::<Event>(scope, event_object.into())
      .unwrap();
  event.dispatch(scope, event_object, &target, target_object, target_override)
}

// TODO(petamorken): list
// report error
// CloseEvent
// ErrorEvent
// MessageEvent
// PromiseRejectionEvent
// CustomEvent
// ProgressEvent

#[derive(Debug)]
struct EventListener {
  callback: v8::Global<v8::Object>,
  capture: bool,
  passive: bool,
  once: bool,
  signal: Option<v8::Global<v8::Object>>,
  // This field exists for simulating Node.js behavior, implemented in https://github.com/nodejs/node/commit/bcd35c334ec75402ee081f1c4da128c339f70c24
  // Some internal event listeners in Node.js can ignore `e.stopImmediatePropagation()` calls　from the earlier event listeners.
  resist_stop_immediate_propagation: bool,
}

#[derive(Debug)]
pub struct EventTarget {
  listeners: RefCell<HashMap<String, Vec<Rc<EventListener>>>>,
}

impl GarbageCollected for EventTarget {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"EventTarget"
  }
}

#[op2]
impl EventTarget {
  #[constructor]
  #[cppgc]
  fn new(_: bool) -> EventTarget {
    EventTarget {
      listeners: RefCell::new(HashMap::new()),
    }
  }

  #[fast]
  #[required(2)]
  #[undefined]
  fn add_event_listener<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[string] typ: String,
    callback: Option<v8::Local<'a, v8::Value>>,
    options: Option<v8::Local<'a, v8::Value>>,
  ) -> Result<(), EventError> {
    let (
      capture,
      passive,
      once,
      resist_stop_immediate_propagation,
      /* signal */
    ) = match options {
      Some(options) => {
        if options.is_object()
          && let Some(options) = options.to_object(scope)
        {
          #[inline]
          fn to_bool<'a>(
            scope: &mut v8::HandleScope<'a>,
            options: v8::Local<'a, v8::Object>,
            str: &'static str,
            is_symbol: bool,
          ) -> bool {
            let str = v8::String::new(scope, str).unwrap();
            let key: v8::Local<v8::Value> = if is_symbol {
              v8::Symbol::for_key(scope, str).into()
            } else {
              str.into()
            };
            match options.get(scope, key) {
              Some(value) => value.to_boolean(scope).is_true(),
              None => false,
            }
          }

          let capture = to_bool(scope, options, "capture", false);
          let passive = to_bool(scope, options, "passive", false);
          let once = to_bool(scope, options, "once", false);
          let resist_stop_immediate_propagation =
            to_bool(scope, options, "Deno.stopImmediatePropagation", true);
          (capture, passive, once, resist_stop_immediate_propagation)
        } else {
          (options.to_boolean(scope).is_true(), false, false, false)
        }
      }
      None => (false, false, false, false),
    };

    // TODO(petamoriken): signal have already aborted

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
        callback.to_object(scope).unwrap()
      }
    };

    let mut listeners = self.listeners.borrow_mut();
    let listeners = listeners.entry(typ.clone()).or_default();
    for listener in listeners.iter() {
      if listener.capture == capture && listener.callback == callback {
        return Ok(());
      }
    }

    // TODO(petamoriken): add signal listeners

    listeners.push(Rc::new(EventListener {
      callback: Global::new(scope, callback),
      capture,
      passive,
      once,
      signal: None,
      resist_stop_immediate_propagation,
    }));

    Ok(())
  }

  #[fast]
  #[required(2)]
  #[undefined]
  fn remove_event_listener<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[string] typ: String,
    callback: Option<v8::Local<'a, v8::Object>>,
    options: Option<v8::Local<'a, v8::Value>>,
  ) {
    let capture = match options {
      Some(options) => {
        if options.is_object()
          && let Some(options) = options.to_object(scope)
        {
          let key = v8::String::new(scope, "capture").unwrap();
          match options.get(scope, key.into()) {
            Some(value) => value.to_boolean(scope).is_true(),
            None => false,
          }
        } else {
          options.to_boolean(scope).is_true()
        }
      }
      None => false,
    };

    let callback = match callback {
      None => {
        return;
      }
      Some(callback) => {
        if callback.is_null() {
          return;
        }
        callback
      }
    };

    let mut listeners = self.listeners.borrow_mut();
    let Some(listeners) = listeners.get_mut(&typ) else {
      return;
    };
    if let Some(index) = listeners.iter().position(|listener| {
      listener.capture == capture && listener.callback == callback
    }) {
      listeners.remove(index);
    }
  }

  #[fast]
  #[reentrant]
  #[required(1)]
  fn dispatch_event<'a>(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::HandleScope<'a>,
    event_object: v8::Local<'a, v8::Object>,
  ) -> Result<bool, EventError> {
    let Some(event) =
      cppgc::try_unwrap_cppgc_object::<Event>(scope, event_object.into())
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

    let listeners = self.listeners.borrow();
    if listeners.get(&*typ).is_none() {
      event.target.replace(Some(this));
      return Ok(true);
    };

    if event.dispatch_flag.get()
      || !matches!(*event.event_phase.borrow(), EventPhase::None)
    {
      return Err(EventError::InvalidState);
    }

    Ok(event.dispatch(scope, event_object, self, this, None))
  }
}

#[op2(fast)]
pub fn op_event_wrap_event_target<'a>(
  scope: &mut v8::HandleScope<'a>,
  obj: v8::Local<'a, v8::Object>,
) {
  cppgc::wrap_object(
    scope,
    obj,
    EventTarget {
      listeners: RefCell::new(HashMap::new()),
    },
  );
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
