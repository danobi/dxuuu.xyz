% `/proc/[pid]` and the missing threads

Suppose you run the following program:

``` {#function .rs .numberLines startFrom="1"}
use std::time;
use std::thread;

fn main() {
    println!("My TID: {}", nix::unistd::gettid());

    let child = thread::spawn(move || {
        println!("My thread's TID: {}", nix::unistd::gettid());
        loop {
            thread::sleep(time::Duration::from_secs(1));
        }
    });

    child.join().unwrap();
}
```

and get the following output:

```
My TID: 840680
My thread's TID: 840695
```

Inspecting thread state through `/proc` works as expected:

```
$ cat /proc/840680/comm
thread_id

$ cat /proc/840695/comm
thread_id
```

However, if you happened to browse through `/proc/` (via `ls` or other), you'll
notice a strange inconsistency:

```
$ ls -l /proc | grep 840680 &> /dev/null; echo $?
0

$ ls -l /proc | grep 840695 &> /dev/null; echo $?
1
```

In other words, there's no directory entry for the _thread_.

Why is this the case? We have to look at the kernel code to find out. First
let's look at where all the entries in `/proc` are instantiated. Remember
that `/proc`, or `procfs`, is a virtual file system so there's not actually
anything on disk backing the fileystem. Everything is generated when we request
it.

In `fs/proc/root.c`:

``` {#function .c .numberLines startFrom="283"}
/*
 * This is the root "inode" in the /proc tree..
 */
struct proc_dir_entry proc_root = {
        .low_ino        = PROC_ROOT_INO,
        .namelen        = 5,
        .mode           = S_IFDIR | S_IRUGO | S_IXUGO,
        .nlink          = 2,
        .refcnt         = REFCOUNT_INIT(1),
        .proc_iops      = &proc_root_inode_operations,
        .proc_dir_ops   = &proc_root_operations,
        .parent         = &proc_root,
        .subdir         = RB_ROOT,
        .name           = "/proc",
};
```

`&proc_root_operations` seems like a likely suspect for **dir**ectory **op**eration**s**:

``` {#function .c .numberLines startFrom="261"}
/*
 * The root /proc directory is special, as it has the
 * <pid> directories. Thus we don't use the generic
 * directory handling functions for that..
 */
static const struct file_operations proc_root_operations = {
        .read            = generic_read_dir,
        .iterate_shared  = proc_root_readdir,
        .llseek         = generic_file_llseek,
};
```

So far the comments confirm our understanding. However, it's somewhat unclear
which callback is called when we run `ls` versus directly `cat` a file. Let's use
bpftrace to investigate.

In one terminal:
```
$ sudo bpftrace -e 'kprobe:generic_read_dir { printf("%s\n", kstack); }'
Attaching 1 probe...
```

In another terminal:
```
$ ls -l /proc
```

Nothing in the first terminal. Let's try the next function.
```
$ sudo bpftrace -e 'kprobe:proc_root_readdir { printf("%s\n", kstack); }'
Attaching 1 probe...
```

Run `ls` again and we get the following output:
```
        proc_root_readdir+1
        iterate_dir+323
        ksys_getdents64+156
        __x64_sys_getdents64+22
        do_syscall_64+78
        entry_SYSCALL_64_after_hwframe+68


        proc_root_readdir+1
        iterate_dir+323
        ksys_getdents64+156
        __x64_sys_getdents64+22
        do_syscall_64+78
        entry_SYSCALL_64_after_hwframe+68

```

Nice, so we know running `ls` generates a `proc_root_readdir` callback. Let's look
at the code:

``` {#function .c .numberLines startFrom="249"}
static int proc_root_readdir(struct file *file, struct dir_context *ctx)
{
        if (ctx->pos < FIRST_PROCESS_ENTRY) {
                int error = proc_readdir(file, ctx);
                if (unlikely(error <= 0))
                        return error;
                ctx->pos = FIRST_PROCESS_ENTRY;
        }

        return proc_pid_readdir(file, ctx);
}
```

`FIRST_PROCESS_ENTRY` is defined as:

in `fs/proc/internal.h`:
``` {#function .c .numberLines startFrom="137"}
/*
 * Offset of the first process in the /proc root directory..
 */
#define FIRST_PROCESS_ENTRY 256
```

and we see `proc_readdir` incrementing `pos` in `proc_readdir_de` (a later
callee). So this code probably handles all the non-process entries in `/proc`
and we can ignore it for now and focus on `proc_pid_readdir`.

In `fs/proc/base.c`:
``` {#function .c .numberLines startFrom="3371"}
/* for the /proc/ directory itself, after non-process stuff has been done */
int proc_pid_readdir(struct file *file, struct dir_context *ctx)
{
        struct tgid_iter iter;
        struct pid_namespace *ns = proc_pid_ns(file_inode(file));
        loff_t pos = ctx->pos;
```

This code just sets up some variables by pulling context information out. Not
really important.

``` {#function .c .numberLines startFrom="3378"}
        if (pos >= PID_MAX_LIMIT + TGID_OFFSET)
                return 0;

        if (pos == TGID_OFFSET - 2) {
                struct inode *inode = d_inode(ns->proc_self);
                if (!dir_emit(ctx, "self", 4, inode->i_ino, DT_LNK))
                        return 0;
                ctx->pos = pos = pos + 1;
        }
        if (pos == TGID_OFFSET - 1) {
                struct inode *inode = d_inode(ns->proc_thread_self);
                if (!dir_emit(ctx, "thread-self", 11, inode->i_ino, DT_LNK))
                        return 0;
                ctx->pos = pos = pos + 1;
        }
```

This code does 3 things:

1. Impose a limit on the number of entries
1. Emit the `/proc/self` entry
1. Emit the `/proc/thread-self` entry

Interesting to note but not important for this article.

``` {#function .c .numberLines startFrom="3393"}
        iter.tgid = pos - TGID_OFFSET;
        iter.task = NULL;
        for (iter = next_tgid(ns, iter);
             iter.task;
             iter.tgid += 1, iter = next_tgid(ns, iter)) {
```

Now this is the interesting bit. Now we're iterating through all the thread
group IDs (tgid) via **`next_tgid`**. TGIDs are better understood from userspace as the PIDs we
see, where each process can have multiple threads (each with their own TID).

``` {#function .c .numberLines startFrom="3398"}
             ...
             <fill dcache entry>
             ...
        }
}
```

There's more code that follows but it's not very interesting for us.

So we know why `ls /proc` does not show threads now. But how does directly
accessing `/proc/[TID]/comm` work?

We follow the same process with bpftrace and try some more functions. Finally, we discover
that the following triggers output when we run `cat /proc/864518/comm`:

```
$ sudo bpftrace -e 'kprobe:proc_root_lookup / comm == "cat" / { printf("%s\n", kstack); }'
Attaching 1 probe...

        proc_root_lookup+1
        __lookup_slow+140
        walk_component+513
        link_path_walk+759
        path_openat+157
        do_filp_open+171
        do_sys_openat2+534
        do_sys_open+68
        do_syscall_64+78
        entry_SYSCALL_64_after_hwframe+68
```

Note that we used a filter in our bpftrace script to limit output to our command.

Astute readers might have noted that our `cat` command used a different TID. That's because
we only trigger output once per lifetime (or some other period of time) of the TID. That's
because the kernel is probably caching directory entries in memory so it doesn't have to
do a full lookup every time.

Now look at `proc_root_lookup`:

In `fs/proc/root.c`:

``` {#function .c .numberLines startFrom="241"}
static struct dentry *proc_root_lookup(struct inode * dir, struct dentry * dentry, unsigned int flags)
{
        if (!proc_pid_lookup(dentry, flags))
                return NULL;

        return proc_lookup(dir, dentry, flags);
}
```

In `fs/proc/base.c`:
``` {#function .c .numberLines startFrom="3300"}
struct dentry *proc_pid_lookup(struct dentry *dentry, unsigned int flags)
{
        struct task_struct *task;
        unsigned tgid;
        struct pid_namespace *ns;
        struct dentry *result = ERR_PTR(-ENOENT);

        tgid = name_to_int(&dentry->d_name);
        if (tgid == ~0U)
                goto out;
```

Some setup and error checks. Not too interesting.

``` {#function .c .numberLines startFrom="3311"}
        ns = dentry->d_sb->s_fs_info;
        rcu_read_lock();
        task = find_task_by_pid_ns(tgid, ns);
```

This is more interesting: we do a lookup on the requested tgid. Note that `tgid`
here is somewhat improperly named. We're doing a lookup based on a `task` which
does not have to be a thread group leader.

``` {#function .c .numberLines startFrom="3314"}
        if (task)
                get_task_struct(task);
        rcu_read_unlock();
        if (!task)
                goto out;

        result = proc_pid_instantiate(dentry, task, NULL);
        put_task_struct(task);
out:
        return result;
}
```

The remainder of the function instantiates an inode for `/proc/[TID]` and most
likely populates it as well. Then in `proc_root_lookup`, `proc_lookup` probably
walks the FS structure and finds the new inode.

Mystery solved.
