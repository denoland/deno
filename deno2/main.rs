extern crate libc;
use libc::c_char;
use libc::c_int;
use std::ffi::CStr;
use std::ffi::CString;

#[link(name = "deno", kind = "static")]
extern "C" {
    fn deno_v8_version() -> *const c_char;
    fn deno_init();
    fn deno_set_flags(argc: *mut c_int, argv: *mut *mut c_char);
}

// Pass the command line arguments to v8.
// Returns a vector of command line arguments that v8 did not understand.
fn set_flags() -> Vec<String> {
    // deno_set_flags(int* argc, char** argv) mutates argc and argv to remove
    // flags that v8 understands.
    // Convert command line arguments to a vector of C strings.
    let mut argv = std::env::args()
        .map(|arg| CString::new(arg).unwrap().into_bytes_with_nul())
        .collect::<Vec<_>>();
    // Make a new array, that can be modified by V8::SetFlagsFromCommandLine(),
    // containing mutable raw pointers to the individual command line args.
    let mut c_argv = argv.iter_mut()
        .map(|arg| arg.as_mut_ptr() as *mut i8)
        .collect::<Vec<_>>();
    // Store the length of the argv array in a local variable. We'll pass a
    // pointer to this local variable to deno_set_flags(), which then
    // updates its value.
    let mut c_argc = argv.len() as c_int;
    // Let v8 parse the arguments it recognizes and remove them from c_argv.
    unsafe {
        deno_set_flags(&mut c_argc, c_argv.as_mut_ptr());
    };
    // If c_argc was updated we have to change the length of c_argv to match.
    c_argv.truncate(c_argc as usize);
    // Copy the modified arguments list into a proper rust vec and return it.
    c_argv
        .iter()
        .map(|ptr| unsafe {
            let cstr = CStr::from_ptr(*ptr as *const i8);
            let slice = cstr.to_str().unwrap();
            slice.to_string()
        })
        .collect::<Vec<_>>()
}

fn main() {
    println!("Hi");
    let args = set_flags();
    unsafe { deno_init() };
    let v = unsafe { deno_v8_version() };
    let c_str = unsafe { CStr::from_ptr(v) };
    let version = c_str.to_str().unwrap();
    println!("version: {}", version);
    for arg in args {
        println!("arg: {}", arg);
    }
}
