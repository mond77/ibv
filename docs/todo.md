## todo
9th, Mar
 1. abstract Device,PD,CQ for 'default'. (done)
10th, Mar
 1. abstract QP for 'create','modify' (done)
 2. implement send/recv operation (lazy)
13th, Mar
 1. implement send/recv operation  (done)
 2. build the connection between QPs by TCP (done)
 future todo:
    1. add multi devices support
14th, Mar
 1. implement write/read operation (done)
 2. abstract MR ManageMent  (yet todo)
 future todo:
    1. add cq channel
15th, Mar
 1. abstract MR ManageMent (done)
 2. think about the transport framework. (done)
16th to ~ , Mar
 1. abstract WR (work Request of write/read,send/recv) (done)
 2. abstract poll CQ with size. (done)
 3. implement the write_with_imm operation, how does the WC generated in the remote end? (generate with a solicited event)
 4. MR management：
     1、MR -> ibv_sge（Implement it when considering the send buffer）
     2、RemoteMR -> Remote_Buffer；
     ringbuf：
		1、region：received/empty（the remote end to inform），writen（ack），writing（no ack）
		2、head and tail handle : single WR split into two Or ignore the tail fragment
        3、performance：use lock at first
		4、Lost processing: not thought right now
 