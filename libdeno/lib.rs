use std::sync::Arc;
use std::ffi::CStr;
use std::os::raw::c_char;

use libdeno::LibWorkerFactoryRoots;
use libdeno::create_and_run_current_thread_with_maybe_metrics;
use libdeno::wait_for_start;
use libdeno::tools::run::eval_command;
use libdeno::args::EvalFlags;
use libdeno::args::Flags;
use libdeno::init_v8;

#[unsafe(no_mangle)]
pub extern "C" fn deno_embedded_eval(code: *const c_char) -> i32 {
    if code.is_null() {
        return -1;
    }

    let c_str = unsafe { CStr::from_ptr(code) };
    
    let code = match c_str.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -2,
    };

    // TODO get from args
    let flags = Arc::new(Flags::default());

    let args: Vec<_> = std::env::args_os().collect();

    let fut = async move {
      let roots = LibWorkerFactoryRoots::default();
      
      #[cfg(unix)]
      let (waited_unconfigured_runtime, _waited_args, _waited_cwd) =
        match wait_for_start(&args, roots.clone()) {
          Some(f) => match f.await {
            Ok(v) => match v {
              Some((u, a, c)) => (Some(u), Some(a), Some(c)),
              None => (None, None, None),
            },
            Err(e) => {
              panic!("Failure from control sock: {e}");
            }
          },
          None => (None, None, None),
        };

      #[cfg(not(unix))]
      let (waited_unconfigured_runtime, waited_args, waited_cwd) = (None, None, None);

      // let args = waited_args.unwrap_or(args);
      // let initial_cwd = waited_cwd.map(Some).unwrap_or_else(|| {
      //   match std::env::current_dir().with_context(|| "Failed getting cwd.") {
      //     Ok(cwd) => Some(cwd),
      //     Err(err) => {
      //       log::error!("Failed getting cwd: {err}");
      //       None
      //     }
      //   }
      // });

      if waited_unconfigured_runtime.is_none() {
        init_v8(&flags);
      }

      println!("Starting command {}", code);

      eval_command(flags, EvalFlags{
        print: false,
        code,
      }).await
    };

    
    match create_and_run_current_thread_with_maybe_metrics(fut){
        Ok(exit_code) => exit_code,
        Err(err) => {
          eprintln!("{}", err);
          1
        },
    }
}
