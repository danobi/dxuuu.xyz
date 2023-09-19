% Packet drop reasons

Figuring out why the kernel is dropping packets can be challenging.
Fortunately, newer kernel releases contain `dropreason` infrastructure that can
make the problem more tractable.

We can easily tap into `dropreason` by using bpftrace. Consider this one-liner:

```
$ sudo bpftrace -e 'tracepoint:skb:kfree_skb { printf("%-18s %d\n", comm, args->reason); }'
Attaching 1 probe...
irq/160-iwlwifi    50
irq/160-iwlwifi    50
irq/160-iwlwifi    50
irq/170-iwlwifi    6
irq/170-iwlwifi    2
irq/160-iwlwifi    50
syncthing          6
syncthing          6
syncthing          6
irq/170-iwlwifi    6
irq/170-iwlwifi    2
irq/160-iwlwifi    50
irq/160-iwlwifi    50

^C
```

So we see some output. But what does `args->reason` correspond to? Find out
by querying the tracepoint arguments:

```
$ sudo bpftrace -lv 'tracepoint:skb:kfree_skb'
tracepoint:skb:kfree_skb
    void * skbaddr
    void * location
    unsigned short protocol
    enum skb_drop_reason reason
```

Now that we know it's an `enum skb_drop_reason`, query the values of the
enum with:

```
$ sudo bpftrace -lv 'enum skb_drop_reason'
enum skb_drop_reason {
        SKB_NOT_DROPPED_YET = 0,
        SKB_CONSUMED = 1,
        SKB_DROP_REASON_NOT_SPECIFIED = 2,
        SKB_DROP_REASON_NO_SOCKET = 3,
        SKB_DROP_REASON_PKT_TOO_SMALL = 4,
        SKB_DROP_REASON_TCP_CSUM = 5,
        SKB_DROP_REASON_SOCKET_FILTER = 6,
        SKB_DROP_REASON_UDP_CSUM = 7,
        SKB_DROP_REASON_NETFILTER_DROP = 8,
        SKB_DROP_REASON_OTHERHOST = 9,
        SKB_DROP_REASON_IP_CSUM = 10,
        SKB_DROP_REASON_IP_INHDR = 11,
        SKB_DROP_REASON_IP_RPFILTER = 12,
        SKB_DROP_REASON_UNICAST_IN_L2_MULTICAST = 13,
        [...]
};
```

Now suppose we are curious about the `SKB_DROP_REASON_SOCKET_FILTER` (6) drops.
Let's see where in the kernel this is happening:

```
$ sudo bpftrace -e 't:skb:kfree_skb / args->reason == SKB_DROP_REASON_SOCKET_FILTER / { print(kstack) }'
Attaching 1 probe...

        kfree_skb_reason+143
        kfree_skb_reason+143
        rawv6_rcv+792
        raw6_local_deliver+269
        ip6_protocol_deliver_rcu+134
        ip6_input_finish+67
        ip6_mc_input+238
        ip6_sublist_rcv_finish+130
        ip6_sublist_rcv+552
        ipv6_list_rcv+319
        __netif_receive_skb_list_core+501
        netif_receive_skb_list_internal+465
        napi_complete_done+114
        iwl_pcie_napi_poll_msix+173
        __napi_poll+40
        net_rx_action+676
        __do_softirq+209
        do_softirq.part.0+95
        __local_bh_enable_ip+104
        iwl_pcie_irq_rx_msix_handler+205
        irq_thread_fn+32
        irq_thread+251
        kthread+229
        ret_from_fork+41

```

This gives us a call stack in the kernel where the drop occurs. Now the problem
is within reach -- we know where to start reading code.
