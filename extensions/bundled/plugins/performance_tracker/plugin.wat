(module
  (import "env" "host_log_performance_snapshot" (func $host_log_performance_snapshot (result i32)))

  (memory (export "memory") 1)

  (data (i32.const 1024)
    "[{\"id\":\"log_performance_snapshot\",\"name\":\"Log performance snapshot\",\"priority\":\"medium\",\"focus_state\":\"global\",\"tags\":[\"wasm\",\"performance\",\"diagnostics\"],\"shortcut_text\":\"WASM\"}]\00")

  (func (export "register_commands_json") (result i32)
    i32.const 1024)

  (func (export "execute") (param $command_id_ptr i32) (param $command_id_len i32) (result i32)
    call $host_log_performance_snapshot)
)
