use deno_core::v8;
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_int as int;
use std::ptr::null_mut;

#[repr(C)]
#[derive(Debug)]
pub struct TCCState {
  _unused: [u8; 0],
}
pub const TCC_OUTPUT_MEMORY: int = 1;

extern "C" {
  pub fn tcc_new() -> *mut TCCState;
  pub fn tcc_set_options(s: *mut TCCState, str: *const ::std::os::raw::c_char);
  pub fn tcc_compile_string(
    s: *mut TCCState,
    buf: *const ::std::os::raw::c_char,
  ) -> ::std::os::raw::c_int;
  pub fn tcc_add_symbol(
    s: *mut TCCState,
    name: *const ::std::os::raw::c_char,
    val: *const ::std::os::raw::c_void,
  ) -> ::std::os::raw::c_int;
  pub fn tcc_set_output_type(
    s: *mut TCCState,
    output_type: ::std::os::raw::c_int,
  ) -> ::std::os::raw::c_int;
  pub fn tcc_relocate(
    s1: *mut TCCState,
    ptr: *mut ::std::os::raw::c_void,
  ) -> ::std::os::raw::c_int;
  pub fn tcc_get_symbol(
    s: *mut TCCState,
    name: *const ::std::os::raw::c_char,
  ) -> *mut ::std::os::raw::c_void;
}

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
  };
}

fn native_to_c(ty: &crate::NativeType) -> &'static str {
  match ty {
    crate::NativeType::Void => "void",
    crate::NativeType::U8 => "unsigned char",
    crate::NativeType::U16 => "unsigned short",
    crate::NativeType::U32 => "unsigned int",
    crate::NativeType::U64 => "unsigned long",
    crate::NativeType::USize => "unsigned long",
    crate::NativeType::I8 => "char",
    crate::NativeType::I16 => "short",
    crate::NativeType::I32 => "int",
    crate::NativeType::I64 => "long",
    crate::NativeType::ISize => "long",
    crate::NativeType::F32 => "float",
    crate::NativeType::F64 => "double",
    crate::NativeType::Pointer => "void*",
    crate::NativeType::Function => "void*",
  }
}

pub(crate) unsafe fn create_func<'s>(
  _scope: &mut v8::HandleScope<'s>,
  sym: &crate::Symbol,
) -> extern "C" fn(*const v8::FunctionCallbackInfo) {
  let ctx = tcc_new();

  tcc_set_options(ctx, cstr!("-nostdlib").as_ptr());
  tcc_set_output_type(ctx, TCC_OUTPUT_MEMORY);
  tcc_add_symbol(
    ctx,
    cstr!("v8__FunctionCallbackInfo__GetReturnValue").as_ptr(),
    v8__FunctionCallbackInfo__GetReturnValue as *const c_void,
  );
  tcc_add_symbol(
    ctx,
    cstr!("v8__FunctionCallbackInfo__GetArgument").as_ptr(),
    v8__FunctionCallbackInfo__GetArgument as *const c_void,
  );
  tcc_add_symbol(
    ctx,
    cstr!("v8__ReturnValue__Set").as_ptr(),
    v8__ReturnValue__Set as *const c_void,
  );
  tcc_add_symbol(ctx, cstr!("func").as_ptr(), sym.ptr.0);

  let mut code = String::from(
    r#"
extern void v8__ReturnValue__Set(void* rv, void* nv);
extern void* v8__FunctionCallbackInfo__GetReturnValue(void* info);
extern void* v8__FunctionCallbackInfo__GetArgument(void* info, int i);

"#,
  );

  code += "extern ";
  let result_c_type = native_to_c(&sym.result_type);
  code += result_c_type;
  code += " func(";
  for (i, param) in sym.parameter_types.iter().enumerate() {
    if i != 0 {
      code += ", ";
    }
    let ty = native_to_c(param);
    code += ty;
    code += &format!(" p{i}");
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
  tcc_compile_string(ctx, cstr!(code).as_ptr());
  // pass null ptr to get required length
  let len = tcc_relocate(ctx, null_mut());
  assert!(len != -1);
  let mut bin = Vec::with_capacity(len as usize);
  let ret = tcc_relocate(ctx, bin.as_mut_ptr() as *mut c_void);
  assert!(ret == 0);
  bin.set_len(len as usize);

  let addr = tcc_get_symbol(ctx, cstr!("main").as_ptr());
  let func = std::mem::transmute(addr);
  Box::leak(Box::new(ctx));
  func
}
