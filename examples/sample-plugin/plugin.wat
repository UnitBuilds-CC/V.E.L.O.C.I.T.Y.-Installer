;; Sample Velocity Installer WASM Plugin
;;
;; This is a minimal plugin that demonstrates the WASM plugin interface.
;; It exports lifecycle hooks that log messages via the host API.
;;
;; Build with:
;;   wat2wasm plugin.wat -o plugin.wasm
;;
;; Or using the Rust `wat` crate:
;;   let wasm = wat::parse_file("plugin.wat")?;
;;   std::fs::write("plugin.wasm", &wasm)?;

(module
  ;; Import host functions from the installer
  (import "env" "host_log" (func $host_log (param i32 i32 i32 i32)))

  ;; Linear memory (1 page = 64KB)
  (memory (export "memory") 1)

  ;; String constants stored in memory at compile time
  ;; Offset 0: "info" (4 bytes)
  (data (i32.const 0) "info")
  ;; Offset 16: "Hello from WASM plugin! on_load called" (40 bytes)
  (data (i32.const 16) "Hello from WASM plugin! on_load called")
  ;; Offset 64: "Hello from WASM plugin! on_pre_install called" (47 bytes)
  (data (i32.const 64) "Hello from WASM plugin! on_pre_install called")
  ;; Offset 128: "Hello from WASM plugin! on_post_install called" (48 bytes)
  (data (i32.const 128) "Hello from WASM plugin! on_post_install called")
  ;; Offset 192: "Hello from WASM plugin! on_error called" (41 bytes)
  (data (i32.const 192) "Hello from WASM plugin! on_error called")
  ;; Offset 256: "Hello from WASM plugin! on_unload called" (42 bytes)
  (data (i32.const 256) "Hello from WASM plugin! on_unload called")

  ;; on_load(ctx_ptr, ctx_len) -> 0
  (func (export "on_load") (param i32 i32) (result i32)
    ;; host_log(level_ptr=0, level_len=4, msg_ptr=16, msg_len=40)
    (call $host_log (i32.const 0) (i32.const 4) (i32.const 16) (i32.const 40))
    (i32.const 0) ;; success
  )

  ;; on_pre_install(ctx_ptr, ctx_len) -> 0
  (func (export "on_pre_install") (param i32 i32) (result i32)
    (call $host_log (i32.const 0) (i32.const 4) (i32.const 64) (i32.const 47))
    (i32.const 0)
  )

  ;; on_post_install(ctx_ptr, ctx_len) -> 0
  (func (export "on_post_install") (param i32 i32) (result i32)
    (call $host_log (i32.const 0) (i32.const 4) (i32.const 128) (i32.const 48))
    (i32.const 0)
  )

  ;; on_error(ctx_ptr, ctx_len, err_ptr, err_len) -> 0
  (func (export "on_error") (param i32 i32 i32 i32) (result i32)
    (call $host_log (i32.const 0) (i32.const 4) (i32.const 192) (i32.const 41))
    (i32.const 0)
  )

  ;; on_unload(ctx_ptr, ctx_len) -> 0
  (func (export "on_unload") (param i32 i32) (result i32)
    (call $host_log (i32.const 0) (i32.const 4) (i32.const 256) (i32.const 42))
    (i32.const 0)
  )
)
