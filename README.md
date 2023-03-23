# ibv
RDMA practice

focus on the function at first, considering the safety problem lately.

## interfaec of `Conn`:
 1.send_msg(data: &[IoSlice]) -> Result<()>
 2.recv_msg() -> Result<&[u8]>

## todo
at ./docs/todo.md
large todo: error information handle
            switch to async code

## safety problem
### memory management
todo: reference count, resources drop


## example
`cargo run --example server`
another terminal:
`cargo run --example client`

## environment
please see ./docs/evc.md