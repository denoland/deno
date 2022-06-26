use libtcc as tcc;
use std::ffi::c_void;
use deno_core::v8;
use std::ffi::CString;
use std::os::raw::c_int as int;

extern "C" {
  fn v8__FunctionCallbackInfo__GetReturnValue(
    info: *const v8::FunctionCallbackInfo,
  ) -> *mut v8::Value;

  fn v8__FunctionCallbackInfo__GetArgument(
    this: *const v8::FunctionCallbackInfo,
    i: int,
  ) -> *const v8::Value;
  
  fn v8__ReturnValue__Set(this: *mut v8::ReturnValue, value: *const v8::Value);
}

macro_rules! cstr {
  ($st:expr) => {
    &CString::new($st).unwrap()
  }
}

fn native_to_c(ty: &crate::NativeType) -> &'static str {
  match ty {
    &crate::NativeType::Void => "void",
    &crate::NativeType::U8 => "unsigned char",
    &crate::NativeType::U16 => "unsigned short",
    &crate::NativeType::U32 => "unsigned int",
    &crate::NativeType::U64 => "unsigned long",
    &crate::NativeType::USize => "unsigned long",
    &crate::NativeType::I8 => "char",
    &crate::NativeType::I16 => "short",
    &crate::NativeType::I32 => "int",
    &crate::NativeType::I64 => "long",
    &crate::NativeType::ISize => "long",
    &crate::NativeType::F32 => "float",
    &crate::NativeType::F64 => "double",
    &crate::NativeType::Pointer => "void*",
    &crate::NativeType::Function => "void*",
  }
}

pub(crate) unsafe fn create_func<'s>(
  _scope: &mut v8::HandleScope<'s>,
  sym: &crate::Symbol,
) -> extern "C" fn(*const v8::FunctionCallbackInfo) {
  let mut guard = tcc::Guard::new().unwrap();
  let mut ctx = tcc::Context::new(&mut guard).unwrap();

  ctx.set_options(cstr!("-nostdlib"));
  ctx.set_output_type(tcc::OutputType::Memory);

  ctx.add_symbol(cstr!("v8__FunctionCallbackInfo__GetReturnValue"), v8__FunctionCallbackInfo__GetReturnValue as *const c_void);
  ctx.add_symbol(cstr!("v8__FunctionCallbackInfo__GetArgument"), v8__FunctionCallbackInfo__GetArgument as *const c_void);
  ctx.add_symbol(cstr!("v8__ReturnValue__Set"), v8__ReturnValue__Set as *const c_void);
  ctx.add_symbol(cstr!("func"), sym.ptr.0);

  let mut code = String::from(r#"
extern void v8__ReturnValue__Set(void* rv, void* nv);
extern void* v8__FunctionCallbackInfo__GetReturnValue(void* info);
extern void* v8__FunctionCallbackInfo__GetArgument(void* info, int i);

"#);

  code += "extern ";
  let result_c_type = native_to_c(&sym.result_type);
  code += result_c_type;
  code += " func(";
  let mut i = 0;
  for param in &sym.parameter_types {
    if i != 0 {
      code += ", ";
    }
    let ty = native_to_c(param);
    code += ty;
    code += &format!(" p{i}");
    i += 1;
  }
  code += ");\n\n";

  code += "void main(void* info) {\n";

  // TODO parameter conv

  match sym.result_type {
    crate::NativeType::Void => {
      code += "  func(";
    }
    _ => {
      code += result_c_type;
      code += "  r = func(";
    }
  }

  for i in 0..sym.parameter_types.len() {
    if i != 0 {
      code += ", ";
    }
    code += &format!("p{i}");
  }

  code += ");\n";

  match sym.result_type {
    crate::NativeType::Void => {}
    _ => {
      code += "  void* rv = v8__FunctionCallbackInfo__GetReturnValue(info);\n";
    }
  }

  code += "}\n";

  println!("{}", code);
  ctx.compile_string(cstr!(code)).unwrap();

  let mut ctx = ctx.relocate().unwrap();
  let func = std::mem::transmute(ctx.get_symbol(cstr!("main")).unwrap());
  Box::leak(Box::new(ctx));
  func
}
