(module
  (import "env" "host_type_text" (func $host_type_text (param i32 i32) (result i32)))

  (memory (export "memory") 1)

  (data (i32.const 1024)
    "[{\"id\":\"type_hello_world\",\"name\":\"Type hello world\",\"priority\":\"normal\",\"focus_state\":\"global\",\"tags\":[\"wasm\",\"typing\",\"demo\"],\"shortcut_text\":\"WASM\"}]\00")
  (data (i32.const 2048) "hello world")

  (func (export "register_commands_json") (result i32)
    i32.const 1024)

  (func (export "execute") (param $command_id_ptr i32) (param $command_id_len i32) (result i32)
    i32.const 2048
    i32.const 11
    call $host_type_text)
)
