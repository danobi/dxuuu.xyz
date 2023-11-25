% Kernel IPIs

An [inter-processor interrupt][0] (IPI) is a way for one processor to signal to
another processor that some work needs to be done. I'm not sure why I got
curious about this today. But since I dug into it, I thought I'd write it
down.

### Interface

Since it looks like there's quite a few IPI vectors, I narrowed down the scope
by only looking at a fairly general user: `smp_call_function_single()`. It helps
that it has an intuitive API:

```c
/*
 * smp_call_function_single - Run a function on a specific CPU
 * @func: The function to run. This must be fast and non-blocking.
 * @info: An arbitrary pointer to pass to the function.
 * @wait: If true, wait until function has completed on other CPUs.
 *
 * Returns 0 on success, else a negative status code.
 */
int smp_call_function_single(int cpu, smp_call_func_t func, void *info, int wait);
```

where `smp_call_func_t` is:

```c
typedef void (*smp_call_func_t)(void *info);
```

In otherwords, this API lets you run code on another processor.

### Implementation

Since I and many others run x86-64, I'm only looked at the x86 codepaths. And
to spare you the code walkthough, here is the rough call chain instead:

```
smp_call_function_single(cpu, ..)
    generic_exec_single(cpu, ..)
        __smp_call_single_queue(cpu, ..)
            llist_add(node, &per_cpu(call_single_queue, cpu))              /* (1) */
            send_call_function_single_ipi(cpu)
                arch_send_call_function_single_ipi(cpu)
                    native_send_call_func_single_ipi(cpu)
                        __apic_send_IPI(cpu, CALL_FUNCTION_SINGLE_VECTOR)  /* (2) */
```

`(1)` is the crucial part. Basically what's happening is that the calling CPU
enqueues some data onto the target CPU's percpu queue. That's why when `(2)`
returns void, this all still makes sense - the target CPU will be interrupted
and can then pull functions off its queue to run.

But just to confirm that theory, let's check the target CPU codepath.

`CALL_FUNCTION_SINGLE_VECTOR`, if you grep for it, is mapped to the following
handler:

```c
DECLARE_IDTENTRY_SYSVEC(CALL_FUNCTION_SINGLE_VECTOR,        sysvec_call_function_single);
```

`sysvec_call_function_single()`'s call chain, in turn, is:

```
sysvec_call_function_single()
    generic_smp_call_function_single_interrupt()
        __flush_smp_call_function_queue(..)
            llist_for_each_entry_safe(..)
                csd_do_func(func, info, ..)
                    func(info)
```

That's basically it. I'm sure there's quite a bit more complexity under the
surface of this, but the high level view of the implementation is fairly
straightforward.

### Aside

I don't know if it's just me or not, but seeing and re-seeing basically the
same technique for running code on other CPUs is kind of a mind bender. You see
remotely appending to percpu queues quite often, for example with BPF
`cpumap`'s `struct ptr_ring` data structure or with the core networking stack's 
`enqueue_to_backlog()` as used by Receive Packet Steering (RPS):

```c
        cpu = get_rps_cpu(skb->dev, skb, &rflow);
        if (cpu < 0)
                cpu = smp_processor_id();
        
        ret = enqueue_to_backlog(skb, cpu, &rflow->last_qtail);
```

In userspace you would typically use threads and queues and maybe some kind of
signaling mechanism like a semaphore to keep a thread parked while waiting for
data. I'd have thought in the kernel there was a wildly different technique.
But at some level, IPIs and the regular userspace stuff are quite similar.


[0]: https://en.wikipedia.org/wiki/Inter-processor_interrupt
