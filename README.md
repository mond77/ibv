# ibv
RDMA practice

## interfaec of `Conn`:
 1.send_msg(data: &[IoSlice]) -> Result<()>

 2.recv_msg() -> Result<&[u8]>

## todo

todo: error information handle

## safety problem

error handle

### memory management


## example

`cargo run --example server`

another terminal:
`cargo run --example client`

## environment
please see ./docs/evc.md