// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use std::{any::Any, borrow::Cow, ffi::c_void, path::PathBuf, rc::Rc};

use deno_core::{
  error::AnyError,
  serde_json::{self, json, Value},
  OpState, Resource, ZeroCopyBuf,
};
use dlopen::symbor::Library;
use libffi::high as ffi;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadLibarayArgs {
  filename: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallParam {
  type_name: String,
  value: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallLibarayFfiArgs {
  rid: u32,
  name: String,
  params: Vec<CallParam>,
  return_type: String,
}

struct DylibResource {
  lib: Rc<Library>,
}

impl Resource for DylibResource {
  fn name(&self) -> Cow<str> {
    "dylib".into()
  }
}

impl DylibResource {
  fn new(lib: &Rc<Library>) -> Self {
    Self { lib: lib.clone() }
  }
}

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_load_libaray", op_load_libaray);
  super::reg_json_sync(rt, "op_call_libaray_ffi", op_call_libaray_ffi);
}

fn op_load_libaray(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: LoadLibarayArgs = serde_json::from_value(args)?;
  let filename = PathBuf::from(&args.filename);

  debug!("Loading Libaray: {:#?}", filename);
  let dy_lib = Library::open(filename).map(Rc::new)?;
  let dylib_resource = DylibResource::new(&dy_lib);

  let rid = state.resource_table.add(dylib_resource);

  Ok(json!(rid))
}

fn op_call_libaray_ffi(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CallLibarayFfiArgs = serde_json::from_value(args)?;
  let mut call_args: Vec<(Box<dyn Any>, String)> = vec![];

  args
    .params
    .iter()
    .for_each(|param| match param.type_name.as_str() {
      "i8" => {
        let val = param.value.as_i64().unwrap() as i8;
        call_args.push((Box::new(val), param.type_name.clone()));
      }
      "i16" => {
        let val = param.value.as_i64().unwrap() as i16;
        call_args.push((Box::new(val), param.type_name.clone()));
      }
      "i32" => {
        let val = param.value.as_i64().unwrap() as i32;
        call_args.push((Box::new(val), param.type_name.clone()));
      }
      "i64" => {
        let val = param.value.as_i64().unwrap();
        call_args.push((Box::new(val), param.type_name.clone()));
      }
      "u8" => {
        let val = param.value.as_u64().unwrap() as u8;
        call_args.push((Box::new(val), param.type_name.clone()));
      }
      "u32" => {
        let val = param.value.as_u64().unwrap() as u32;
        call_args.push((Box::new(val), param.type_name.clone()));
      }
      "u64" => {
        let val = param.value.as_u64().unwrap();
        call_args.push((Box::new(val), param.type_name.clone()));
      }
      "f32" => {
        let val = param.value.as_f64().unwrap() as f32;
        call_args.push((Box::new(val), param.type_name.clone()));
      }
      "f64" => {
        let val = param.value.as_f64().unwrap();
        call_args.push((Box::new(val), param.type_name.clone()));
      }
      _ => {}
    });

  let params: Vec<ffi::Arg> = call_args
    .iter()
    .map(|param| {
      let (val, name) = param;
      match name.as_str() {
        "i8" => {
          let v = val.downcast_ref::<i8>().unwrap();
          ffi::arg(&*v)
        }
        "i16" => {
          let v = val.downcast_ref::<i16>().unwrap();
          ffi::arg(&*v)
        }
        "i32" => {
          let v = val.downcast_ref::<i32>().unwrap();
          ffi::arg(&*v)
        }
        "i64" => {
          let v = val.downcast_ref::<i64>().unwrap();
          ffi::arg(&*v)
        }
        "u8" => {
          let v = val.downcast_ref::<u8>().unwrap();
          ffi::arg(&*v)
        }
        "u16" => {
          let v = val.downcast_ref::<u16>().unwrap();
          ffi::arg(&*v)
        }
        "u32" => {
          let v = val.downcast_ref::<u32>().unwrap();
          ffi::arg(&*v)
        }
        "u64" => {
          let v = val.downcast_ref::<u64>().unwrap();
          ffi::arg(&*v)
        }
        "f32" => {
          let v = val.downcast_ref::<f32>().unwrap();
          ffi::arg(&*v)
        }
        "f64" => {
          let v = val.downcast_ref::<f64>().unwrap();
          ffi::arg(&*v)
        }
        _ => ffi::arg(&()),
      }
    })
    .collect();

  let lib = state
    .resource_table
    .get::<DylibResource>(args.rid)
    .unwrap()
    .lib
    .clone();

  let fn_ptr: *const c_void = *unsafe { lib.symbol(&args.name) }?;
  let fn_code_ptr = ffi::CodePtr::from_ptr(fn_ptr);

  let ret = match args.return_type.as_str() {
    "i8" => {
      json!(unsafe { ffi::call::<i8>(fn_code_ptr, params.as_slice()) })
    }
    "i16" => {
      json!(unsafe { ffi::call::<i16>(fn_code_ptr, params.as_slice()) })
    }
    "i32" => {
      json!(unsafe { ffi::call::<i32>(fn_code_ptr, params.as_slice()) })
    }
    "i64" => {
      json!(unsafe { ffi::call::<i32>(fn_code_ptr, params.as_slice()) })
    }
    "u8" => {
      json!(unsafe { ffi::call::<u8>(fn_code_ptr, params.as_slice()) })
    }
    "u16" => {
      json!(unsafe { ffi::call::<u16>(fn_code_ptr, params.as_slice()) })
    }
    "u32" => {
      json!(unsafe { ffi::call::<u32>(fn_code_ptr, params.as_slice()) })
    }
    "u64" => {
      json!(unsafe { ffi::call::<u64>(fn_code_ptr, params.as_slice()) })
    }
    "f32" => {
      json!(unsafe { ffi::call::<f32>(fn_code_ptr, params.as_slice()) })
    }
    "f64" => {
      json!(unsafe { ffi::call::<f64>(fn_code_ptr, params.as_slice()) })
    }
    _ => {
      unsafe { ffi::call::<()>(fn_code_ptr, params.as_slice()) };
      json!(null)
    }
  };

  Ok(ret)
}
