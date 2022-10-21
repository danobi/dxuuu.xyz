% File capabilities and /proc/\<pid\>

I've spent the past few evenings staring at a rather interesting kernel "bug".
I put "bug" in quotes b/c, spoiler alert, it's not a bug.

It all started with an integration test failing due to a `-EACCES` return code.
Normally I'd assume it's a bug with the code under test, but we also happen to
cover the same codepath in unit tests. So a failure when the code was run in a
separate binary was something worth investigating.

The details of the test and test infrastructure aren't important b/c I was
able to come up with a short reproducer:

``` {#function .c .numberLines startFrom="1"}
#include <fcntl.h>
#include <stdio.h>

int main() {
  int fd = open("/proc/self/auxv", O_RDONLY);

  if (fd < 0) {
    perror("open");
    return 1;
  }

  printf("ok\n");
  return 0;
}
```

```
$ gcc main.c
$ ./a.out                                             # (1)
ok
$ sudo setcap "cap_net_admin,cap_sys_admin+p" a.out   # (2)
$ ./a.out                                             # (3)
open: Permission denied
```

Basically the above program tries to open `/proc/self/auxv` and reports on
whether it was successful. Pretty simple, and that's all we do in step (1).

Things get more interesting when we get to step (2). For the unititiated, in
step (2) we set a couple file capabilities on the executable. File capabilities
(filecaps) are basically the same thing as [setuid][0], except filecaps operate
on capabilities which are much more granular than setuid's "root or not".

More specifically, we add the capabilities to the **permitted** set. This is an
important distinction b/c it means the executable, when run as an
unprivileged process, starts out with no extra capabilities.  However, the
process may grant itself any of the permitted capabilities (ie. "elevate").
Note how the reproducer does not elevate to any capabilities. In other words,
our reproducer is capability-dumb.

Finally, in step (3) we run the reproducer just like in step (1). However
unlike in step (1), we get `-EACCES`. And to be explicit, this is really
strange b/c usually programs have _extra_ powers with more capabilities, not
_less_. Furthermore, the process doesn't even use extra capabilities at
runtime.

The stage is set for an interesting investigation: a trivial reproducer and a
wildly unexpected result.

## Odd ownership

I spent some time trying to trace kernel execution with everyone's favorite
tracer (bpftrace!). Unfortunately, a lot of the interesting logic was either in
the body of a long function or in an inlined function call so I had to fall
back to `printk()` debugging.

What followed was a lot of edit-compile-run loops (shoutout to [virtme][1] for
making testing kernels so easy). I won't bore you with the details, so suffice
to say after a few hours I discovered that `/proc/self/auxv` was owned by
`root:root`. And since `/proc/self/auxv` has `0400` permissions, our process
running under `daniel:daniel` fails the UNIX permission check in
`acl_permission_check()`.

Christian Brauner [later confirms][2] this discovery on the lists.

So `-EACCES` is explained. But this begs a new question: why is
`/proc/self/auxv` owned by root?

## Process dumpability

I've looked at procfs internals [before][3] so I had an idea where to start
looking. It took another few hours and a mixture of `printk()` and tracing, but
I discovered where the procfs inodes get their owners calculated:
`task_dump_owner()`.

Around the same time (within minutes actually), Omar Sandoval kindly [pointed
out][4] on twitter to look at process dumpability.

Process dumpability appears to govern whether or not processes can have their
cores dumped. This seems reasonable -- you may not want a privileged process's
memory to be dumped to disk if it contains sensitive information.

With the above hint, the search is narrowed down to the following block:

``` {#function .c .numberLines startFrom="1848"}
void task_dump_owner(struct task_struct *task, umode_t mode,
                     kuid_t *ruid, kgid_t *rgid)
{
        [...]

        if (mode != (S_IFDIR|S_IRUGO|S_IXUGO)) {
                struct mm_struct *mm;
                task_lock(task);
                mm = task->mm;
                /* Make non-dumpable tasks owned by some root */
                if (mm) {
                        if (get_dumpable(mm) != SUID_DUMP_USER) {
                                struct user_namespace *user_ns = mm->user_ns;

                                uid = make_kuid(user_ns, 0);
                                if (!uid_valid(uid))
                                        uid = GLOBAL_ROOT_UID;

                                gid = make_kgid(user_ns, 0);
                                if (!gid_valid(gid))
                                        gid = GLOBAL_ROOT_GID;
                        }
                } else {
                        uid = GLOBAL_ROOT_UID;
                        gid = GLOBAL_ROOT_GID;
                }
                task_unlock(task);
        }
        *ruid = uid;
        *rgid = gid;
}
```

Line 1853 checks if the inode is a globally accessible directory.
`/proc/pid/auxv` is a regular file so we enter the body.

Line 1859 then checks for process dumpability. If it's not globally dumpable
(globally dumpable meaning anyone can read the process's core dumps), the
procfs inode is set to be owned by `root:root`. This decision is returned via
`ruid` and `rgid` out-params on lines 1876-1877.

## File capabilities

Now that we suspect process dumpability is involved, all that's left to
do is figure out who or what is setting process dumpability. To that end,
we run the following bpftrace script against the reproducer:

```
$ cat b.bt
kfunc:set_dumpable
/ comm == "a.out" /
{
  printf("value=%d\n", args->value);
  print(kstack);
}

$ sudo bpftrace ./b.bt
Attaching 1 probe...
value=2

        bpf_prog_f61e3e4e5dc2ac1c_set_dumpable+306
        bpf_get_stackid_raw_tp+119
        bpf_prog_f61e3e4e5dc2ac1c_set_dumpable+306
        bpf_trampoline_6442491445_0+71
        set_dumpable+9
        commit_creds+111
        begin_new_exec+1653
        load_elf_binary+1665
        bprm_execve+639
        do_execveat_common.isra.0+429
        __x64_sys_execve+54
        do_syscall_64+92
        entry_SYSCALL_64_after_hwframe+99

^C
```

Here we see the smoking gun: `__x64_sys_execve+54`. This means that dumpability
is being set at process creation time. This smells badly of file capability
shenanigans. However for completeness, we'll look at `commit_creds()` which
directly calls `set_dumpable()`:

``` {#function .c .numberLines startFrom="466"}
int commit_creds(struct cred *new)
{
        [...]

        /* dumpability changes */
        if (!uid_eq(old->euid, new->euid) ||
            !gid_eq(old->egid, new->egid) ||
            !uid_eq(old->fsuid, new->fsuid) ||
            !gid_eq(old->fsgid, new->fsgid) ||
            !cred_cap_issubset(old, new)) {
                if (task->mm)
                        set_dumpable(task->mm, suid_dumpable);
                task->pdeath_signal = 0;
                /*
                 * If a task drops privileges and becomes nondumpable,
                 * the dumpability change must become visible before
                 * the credential change; otherwise, a __ptrace_may_access()
                 * racing with this change may be able to attach to a task it
                 * shouldn't be able to attach to (as if the task had dropped
                 * privileges without becoming nondumpable).
                 * Pairs with a read barrier in __ptrace_may_access().
                 */
                smp_wmb();
        }

        [...]
}
```

Line 475 is what we've been after. `cred_cap_issubset()` is checking is if the
old capabilities are a subset of the new capabilities.  If so, it means the
process has gained capabilities during an `execve(2)` and process dumpability
is adjusted. Again, this checks out b/c the new process may (or already) have
elevated capabilities and could contain sensitive information going forward.

## Conclusion

So we have our answer: our unprivileged cannot read `/proc/self/auxv` because
it has filecaps assigned. And b/c filecaps can potentially make a process
contain sensitive information, the kernel makes its `/proc/<pid>` inodes owned
by `root:root`.

Although nothing terribly actionable resulted from this investivation, I
nonetheless found this to be an interesting case study in emergent properties.
Each new discovery made sense in isolation. But despite each step making sense,
the final result remains unexpected. And b/c each step makes sense in
isolation, I cannot think of a good way to change the final result without
making some piece of the puzzle _not_ make sense.

I suppose that's why documentation is a thing.

And by the way, this behavior is actually documented in `proc(2)` man page
under `PR_SET_DUMPABLE`.


[0]: https://en.wikipedia.org/wiki/Setuid
[1]: https://github.com/amluto/virtme
[2]: https://lore.kernel.org/linux-fsdevel/20221019132201.kd35firo6ks6ph4j@wittgenstein/
[3]: https://dxuuu.xyz/proc-threads.html
[4]: https://twitter.com/omsandov/status/1582941637705359360
