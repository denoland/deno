use crate::generate;
use crate::types::*;

generate! {
  extern "C" {
    fn uv_version() -> ::std::os::raw::c_uint;
    fn uv_version_string() -> *const ::std::os::raw::c_char;
    fn uv_library_shutdown();
    fn uv_replace_allocator(
      malloc_func: uv_malloc_func,
      realloc_func: uv_realloc_func,
      calloc_func: uv_calloc_func,
      free_func: uv_free_func,
    ) -> ::std::os::raw::c_int;
    fn uv_default_loop() -> *mut uv_loop_t;
    fn uv_loop_init(loop_: *mut uv_loop_t) -> ::std::os::raw::c_int;
    fn uv_loop_close(loop_: *mut uv_loop_t) -> ::std::os::raw::c_int;
    fn uv_loop_new() -> *mut uv_loop_t;
    fn uv_loop_delete(arg1: *mut uv_loop_t);
    fn uv_loop_size() -> usize;
    fn uv_loop_alive(loop_: *const uv_loop_t) -> ::std::os::raw::c_int;
    fn uv_loop_fork(loop_: *mut uv_loop_t) -> ::std::os::raw::c_int;
    fn uv_run(arg1: *mut uv_loop_t, mode: uv_run_mode) -> ::std::os::raw::c_int;
    fn uv_stop(arg1: *mut uv_loop_t);
    fn uv_ref(arg1: *mut uv_handle_t);
    fn uv_unref(arg1: *mut uv_handle_t);
    fn uv_has_ref(arg1: *const uv_handle_t) -> ::std::os::raw::c_int;
    fn uv_update_time(arg1: *mut uv_loop_t);
    fn uv_now(arg1: *const uv_loop_t) -> u64;
    fn uv_backend_fd(arg1: *const uv_loop_t) -> ::std::os::raw::c_int;
    fn uv_backend_timeout(arg1: *const uv_loop_t) -> ::std::os::raw::c_int;
    fn uv_translate_sys_error(sys_errno: ::std::os::raw::c_int) -> ::std::os::raw::c_int;
    fn uv_strerror(err: ::std::os::raw::c_int) -> *const ::std::os::raw::c_char;
    fn uv_strerror_r(
      err: ::std::os::raw::c_int,
      buf: *mut ::std::os::raw::c_char,
      buflen: usize,
    ) -> *mut ::std::os::raw::c_char;
    fn uv_err_name(err: ::std::os::raw::c_int) -> *const ::std::os::raw::c_char;
    fn uv_err_name_r(
      err: ::std::os::raw::c_int,
      buf: *mut ::std::os::raw::c_char,
      buflen: usize,
    ) -> *mut ::std::os::raw::c_char;
    fn uv_shutdown(
      req: *mut uv_shutdown_t,
      handle: *mut uv_stream_t,
      cb: uv_shutdown_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_handle_size(type_: uv_handle_type) -> usize;
    fn uv_handle_get_type(handle: *const uv_handle_t) -> uv_handle_type;
    fn uv_handle_type_name(type_: uv_handle_type) -> *const ::std::os::raw::c_char;
    fn uv_handle_get_data(handle: *const uv_handle_t) -> *mut ::std::os::raw::c_void;
    fn uv_handle_get_loop(handle: *const uv_handle_t) -> *mut uv_loop_t;
    fn uv_handle_set_data(handle: *mut uv_handle_t, data: *mut ::std::os::raw::c_void);
    fn uv_req_size(type_: uv_req_type) -> usize;
    fn uv_req_get_data(req: *const uv_req_t) -> *mut ::std::os::raw::c_void;
    fn uv_req_set_data(req: *mut uv_req_t, data: *mut ::std::os::raw::c_void);
    fn uv_req_get_type(req: *const uv_req_t) -> uv_req_type;
    fn uv_req_type_name(type_: uv_req_type) -> *const ::std::os::raw::c_char;
    fn uv_is_active(handle: *const uv_handle_t) -> ::std::os::raw::c_int;
    fn uv_walk(loop_: *mut uv_loop_t, walk_cb: uv_walk_cb, arg: *mut ::std::os::raw::c_void);
    fn uv_print_all_handles(loop_: *mut uv_loop_t, stream: *mut FILE);
    fn uv_print_active_handles(loop_: *mut uv_loop_t, stream: *mut FILE);
    fn uv_close(handle: *mut uv_handle_t, close_cb: uv_close_cb);
    fn uv_send_buffer_size(
      handle: *mut uv_handle_t,
      value: *mut ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_recv_buffer_size(
      handle: *mut uv_handle_t,
      value: *mut ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_fileno(handle: *const uv_handle_t, fd: *mut uv_os_fd_t) -> ::std::os::raw::c_int;
    fn uv_buf_init(base: *mut ::std::os::raw::c_char, len: ::std::os::raw::c_uint) -> uv_buf_t;
    fn uv_pipe(
      fds: *mut uv_file,
      read_flags: ::std::os::raw::c_int,
      write_flags: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_socketpair(
      type_: ::std::os::raw::c_int,
      protocol: ::std::os::raw::c_int,
      socket_vector: *mut uv_os_sock_t,
      flags0: ::std::os::raw::c_int,
      flags1: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_stream_get_write_queue_size(stream: *const uv_stream_t) -> usize;
    fn uv_listen(
      stream: *mut uv_stream_t,
      backlog: ::std::os::raw::c_int,
      cb: uv_connection_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_accept(server: *mut uv_stream_t, client: *mut uv_stream_t) -> ::std::os::raw::c_int;
    fn uv_read_start(
      arg1: *mut uv_stream_t,
      alloc_cb: uv_alloc_cb,
      read_cb: uv_read_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_read_stop(arg1: *mut uv_stream_t) -> ::std::os::raw::c_int;
    fn uv_write(
      req: *mut uv_write_t,
      handle: *mut uv_stream_t,
      bufs: *const uv_buf_t,
      nbufs: ::std::os::raw::c_uint,
      cb: uv_write_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_write2(
      req: *mut uv_write_t,
      handle: *mut uv_stream_t,
      bufs: *const uv_buf_t,
      nbufs: ::std::os::raw::c_uint,
      send_handle: *mut uv_stream_t,
      cb: uv_write_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_try_write(
      handle: *mut uv_stream_t,
      bufs: *const uv_buf_t,
      nbufs: ::std::os::raw::c_uint,
    ) -> ::std::os::raw::c_int;
    fn uv_try_write2(
      handle: *mut uv_stream_t,
      bufs: *const uv_buf_t,
      nbufs: ::std::os::raw::c_uint,
      send_handle: *mut uv_stream_t,
    ) -> ::std::os::raw::c_int;
    fn uv_is_readable(handle: *const uv_stream_t) -> ::std::os::raw::c_int;
    fn uv_is_writable(handle: *const uv_stream_t) -> ::std::os::raw::c_int;
    fn uv_stream_set_blocking(
      handle: *mut uv_stream_t,
      blocking: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_is_closing(handle: *const uv_handle_t) -> ::std::os::raw::c_int;
    fn uv_tcp_init(arg1: *mut uv_loop_t, handle: *mut uv_tcp_t) -> ::std::os::raw::c_int;
    fn uv_tcp_init_ex(
      arg1: *mut uv_loop_t,
      handle: *mut uv_tcp_t,
      flags: ::std::os::raw::c_uint,
    ) -> ::std::os::raw::c_int;
    fn uv_tcp_open(handle: *mut uv_tcp_t, sock: uv_os_sock_t) -> ::std::os::raw::c_int;
    fn uv_tcp_nodelay(
      handle: *mut uv_tcp_t,
      enable: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_tcp_keepalive(
      handle: *mut uv_tcp_t,
      enable: ::std::os::raw::c_int,
      delay: ::std::os::raw::c_uint,
    ) -> ::std::os::raw::c_int;
    fn uv_tcp_simultaneous_accepts(
      handle: *mut uv_tcp_t,
      enable: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_tcp_bind(
      handle: *mut uv_tcp_t,
      addr: *const sockaddr,
      flags: ::std::os::raw::c_uint,
    ) -> ::std::os::raw::c_int;
    fn uv_tcp_getsockname(
      handle: *const uv_tcp_t,
      name: *mut sockaddr,
      namelen: *mut ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_tcp_getpeername(
      handle: *const uv_tcp_t,
      name: *mut sockaddr,
      namelen: *mut ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_tcp_close_reset(handle: *mut uv_tcp_t, close_cb: uv_close_cb) -> ::std::os::raw::c_int;
    fn uv_tcp_connect(
      req: *mut uv_connect_t,
      handle: *mut uv_tcp_t,
      addr: *const sockaddr,
      cb: uv_connect_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_init(arg1: *mut uv_loop_t, handle: *mut uv_udp_t) -> ::std::os::raw::c_int;
    fn uv_udp_init_ex(
      arg1: *mut uv_loop_t,
      handle: *mut uv_udp_t,
      flags: ::std::os::raw::c_uint,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_open(handle: *mut uv_udp_t, sock: uv_os_sock_t) -> ::std::os::raw::c_int;
    fn uv_udp_bind(
      handle: *mut uv_udp_t,
      addr: *const sockaddr,
      flags: ::std::os::raw::c_uint,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_connect(handle: *mut uv_udp_t, addr: *const sockaddr) -> ::std::os::raw::c_int;
    fn uv_udp_getpeername(
      handle: *const uv_udp_t,
      name: *mut sockaddr,
      namelen: *mut ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_getsockname(
      handle: *const uv_udp_t,
      name: *mut sockaddr,
      namelen: *mut ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_set_membership(
      handle: *mut uv_udp_t,
      multicast_addr: *const ::std::os::raw::c_char,
      interface_addr: *const ::std::os::raw::c_char,
      membership: uv_membership,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_set_source_membership(
      handle: *mut uv_udp_t,
      multicast_addr: *const ::std::os::raw::c_char,
      interface_addr: *const ::std::os::raw::c_char,
      source_addr: *const ::std::os::raw::c_char,
      membership: uv_membership,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_set_multicast_loop(
      handle: *mut uv_udp_t,
      on: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_set_multicast_ttl(
      handle: *mut uv_udp_t,
      ttl: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_set_multicast_interface(
      handle: *mut uv_udp_t,
      interface_addr: *const ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_set_broadcast(
      handle: *mut uv_udp_t,
      on: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_set_ttl(handle: *mut uv_udp_t, ttl: ::std::os::raw::c_int)
      -> ::std::os::raw::c_int;
    fn uv_udp_send(
      req: *mut uv_udp_send_t,
      handle: *mut uv_udp_t,
      bufs: *const uv_buf_t,
      nbufs: ::std::os::raw::c_uint,
      addr: *const sockaddr,
      send_cb: uv_udp_send_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_try_send(
      handle: *mut uv_udp_t,
      bufs: *const uv_buf_t,
      nbufs: ::std::os::raw::c_uint,
      addr: *const sockaddr,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_recv_start(
      handle: *mut uv_udp_t,
      alloc_cb: uv_alloc_cb,
      recv_cb: uv_udp_recv_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_udp_using_recvmmsg(handle: *const uv_udp_t) -> ::std::os::raw::c_int;
    fn uv_udp_recv_stop(handle: *mut uv_udp_t) -> ::std::os::raw::c_int;
    fn uv_udp_get_send_queue_size(handle: *const uv_udp_t) -> usize;
    fn uv_udp_get_send_queue_count(handle: *const uv_udp_t) -> usize;
    fn uv_tty_init(
      arg1: *mut uv_loop_t,
      arg2: *mut uv_tty_t,
      fd: uv_file,
      readable: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_tty_set_mode(arg1: *mut uv_tty_t, mode: uv_tty_mode_t) -> ::std::os::raw::c_int;
    fn uv_tty_reset_mode() -> ::std::os::raw::c_int;
    fn uv_tty_get_winsize(
      arg1: *mut uv_tty_t,
      width: *mut ::std::os::raw::c_int,
      height: *mut ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_tty_set_vterm_state(state: uv_tty_vtermstate_t);
    fn uv_tty_get_vterm_state(state: *mut uv_tty_vtermstate_t) -> ::std::os::raw::c_int;
    fn uv_guess_handle(file: uv_file) -> uv_handle_type;
    fn uv_pipe_init(
      arg1: *mut uv_loop_t,
      handle: *mut uv_pipe_t,
      ipc: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_pipe_open(arg1: *mut uv_pipe_t, file: uv_file) -> ::std::os::raw::c_int;
    fn uv_pipe_bind(
      handle: *mut uv_pipe_t,
      name: *const ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int;
    fn uv_pipe_bind2(
      handle: *mut uv_pipe_t,
      name: *const ::std::os::raw::c_char,
      namelen: usize,
      flags: ::std::os::raw::c_uint,
    ) -> ::std::os::raw::c_int;
    fn uv_pipe_connect(
      req: *mut uv_connect_t,
      handle: *mut uv_pipe_t,
      name: *const ::std::os::raw::c_char,
      cb: uv_connect_cb,
    );
    fn uv_pipe_connect2(
      req: *mut uv_connect_t,
      handle: *mut uv_pipe_t,
      name: *const ::std::os::raw::c_char,
      namelen: usize,
      flags: ::std::os::raw::c_uint,
      cb: uv_connect_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_pipe_getsockname(
      handle: *const uv_pipe_t,
      buffer: *mut ::std::os::raw::c_char,
      size: *mut usize,
    ) -> ::std::os::raw::c_int;
    fn uv_pipe_getpeername(
      handle: *const uv_pipe_t,
      buffer: *mut ::std::os::raw::c_char,
      size: *mut usize,
    ) -> ::std::os::raw::c_int;
    fn uv_pipe_pending_instances(handle: *mut uv_pipe_t, count: ::std::os::raw::c_int);
    fn uv_pipe_pending_count(handle: *mut uv_pipe_t) -> ::std::os::raw::c_int;
    fn uv_pipe_pending_type(handle: *mut uv_pipe_t) -> uv_handle_type;
    fn uv_pipe_chmod(
      handle: *mut uv_pipe_t,
      flags: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_poll_init(
      loop_: *mut uv_loop_t,
      handle: *mut uv_poll_t,
      fd: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_poll_init_socket(
      loop_: *mut uv_loop_t,
      handle: *mut uv_poll_t,
      socket: uv_os_sock_t,
    ) -> ::std::os::raw::c_int;
    fn uv_poll_start(
      handle: *mut uv_poll_t,
      events: ::std::os::raw::c_int,
      cb: uv_poll_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_poll_stop(handle: *mut uv_poll_t) -> ::std::os::raw::c_int;
    fn uv_prepare_init(arg1: *mut uv_loop_t, prepare: *mut uv_prepare_t)
      -> ::std::os::raw::c_int;
    fn uv_prepare_start(prepare: *mut uv_prepare_t, cb: uv_prepare_cb) -> ::std::os::raw::c_int;
    fn uv_prepare_stop(prepare: *mut uv_prepare_t) -> ::std::os::raw::c_int;
    fn uv_check_init(arg1: *mut uv_loop_t, check: *mut uv_check_t) -> ::std::os::raw::c_int;
    fn uv_check_start(check: *mut uv_check_t, cb: uv_check_cb) -> ::std::os::raw::c_int;
    fn uv_check_stop(check: *mut uv_check_t) -> ::std::os::raw::c_int;
    fn uv_idle_init(arg1: *mut uv_loop_t, idle: *mut uv_idle_t) -> ::std::os::raw::c_int;
    fn uv_idle_start(idle: *mut uv_idle_t, cb: uv_idle_cb) -> ::std::os::raw::c_int;
    fn uv_idle_stop(idle: *mut uv_idle_t) -> ::std::os::raw::c_int;
    fn uv_async_init(
      arg1: *mut uv_loop_t,
      async_: *mut uv_async_t,
      async_cb: uv_async_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_async_send(async_: *mut uv_async_t) -> ::std::os::raw::c_int;
    fn uv_timer_init(arg1: *mut uv_loop_t, handle: *mut uv_timer_t) -> ::std::os::raw::c_int;
    fn uv_timer_start(
      handle: *mut uv_timer_t,
      cb: uv_timer_cb,
      timeout: u64,
      repeat: u64,
    ) -> ::std::os::raw::c_int;
    fn uv_timer_stop(handle: *mut uv_timer_t) -> ::std::os::raw::c_int;
    fn uv_timer_again(handle: *mut uv_timer_t) -> ::std::os::raw::c_int;
    fn uv_timer_set_repeat(handle: *mut uv_timer_t, repeat: u64);
    fn uv_timer_get_repeat(handle: *const uv_timer_t) -> u64;
    fn uv_timer_get_due_in(handle: *const uv_timer_t) -> u64;
    fn uv_getaddrinfo(
      loop_: *mut uv_loop_t,
      req: *mut uv_getaddrinfo_t,
      getaddrinfo_cb: uv_getaddrinfo_cb,
      node: *const ::std::os::raw::c_char,
      service: *const ::std::os::raw::c_char,
      hints: *const addrinfo,
    ) -> ::std::os::raw::c_int;
    fn uv_freeaddrinfo(ai: *mut addrinfo);
    fn uv_getnameinfo(
      loop_: *mut uv_loop_t,
      req: *mut uv_getnameinfo_t,
      getnameinfo_cb: uv_getnameinfo_cb,
      addr: *const sockaddr,
      flags: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_spawn(
      loop_: *mut uv_loop_t,
      handle: *mut uv_process_t,
      options: *const uv_process_options_t,
    ) -> ::std::os::raw::c_int;
    fn uv_process_kill(
      arg1: *mut uv_process_t,
      signum: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_kill(
      pid: ::std::os::raw::c_int,
      signum: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_process_get_pid(arg1: *const uv_process_t) -> uv_pid_t;
    fn uv_queue_work(
      loop_: *mut uv_loop_t,
      req: *mut uv_work_t,
      work_cb: uv_work_cb,
      after_work_cb: uv_after_work_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_cancel(req: *mut uv_req_t) -> ::std::os::raw::c_int;
    fn uv_setup_args(
      argc: ::std::os::raw::c_int,
      argv: *mut *mut ::std::os::raw::c_char,
    ) -> *mut *mut ::std::os::raw::c_char;
    fn uv_get_process_title(
      buffer: *mut ::std::os::raw::c_char,
      size: usize,
    ) -> ::std::os::raw::c_int;
    fn uv_set_process_title(title: *const ::std::os::raw::c_char) -> ::std::os::raw::c_int;
    fn uv_resident_set_memory(rss: *mut usize) -> ::std::os::raw::c_int;
    fn uv_uptime(uptime: *mut f64) -> ::std::os::raw::c_int;
    fn uv_get_osfhandle(fd: ::std::os::raw::c_int) -> uv_os_fd_t;
    fn uv_open_osfhandle(os_fd: uv_os_fd_t) -> ::std::os::raw::c_int;
    fn uv_getrusage(rusage: *mut uv_rusage_t) -> ::std::os::raw::c_int;
    fn uv_os_homedir(
      buffer: *mut ::std::os::raw::c_char,
      size: *mut usize,
    ) -> ::std::os::raw::c_int;
    fn uv_os_tmpdir(
      buffer: *mut ::std::os::raw::c_char,
      size: *mut usize,
    ) -> ::std::os::raw::c_int;
    fn uv_os_get_passwd(pwd: *mut uv_passwd_t) -> ::std::os::raw::c_int;
    fn uv_os_free_passwd(pwd: *mut uv_passwd_t);
    fn uv_os_get_passwd2(pwd: *mut uv_passwd_t, uid: uv_uid_t) -> ::std::os::raw::c_int;
    fn uv_os_get_group(grp: *mut uv_group_t, gid: uv_uid_t) -> ::std::os::raw::c_int;
    fn uv_os_free_group(grp: *mut uv_group_t);
    fn uv_os_getpid() -> uv_pid_t;
    fn uv_os_getppid() -> uv_pid_t;
    fn uv_os_getpriority(
      pid: uv_pid_t,
      priority: *mut ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_os_setpriority(pid: uv_pid_t, priority: ::std::os::raw::c_int)
      -> ::std::os::raw::c_int;
    fn uv_thread_getpriority(
      tid: uv_thread_t,
      priority: *mut ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_thread_setpriority(
      tid: uv_thread_t,
      priority: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_available_parallelism() -> ::std::os::raw::c_uint;
    fn uv_cpu_info(
      cpu_infos: *mut *mut uv_cpu_info_t,
      count: *mut ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_free_cpu_info(cpu_infos: *mut uv_cpu_info_t, count: ::std::os::raw::c_int);
    fn uv_cpumask_size() -> ::std::os::raw::c_int;
    fn uv_interface_addresses(
      addresses: *mut *mut uv_interface_address_t,
      count: *mut ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_free_interface_addresses(
      addresses: *mut uv_interface_address_t,
      count: ::std::os::raw::c_int,
    );
    fn uv_os_environ(
      envitems: *mut *mut uv_env_item_t,
      count: *mut ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_os_free_environ(envitems: *mut uv_env_item_t, count: ::std::os::raw::c_int);
    fn uv_os_getenv(
      name: *const ::std::os::raw::c_char,
      buffer: *mut ::std::os::raw::c_char,
      size: *mut usize,
    ) -> ::std::os::raw::c_int;
    fn uv_os_setenv(
      name: *const ::std::os::raw::c_char,
      value: *const ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int;
    fn uv_os_unsetenv(name: *const ::std::os::raw::c_char) -> ::std::os::raw::c_int;
    fn uv_os_gethostname(
      buffer: *mut ::std::os::raw::c_char,
      size: *mut usize,
    ) -> ::std::os::raw::c_int;
    fn uv_os_uname(buffer: *mut uv_utsname_t) -> ::std::os::raw::c_int;
    fn uv_metrics_info(
      loop_: *mut uv_loop_t,
      metrics: *mut uv_metrics_t,
    ) -> ::std::os::raw::c_int;
    fn uv_metrics_idle_time(loop_: *mut uv_loop_t) -> u64;
    fn uv_fs_get_type(arg1: *const uv_fs_t) -> uv_fs_type;
    fn uv_fs_get_result(arg1: *const uv_fs_t) -> isize;
    fn uv_fs_get_system_error(arg1: *const uv_fs_t) -> ::std::os::raw::c_int;
    fn uv_fs_get_ptr(arg1: *const uv_fs_t) -> *mut ::std::os::raw::c_void;
    fn uv_fs_get_path(arg1: *const uv_fs_t) -> *const ::std::os::raw::c_char;
    fn uv_fs_get_statbuf(arg1: *mut uv_fs_t) -> *mut uv_stat_t;
    fn uv_fs_req_cleanup(req: *mut uv_fs_t);
    fn uv_fs_close(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      file: uv_file,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_open(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      flags: ::std::os::raw::c_int,
      mode: ::std::os::raw::c_int,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_read(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      file: uv_file,
      bufs: *const uv_buf_t,
      nbufs: ::std::os::raw::c_uint,
      offset: i64,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_unlink(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_write(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      file: uv_file,
      bufs: *const uv_buf_t,
      nbufs: ::std::os::raw::c_uint,
      offset: i64,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_copyfile(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      new_path: *const ::std::os::raw::c_char,
      flags: ::std::os::raw::c_int,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_mkdir(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      mode: ::std::os::raw::c_int,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_mkdtemp(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      tpl: *const ::std::os::raw::c_char,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_mkstemp(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      tpl: *const ::std::os::raw::c_char,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_rmdir(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_scandir(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      flags: ::std::os::raw::c_int,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_scandir_next(req: *mut uv_fs_t, ent: *mut uv_dirent_t) -> ::std::os::raw::c_int;
    fn uv_fs_opendir(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_readdir(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      dir: *mut uv_dir_t,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_closedir(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      dir: *mut uv_dir_t,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_stat(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_fstat(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      file: uv_file,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_rename(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      new_path: *const ::std::os::raw::c_char,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_fsync(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      file: uv_file,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_fdatasync(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      file: uv_file,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_ftruncate(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      file: uv_file,
      offset: i64,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_sendfile(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      out_fd: uv_file,
      in_fd: uv_file,
      in_offset: i64,
      length: usize,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_access(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      mode: ::std::os::raw::c_int,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_chmod(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      mode: ::std::os::raw::c_int,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_utime(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      atime: f64,
      mtime: f64,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_futime(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      file: uv_file,
      atime: f64,
      mtime: f64,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_lutime(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      atime: f64,
      mtime: f64,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_lstat(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_link(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      new_path: *const ::std::os::raw::c_char,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_symlink(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      new_path: *const ::std::os::raw::c_char,
      flags: ::std::os::raw::c_int,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_readlink(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_realpath(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_fchmod(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      file: uv_file,
      mode: ::std::os::raw::c_int,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_chown(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      uid: uv_uid_t,
      gid: uv_gid_t,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_fchown(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      file: uv_file,
      uid: uv_uid_t,
      gid: uv_gid_t,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_lchown(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      uid: uv_uid_t,
      gid: uv_gid_t,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_statfs(
      loop_: *mut uv_loop_t,
      req: *mut uv_fs_t,
      path: *const ::std::os::raw::c_char,
      cb: uv_fs_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_poll_init(loop_: *mut uv_loop_t, handle: *mut uv_fs_poll_t)
      -> ::std::os::raw::c_int;
    fn uv_fs_poll_start(
      handle: *mut uv_fs_poll_t,
      poll_cb: uv_fs_poll_cb,
      path: *const ::std::os::raw::c_char,
      interval: ::std::os::raw::c_uint,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_poll_stop(handle: *mut uv_fs_poll_t) -> ::std::os::raw::c_int;
    fn uv_fs_poll_getpath(
      handle: *mut uv_fs_poll_t,
      buffer: *mut ::std::os::raw::c_char,
      size: *mut usize,
    ) -> ::std::os::raw::c_int;
    fn uv_signal_init(loop_: *mut uv_loop_t, handle: *mut uv_signal_t) -> ::std::os::raw::c_int;
    fn uv_signal_start(
      handle: *mut uv_signal_t,
      signal_cb: uv_signal_cb,
      signum: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_signal_start_oneshot(
      handle: *mut uv_signal_t,
      signal_cb: uv_signal_cb,
      signum: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    fn uv_signal_stop(handle: *mut uv_signal_t) -> ::std::os::raw::c_int;
    fn uv_loadavg(avg: *mut f64);
    fn uv_fs_event_init(
      loop_: *mut uv_loop_t,
      handle: *mut uv_fs_event_t,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_event_start(
      handle: *mut uv_fs_event_t,
      cb: uv_fs_event_cb,
      path: *const ::std::os::raw::c_char,
      flags: ::std::os::raw::c_uint,
    ) -> ::std::os::raw::c_int;
    fn uv_fs_event_stop(handle: *mut uv_fs_event_t) -> ::std::os::raw::c_int;
    fn uv_fs_event_getpath(
      handle: *mut uv_fs_event_t,
      buffer: *mut ::std::os::raw::c_char,
      size: *mut usize,
    ) -> ::std::os::raw::c_int;
    fn uv_ip4_addr(
      ip: *const ::std::os::raw::c_char,
      port: ::std::os::raw::c_int,
      addr: *mut sockaddr_in,
    ) -> ::std::os::raw::c_int;
    fn uv_ip6_addr(
      ip: *const ::std::os::raw::c_char,
      port: ::std::os::raw::c_int,
      addr: *mut sockaddr_in6,
    ) -> ::std::os::raw::c_int;
    fn uv_ip4_name(
      src: *const sockaddr_in,
      dst: *mut ::std::os::raw::c_char,
      size: usize,
    ) -> ::std::os::raw::c_int;
    fn uv_ip6_name(
      src: *const sockaddr_in6,
      dst: *mut ::std::os::raw::c_char,
      size: usize,
    ) -> ::std::os::raw::c_int;
    fn uv_ip_name(
      src: *const sockaddr,
      dst: *mut ::std::os::raw::c_char,
      size: usize,
    ) -> ::std::os::raw::c_int;
    fn uv_inet_ntop(
      af: ::std::os::raw::c_int,
      src: *const ::std::os::raw::c_void,
      dst: *mut ::std::os::raw::c_char,
      size: usize,
    ) -> ::std::os::raw::c_int;
    fn uv_inet_pton(
      af: ::std::os::raw::c_int,
      src: *const ::std::os::raw::c_char,
      dst: *mut ::std::os::raw::c_void,
    ) -> ::std::os::raw::c_int;
    fn uv_random(
      loop_: *mut uv_loop_t,
      req: *mut uv_random_t,
      buf: *mut ::std::os::raw::c_void,
      buflen: usize,
      flags: ::std::os::raw::c_uint,
      cb: uv_random_cb,
    ) -> ::std::os::raw::c_int;
    fn uv_if_indextoname(
      ifindex: ::std::os::raw::c_uint,
      buffer: *mut ::std::os::raw::c_char,
      size: *mut usize,
    ) -> ::std::os::raw::c_int;
    fn uv_if_indextoiid(
      ifindex: ::std::os::raw::c_uint,
      buffer: *mut ::std::os::raw::c_char,
      size: *mut usize,
    ) -> ::std::os::raw::c_int;
    fn uv_exepath(buffer: *mut ::std::os::raw::c_char, size: *mut usize)
      -> ::std::os::raw::c_int;
    fn uv_cwd(buffer: *mut ::std::os::raw::c_char, size: *mut usize) -> ::std::os::raw::c_int;
    fn uv_chdir(dir: *const ::std::os::raw::c_char) -> ::std::os::raw::c_int;
    fn uv_get_free_memory() -> u64;
    fn uv_get_total_memory() -> u64;
    fn uv_get_constrained_memory() -> u64;
    fn uv_get_available_memory() -> u64;
    fn uv_clock_gettime(clock_id: uv_clock_id, ts: *mut uv_timespec64_t)
      -> ::std::os::raw::c_int;
    fn uv_hrtime() -> u64;
    fn uv_sleep(msec: ::std::os::raw::c_uint);
    fn uv_disable_stdio_inheritance();
    fn uv_dlopen(
      filename: *const ::std::os::raw::c_char,
      lib: *mut uv_lib_t,
    ) -> ::std::os::raw::c_int;
    fn uv_dlclose(lib: *mut uv_lib_t);
    fn uv_dlsym(
      lib: *mut uv_lib_t,
      name: *const ::std::os::raw::c_char,
      ptr: *mut *mut ::std::os::raw::c_void,
    ) -> ::std::os::raw::c_int;
    fn uv_dlerror(lib: *const uv_lib_t) -> *const ::std::os::raw::c_char;
    fn uv_mutex_init(handle: *mut uv_mutex_t) -> ::std::os::raw::c_int;
    fn uv_mutex_init_recursive(handle: *mut uv_mutex_t) -> ::std::os::raw::c_int;
    fn uv_mutex_destroy(handle: *mut uv_mutex_t);
    fn uv_mutex_lock(handle: *mut uv_mutex_t);
    fn uv_mutex_trylock(handle: *mut uv_mutex_t) -> ::std::os::raw::c_int;
    fn uv_mutex_unlock(handle: *mut uv_mutex_t);
    fn uv_rwlock_init(rwlock: *mut uv_rwlock_t) -> ::std::os::raw::c_int;
    fn uv_rwlock_destroy(rwlock: *mut uv_rwlock_t);
    fn uv_rwlock_rdlock(rwlock: *mut uv_rwlock_t);
    fn uv_rwlock_tryrdlock(rwlock: *mut uv_rwlock_t) -> ::std::os::raw::c_int;
    fn uv_rwlock_rdunlock(rwlock: *mut uv_rwlock_t);
    fn uv_rwlock_wrlock(rwlock: *mut uv_rwlock_t);
    fn uv_rwlock_trywrlock(rwlock: *mut uv_rwlock_t) -> ::std::os::raw::c_int;
    fn uv_rwlock_wrunlock(rwlock: *mut uv_rwlock_t);
    fn uv_sem_init(sem: *mut uv_sem_t, value: ::std::os::raw::c_uint) -> ::std::os::raw::c_int;
    fn uv_sem_destroy(sem: *mut uv_sem_t);
    fn uv_sem_post(sem: *mut uv_sem_t);
    fn uv_sem_wait(sem: *mut uv_sem_t);
    fn uv_sem_trywait(sem: *mut uv_sem_t) -> ::std::os::raw::c_int;
    fn uv_cond_init(cond: *mut uv_cond_t) -> ::std::os::raw::c_int;
    fn uv_cond_destroy(cond: *mut uv_cond_t);
    fn uv_cond_signal(cond: *mut uv_cond_t);
    fn uv_cond_broadcast(cond: *mut uv_cond_t);
    fn uv_barrier_init(
      barrier: *mut uv_barrier_t,
      count: ::std::os::raw::c_uint,
    ) -> ::std::os::raw::c_int;
    fn uv_barrier_destroy(barrier: *mut uv_barrier_t);
    fn uv_barrier_wait(barrier: *mut uv_barrier_t) -> ::std::os::raw::c_int;
    fn uv_cond_wait(cond: *mut uv_cond_t, mutex: *mut uv_mutex_t);
    fn uv_cond_timedwait(
      cond: *mut uv_cond_t,
      mutex: *mut uv_mutex_t,
      timeout: u64,
    ) -> ::std::os::raw::c_int;
    fn uv_once(guard: *mut uv_once_t, callback: ::std::option::Option<unsafe extern "C" fn()>);
    fn uv_key_create(key: *mut uv_key_t) -> ::std::os::raw::c_int;
    fn uv_key_delete(key: *mut uv_key_t);
    fn uv_key_get(key: *mut uv_key_t) -> *mut ::std::os::raw::c_void;
    fn uv_key_set(key: *mut uv_key_t, value: *mut ::std::os::raw::c_void);
    fn uv_gettimeofday(tv: *mut uv_timeval64_t) -> ::std::os::raw::c_int;
    fn uv_thread_create(
      tid: *mut uv_thread_t,
      entry: uv_thread_cb,
      arg: *mut ::std::os::raw::c_void,
    ) -> ::std::os::raw::c_int;
    fn uv_thread_create_ex(
      tid: *mut uv_thread_t,
      params: *const uv_thread_options_t,
      entry: uv_thread_cb,
      arg: *mut ::std::os::raw::c_void,
    ) -> ::std::os::raw::c_int;
    fn uv_thread_setaffinity(
      tid: *mut uv_thread_t,
      cpumask: *mut ::std::os::raw::c_char,
      oldmask: *mut ::std::os::raw::c_char,
      mask_size: usize,
    ) -> ::std::os::raw::c_int;
    fn uv_thread_getaffinity(
      tid: *mut uv_thread_t,
      cpumask: *mut ::std::os::raw::c_char,
      mask_size: usize,
    ) -> ::std::os::raw::c_int;
    fn uv_thread_getcpu() -> ::std::os::raw::c_int;
    fn uv_thread_self() -> uv_thread_t;
    fn uv_thread_join(tid: *mut uv_thread_t) -> ::std::os::raw::c_int;
    fn uv_thread_equal(t1: *const uv_thread_t, t2: *const uv_thread_t) -> ::std::os::raw::c_int;
    fn uv_loop_get_data(arg1: *const uv_loop_t) -> *mut ::std::os::raw::c_void;
    fn uv_loop_set_data(arg1: *mut uv_loop_t, data: *mut ::std::os::raw::c_void);
    fn uv_utf16_length_as_wtf8(utf16: *const u16, utf16_len: isize) -> usize;
    fn uv_utf16_to_wtf8(
      utf16: *const u16,
      utf16_len: isize,
      wtf8_ptr: *mut *mut ::std::os::raw::c_char,
      wtf8_len_ptr: *mut usize,
    ) -> ::std::os::raw::c_int;
    fn uv_wtf8_length_as_utf16(wtf8: *const ::std::os::raw::c_char) -> isize;
    fn uv_wtf8_to_utf16(wtf8: *const ::std::os::raw::c_char, utf16: *mut u16, utf16_len: usize);
  }

}
extern "C" {
  pub fn uv_loop_configure(loop_: *mut uv_loop_t, option: uv_loop_option, ...);
}

#[cfg(feature = "dyn-symbols")]
pub(super) unsafe fn load_all() -> Result<libloading::Library, libloading::Error> {
  #[cfg(windows)]
  let host = libloading::os::windows::Library::this()?.into();

  #[cfg(unix)]
  let host = libloading::os::unix::Library::this().into();

  load(&host)?;

  Ok(host)
}
