#[derive(Debug)]
pub struct Error(String);

impl From<dlopen::Error> for Error {
  fn from(e: dlopen::Error) -> Self {
    match e {
      #[cfg(target_os = "windows")]
      // This calls FormatMessageW and tells it to *not*
      // ignore the insert sequences. Unlike libstd
      // which passes the FORMAT_MESSAGE_IGNORE_INSERTS
      // flag.
      //
      // https://github.com/denoland/deno/issues/11632
      dlopen::Error::OpeningLibraryError(e) => {
        use winapi::shared::minwindef::DWORD;
        use winapi::shared::ntdef::WCHAR;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::winbase::FormatMessageW;
        use winapi::um::winbase::FORMAT_MESSAGE_FROM_SYSTEM;

        let error_num = e.raw_os_error().unwrap();

        // Language ID given by
        // MAKELANGID(LANG_SYSTEM_DEFAULT, SUBLANG_SYS_DEFAULT) as DWORD;
        let langId = 0x0800 as DWORD;

        let mut buf = [0 as WCHAR; 2048];

        unsafe {
          let length = FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM,
            ptr::null_mut(),
            errnum as DWORD,
            langId as DWORD,
            buf.as_mut_ptr(),
            buf.len() as DWORD,
            std::ptr::null_mut(),
          );

          if length == 0 {
            // Language ID is wrong?
            let err_num = unsafe { GetLastError() };
            return format!("FormatMessageW failed with error {}", err_num,);
          }

          let msg = String::from_utf16_lossy(&buf[..length as usize]);
          Self(msg)
        }
      }
      _ => Self(e.to_string()),
    }
  }
}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl std::error::Error for Error {}
