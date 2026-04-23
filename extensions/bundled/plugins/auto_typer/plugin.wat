(module
  (import "env" "host_write_text" (func $host_write_text (param i32 i32) (result i32)))
  (import "env" "host_read_time_text" (func $host_read_time_text (param i32 i32) (result i32)))

  (memory (export "memory") 1)

  (data (i32.const 1024)
    "[{\"id\":\"type_hello_world\",\"name\":\"Type hello world\",\"priority\":\"medium\",\"focus_state\":\"global\",\"tags\":[\"wasm\",\"typing\",\"demo\"],\"shortcut_text\":\"WASM\"},{\"id\":\"type_current_date\",\"name\":\"Type current date\",\"priority\":\"medium\",\"focus_state\":\"global\",\"tags\":[\"wasm\",\"typing\",\"date\"],\"shortcut_text\":\"WASM\"}]\00")
  (data (i32.const 2048) "hello world")
  (data (i32.const 3072) "type_hello_world")
  (data (i32.const 4096) "type_current_date")

  (func $matches (param $command_id_ptr i32) (param $command_id_len i32) (param $expected_ptr i32) (param $expected_len i32) (result i32)
    (local $index i32)
    local.get $command_id_len
    local.get $expected_len
    i32.ne
    if (result i32)
      i32.const 0
    else
      block $done
        loop $compare
          local.get $index
          local.get $expected_len
          i32.ge_u
          br_if $done
          local.get $command_id_ptr
          local.get $index
          i32.add
          i32.load8_u
          local.get $expected_ptr
          local.get $index
          i32.add
          i32.load8_u
          i32.ne
          if
            i32.const 0
            return
          end
          local.get $index
          i32.const 1
          i32.add
          local.set $index
          br $compare
        end
      end
      i32.const 1
    end)

  (func (export "register_commands_json") (result i32)
    i32.const 1024)

  (func (export "execute") (param $command_id_ptr i32) (param $command_id_len i32) (result i32)
    local.get $command_id_ptr
    local.get $command_id_len
    i32.const 3072
    i32.const 16
    call $matches
    if
      i32.const 2048
      i32.const 11
      call $host_write_text
      return
    end

    local.get $command_id_ptr
    local.get $command_id_len
    i32.const 4096
    i32.const 17
    call $matches
    if
      i32.const 8192
      i32.const 32
      call $host_read_time_text
      local.tee $command_id_len
      i32.const 0
      i32.lt_s
      if
        i32.const 1
        return
      end

      i32.const 8192
      local.get $command_id_len
      call $host_write_text
      return
    end

    i32.const 2)
)
