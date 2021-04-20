use rusty_v8 as v8;
use std::cell::RefCell;

pub struct SerializedValue {
  pub buf: RefCell<Vec<u8>>
}

impl serde_v8::Deserializable for SerializedValue {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, serde_v8::Error> {
    // TODO: error handling
    let buf = RefCell::new(match serialize(scope, value) {
      Some(buf) => buf,
      None => vec![],
    });
    Ok(Self { buf })
  }
}

impl serde_v8::Serializable for SerializedValue {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, serde_v8::Error> {
    let buf = self.buf.take();
    let val = deserialize(scope, buf);
    
    match val {
      Some(val) => Ok(val),
      None => Err(serde_v8::Error::Message("invalid SerializedValue".to_string()))
    }
  }
}

struct SerializeDeserialize {}

impl v8::ValueSerializerImpl for SerializeDeserialize {
  #[allow(unused_variables)]
  fn throw_data_clone_error<'s>(
    &mut self,
    scope: &mut v8::HandleScope<'s>,
    message: v8::Local<'s, v8::String>,
  ) {
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
  }
}

impl v8::ValueDeserializerImpl for SerializeDeserialize {}

// essentially from_v8
fn serialize(
  scope: &mut v8::HandleScope,
  value: v8::Local<v8::Value>,
) -> Option<Vec<u8>> {
  let sd = Box::new(SerializeDeserialize {});
  let mut vs = v8::ValueSerializer::new(scope, sd);
  
  match vs.write_value(scope.get_current_context(), value) {
    Some(true) => Some(vs.release()),
    _ => None
  }
}

// essentially to_v8
fn deserialize<'a>(
  scope: &mut v8::HandleScope<'a>,
  buf: Vec<u8>,
) -> Option<v8::Local<'a, v8::Value>> {
  let sd = Box::new(SerializeDeserialize {});
  let mut vd = v8::ValueDeserializer::new(scope, sd, &buf);
  
  vd.read_value(scope.get_current_context())
}
