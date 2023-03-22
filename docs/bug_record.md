Segmentation fault (core dumped):
    client 没发完请求就挂了，改了RecvBuf（ManuallyDrop）和polling（daemon）之后还是会挂，
           在改了SendBuf（ManuallyDrop）就成功了。
    future todo: 内存管理需要重视一下。