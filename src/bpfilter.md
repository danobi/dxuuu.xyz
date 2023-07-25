% bpfilter is forever

This week it was brought to my attention that `bpfilter` might be delaying our
kernel boot sequence. The initial thought was that bpfilter's usermode upcalls
were stalling for some reason and caused boot time stalls.

While it is true that module initialization for built-in modules (ie.
`CONFIG_FOO=y`) is serialized and that in theory it is possible for the boot to
be stalled if a module was slow, it turned out not to be the case for bpfilter,
as we had `CONFIG_BPFILTER_UMH=m` which actually causes `bpfilter.ko` to be
built and loaded separately.

So end of story, at least for the important side of the investigation.

### Mystery reloads

In the process of debugging the above, I unloaded and loaded `bpfilter.ko` a
few times. After one of the `modprobe -r`'s, I noticed that bpfilter was
automatically reloaded without my involvement. It looked something like this:

```
$ sudo modprobe -r bpfilter
$ lsmod | grep bpf
$ lsmod | grep bpf
$ lsmod | grep bpf
$ lsmod | grep bpf
$ lsmod | grep bpf
$ lsmod | grep bpf
$ lsmod | grep bpf
lsmod | grep bpf
bpfilter               24576  0
```

No good investigation is without a rabbit hole, so naturally I fixated on this.

The first thing to check was [`modules-load.d`][0] and [`modprobe.d`][1].  A
thorough `grep` of the relevant directories for choice keywords like "bpf" or
"bpfilter" turned up nothing interesting. I couldn't think of any more top-down
avenues to explore, so I turned to bottom-up.

### Luckily we have `bpftrace`

I had poked around the kernel module subsystem a few weeks back for an
unrelated task so I knew about the entry and exit points: `init_module(2)`,
`finit_module(2)`, and `delete_module(2)`.

Knowing that, it's a simple matter of writing a one-liner to monitor
those system calls:

```
$ sudo bpftrace -e 't:syscalls:sys_enter_*module { printf("%s: comm=%s\n", probe, comm) }' &
$ sudo modprobe -r bpfilter
tracepoint:syscalls:sys_enter_delete_module: comm=modprobe
tracepoint:syscalls:sys_enter_finit_module: comm=modprobe
```

Alright, so something is calling `modprobe`. I was halfway through writing
a wrapper script to stash the caller's `comm` in a file when I remembered that
bpfilter will call `request_module()` in certain cases.

For the uninitiated, `request_module()` is a kernel function that requests
userspace load a particular kernel module. Internally, it performs and upcall
to userspace's `modprobe(8)` -- exactly the sort of thing we might have seen
above.

Indeed, `net/ipv4/bpfilter/sockopt.c` confirms this:

``` {#function .c .numberLines startFrom="15"}
static int bpfilter_mbox_request(struct sock *sk, int optname, sockptr_t optval,
                                 unsigned int optlen, bool is_set)
{
        int err;
        mutex_lock(&bpfilter_ops.lock);
        if (!bpfilter_ops.sockopt) {
                mutex_unlock(&bpfilter_ops.lock);
                request_module("bpfilter");
                mutex_lock(&bpfilter_ops.lock);

                if (!bpfilter_ops.sockopt) {
                        err = -ENOPROTOOPT;
                        goto out;
                }
        }

        [...]
}
```

`bpfilter_mbox_request()` is called from both the `getsockopt(2)` and
`setsockopt(2)` codepaths. Since `getsockopt()` seemed like a more likely
run codepath, I further traced there:

```
$ sudo bpftrace -e 'k:bpfilter_ip_get_sockopt { printf("comm=%s\n kstack=\n%s\n", comm, kstack) }'
Attaching 1 probe...
comm=iptables
kstack=

        bpfilter_ip_get_sockopt+1
        raw_getsockopt+52
        sock_common_getsockopt+26
        __sys_getsockopt+181
        __x64_sys_getsockopt+36
        do_syscall_64+87
        entry_SYSCALL_64_after_hwframe+6

```

In this case, I didn't even have to unload the module! It turns out `iptables`
was regularly calling `getsockopt(2)`.

Now the question is why iptables is talking to bpfilter. bpfilter literally has
no functionality, so it does not make sense for iptables to use it. To answer this,
we must look further up the call stack, to where `getsockopt()` is dispatched
to `bpfilter_ip_get_sockopt()` -- in `net/ipv4/ip_sockglue.c`:

``` {#function .c .numberLines startFrom="1803"}
int ip_getsockopt(struct sock *sk, int level,
                  int optname, char __user *optval, int __user *optlen)
{
        int err;

        err = do_ip_getsockopt(sk, level, optname,
                               USER_SOCKPTR(optval), USER_SOCKPTR(optlen));

#if IS_ENABLED(CONFIG_BPFILTER_UMH)
        if (optname >= BPFILTER_IPT_SO_GET_INFO &&
            optname < BPFILTER_IPT_GET_MAX)
                err = bpfilter_ip_get_sockopt(sk, optname, optval, optlen);
#endif
#ifdef CONFIG_NETFILTER
        /* we need to exclude all possible ENOPROTOOPTs except default case */
        if (err == -ENOPROTOOPT && optname != IP_PKTOPTIONS &&
                        !ip_mroute_opt(optname)) {
                int len;

                if (get_user(len, optlen))
                        return -EFAULT;

                err = nf_getsockopt(sk, PF_INET, optname, optval, &len);
                if (err >= 0)
                        err = put_user(len, optlen);
                return err;
        }
#endif
        return err;
}
```

Fortunately the code here is pretty simple: `optname` must be in the range
`[BPFFILTER_IPT_SO_GET_INFO, BPFILTER_IPT_GET_MAX)` for our fateful function
to be called. Another quick trace gives us there raw values:

```
$ sudo bpftrace -e 'k:bpfilter_ip_get_sockopt { printf("comm=%s\n optname=%d\n ustack=%s\n kstack=%s\n", comm, arg1, ustack, kstack) }'
Attaching 1 probe...
comm=iptables
 optname=64
 ustack=
        0x7fa0acd7783a

 kstack=
        bpfilter_ip_get_sockopt+1
        raw_getsockopt+52
        sock_common_getsockopt+26
        __sys_getsockopt+181
        __x64_sys_getsockopt+36
        do_syscall_64+87
        entry_SYSCALL_64_after_hwframe+68
```

From here, it was basic due dilligence to check the values of
`BPFFILTER_IPT_SO_GET_INFO` and `BPFILTER_IPT_GETMAX` to see if:

1. The range contains 64
1. If the range is shared with any iptables sockopts

Some quick grepping gives us the (abbreviated) definitions:

```c
#define IPT_BASE_CTL            64
#define IPT_SO_GET_INFO         (IPT_BASE_CTL)

num {
        BPFILTER_IPT_SO_GET_INFO = 64,
        [...]
        BPFILTER_IPT_GET_MAX,
};
```

So there you have it. bpfilter deliberately overlaps the iptables sockopt
optname ranges. Probably so `iptables(8)` (the CLI) can be transparently used
with the bpfilter backend. Unfortunately as a result of this design,
`bpfilter_ip_get_sockopt()` is unconditionally called when iptables
`getsockopt(2)`/`setsockopt(2)` is used. And this triggers our module
reloads. Unfortunate, understandable, and relatively harmless.


### The obvious question

Astute readers will have already wondered the following question: why do you
have kernels with `CONFIG_BPFILTER=y` if bpfilter does not contain any
functionality in it's current form?

Well, that's a question for Ubuntu:

```
$ vagrant init ubuntu/jammy64
$ vagrant up
[...]
$ vagrant ssh
vagrant@ubuntu-jammy:~$ uname -r
5.15.0-76-generic
vagrant@ubuntu-jammy:~$ grep CONFIG_BPFILTER /boot/config-$(uname -r)
CONFIG_BPFILTER=y
CONFIG_BPFILTER_UMH=m
```


[0]: https://www.freedesktop.org/software/systemd/man/modules-load.d.html
[1]: https://man7.org/linux/man-pages/man5/modprobe.d.5.html
