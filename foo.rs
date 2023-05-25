#[instrument(level = "trace", skip_all)]
pub fn transform(
  src: String,
  is_module: bool,
  options: JsBuffer,
  signal: Option<AbortSignal>,
) -> napi::Result<AsyncTask<TransformTask>> {
  crate::util::init_default_trace_subscriber();
  let c = get_compiler();
  eprintln!("js buffer");
  let input = if is_module {
    Input::Program(src)
  } else {
    Input::Source { src }
  };
  let task = TransformTask {
    c,
    input,
    options: options.into_ref()?,
  };
  Ok(AsyncTask::with_optional_signal(task, signal))
}
#[instrument(level = "trace", skip_all)]
#[doc(hidden)]
#[allow(non_snake_case)]
#[allow(clippy::all)]
extern "C" fn __napi__transform(
  env: napi::bindgen_prelude::sys::napi_env,
  cb: napi::bindgen_prelude::sys::napi_callback_info,
) -> napi::bindgen_prelude::sys::napi_value {
  unsafe {
    napi::bindgen_prelude::CallbackInfo::<4usize>::new(env, cb, None)
      .and_then(|mut cb| {
        let arg0 = {
          <String as napi::bindgen_prelude::FromNapiValue>::from_napi_value(
            env,
            cb.get_arg(0usize),
          )?
        };
        let arg1 = {
          <bool as napi::bindgen_prelude::FromNapiValue>::from_napi_value(
            env,
            cb.get_arg(1usize),
          )?
        };
        let arg2 = {
          <JsBuffer as napi::bindgen_prelude::FromNapiValue>::from_napi_value(
            env,
            cb.get_arg(2usize),
          )?
        };
        let arg3 = {
          < Option < AbortSignal > as napi :: bindgen_prelude ::
                FromNapiValue > :: from_napi_value(env, cb.get_arg(3usize)) ?
        };
        napi::bindgen_prelude::within_runtime_if_available(move || {
          let _ret = { transform(arg0, arg1, arg2, arg3) };
          match _ret {
            Ok(value) => {
              napi::bindgen_prelude::ToNapiValue::to_napi_value(env, value)
            }
            Err(err) => {
              napi::bindgen_prelude::JsError::from(err).throw_into(env);
              Ok(std::ptr::null_mut())
            }
          }
        })
      })
      .unwrap_or_else(|e| {
        napi::bindgen_prelude::JsError::from(e).throw_into(env);
        std::ptr::null_mut::<napi::bindgen_prelude::sys::napi_value__>()
      })
  }
}
