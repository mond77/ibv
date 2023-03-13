## Environment Setup

This section is for RDMA novices who want to try this library.

You can skip if your Machines have been configured with RDMA.

Next we will configure the RDMA environment in an Ubuntu20.04 VM.
If you are using another operating system distribution, please search and replace the relevant commands.

### 1. Check whether the current kernel supports RXE

Run the following command and if the CONFIG_RDMA_RXE = `y` or `m`, the current operating system supports RXE.
If not you need to search how to recompile the kernel to support RDMA.

```shell
cat /boot/config-$(uname -r) | grep RXE
```

### 2. Install Dependencies

```shell
sudo apt install -y libibverbs1 ibverbs-utils librdmacm1 libibumad3 ibverbs-providers rdma-core libibverbs-dev iproute2 perftest build-essential net-tools git librdmacm-dev rdmacm-utils cmake libprotobuf-dev protobuf-compiler clang curl
```

### 3. Configure RDMA netdev

(1) Load kernel driver

```shell
modprobe rdma_rxe
```

(2) User mode RDMA netdev configuration.

```shell
sudo rdma link add rxe_0 type rxe netdev ens33
```

`rxe_0` is the RDMA device name, and you can name it whatever you want. `ens33` is the name of the network device. The name of the network device may be different in each VM, and we can see it by running command "ifconfig".

(3) Check the RDMA device state

Run the following command and check if the state is `ACTIVE`.

```shell
rdma link
```

(4) Test it

Ib_send_bw is a program used to test the bandwidth of `RDMA SEND` operations.

Run the following command in a terminal.

```shell
ib_send_bw -d rxe_0
```

And run the following command in another terminal.

```shell
ib_send_bw -d rxe_0 localhost
```
