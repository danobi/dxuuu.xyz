% Tracking down a race condition in the Apache Trafficserver logging subsystem

TODO: make this post more coherent

Lately at work, I've been tracking down a log corruption issue inside of [Trafficserver's][0]
[logging subsystem][1]. The issue is that there's often random data inserted into the middle of log statements. Specifically, these are access logs
that a lot of Yahoo's analytics are based off of, so this is kind of an important issue.

Now before we get into the details of the bug, there are a few things I have to say about the logging subsystem. I've not had too much experience
with the rest of the code, but ATS is one of those old code bases where documentation is very sparse. In some cases, the only existing documentation is
[flat out wrong][2]. Furthermore, when Yahoo open sourced this project about 8 years ago, the project lost all of the commit history. This means that
git-blame will take us only as far back as the initial open source commit in 2009. This makes for a very unpleasant time tracking down the reason
a particular section of code is the way it is.

Now back to the bug. To make things even more challenging, this corruption issue only occurs under high load. What I'm really trying to say is that
this bug not only happens in production, but also in a very specific region of the United States. Fancy that. Needless to say, this isn't something I can
debug live, much less reproduce locally. For those of you that have experienced this pain before, you're probably screaming "RACE CONDITION".
Unfortunately, you're probably right. Seeing as I haven't quite solved this bug yet, I'll keep this post updated with all of the strategies and dead ends
I come across.

## The LogBuffer

There's a lot of interesting design choices that go into making a high performance logging system. I'm not going to claim I'm some expert (yet), so I'll just
jot down the things I've noticed and/or surmised.

1. Heap allocations are expensive.

    * Heap allocations are many times more expensive than stack allocations, so we want to be avoiding heap allocations as often as we can. This kind of incentive
    often leads to things like memory pools and buffer spaces.

2. Excessivily writing to disk is expensive.

    * We don't want to write each individual log line to disk immediately, since that incurs significant overhead. Instead, we want to buffer a bunch of log entries
    and flush them all at once. This is where the LogBuffer comes in.

The [LogBuffer][3] class is designed to provide a thread-safe mechanism to store log entries before they're flushed. To reduce system call overhead, LogBuffers
are designed to avoid heavy-weight mutexes in favor of using lightweight atomics built on top of [compare-and-swap][4] [operations][5]. When a caller wants
to write into a LogBuffer, the caller "checks out" a segment of the buffer to write into. LogBuffer makes sure that no two callers are served overlapping segments.
To illustrate this point, consider this diagram of a buffer:

```
          +--------------------------------+
          | thread_1's segment             |
          |--------------------------------|
          | thread_2's segment             |
          |                                |
          |                                |
          |--------------------------------|
          | thread_3's segment             |
          |                                |
          |                                |
          |                                |
          |--------------------------------|
          | thread_4's segment             |
          |--------------------------------|
          | <unused>                       |
          |                                |
          |                                |
          |                                |
          |                                |
          |                                |
          |                                |
          |                                |
          +--------------------------------+
```

In this manner, since no two threads are writing in the other's segment, we avoid race conditions on the actual logging. This also makes LogBuffer's critical
section extremely small. In fact, the only time we need to enter a critical section is when we do the book keeping to keep track of which segments are checked out.

When a thread wants to write a log entry, it does so by calling the global `Log::access(...)` function. `Log::access(...)` in turn does a bunch of work eventually
culminating in checking out a buffer segment from an active LogBuffer, serializing the log entry into the LogBuffer segment, and checking the LogBuffer segment
back in. Between the inclusive checkout and checkin operations is where I currently believe the bug to be.

## Lead 1

The first step I took to try and narrow down this bug was to run [TSan][6] on a local instance. TSan suggested that (1) could be an issue:

``` {#function .cpp .numberLines startFrom="1"}
LogBuffer::LB_ResultCode
LogBuffer::checkout_write(size_t *write_offset, size_t write_size)
{
  ...

  do {
    new_s = old_s = m_state;  // (1)

    if (old_s.s.full) {       // (2)
      break;
    } else {
      ++new_s.s.num_writers;
      new_s.s.offset += actual_write_size;
      ++new_s.s.num_entries;

      ...

      // essentially cas(m_state, old_s, new_s)
      if (switch_state(old_s, new_s)) {   // (3)
        // we succeded in setting the new state
        break;
      }
    }
  } while (--retries);

  ...
}
```

After reasoning with this code for a bit, I realized that if `m_state` was changed by another thread between assignments of `new_s` and `old_s`,
then this could very well explain the data corruption issue I was facing. The reason being is that another thread could have given away the last available segment of the
current LogBuffer object and marked `m_state` as full. However, our current thread's `old_s` still reports the state as being NOT full on (2). I let myself celebrate for a few
minutes before realizing it couldn't be _this_ easy. Unfortunately I was right.

My previous assumtion that `new_s` and `old_s` could hold different values was wrong. If you decompile the expression `new_s = old_s = m_state`, you'll see something like this:

```
        movl    -20(%rbp), %eax
        movl    %eax, -4(%rbp)
        movl    -4(%rbp), %eax
        movl    %eax, -8(%rbp)
```

where `-4(%rbp)` is `old_s` and `-8(%rbp)` is `new_s`. What this snippet of assembly is saying is essentially this:

```
        %eax = m_state
        old_s = %eax
        %eax = old_s
        new_s = %eax
```

Too bad for me, this means that `old_s` and `new_s` **have** to be the same value. Dead end :(. However, I'm not convinced the race condition _isn't_ in LogBuffer just yet.


## Lead 2

After delving into the depths of the LogBuffer code, I felt safe in assuming there weren't any bugs in that area of the codebase. So I took a step back and examined the
LogObject code. The way LogObjects and LogBuffers interact is not unnatural. The general gist of it is that each LogObject instance represents one logical logging
"object" in ATS. For example, if there was one access log in squid format and another as a named pipe, then ATS would be keeping track of two LogObjects. The details
of where all the bits and bytes should go are encapsulated in the LogObject implementation. This is orthogonal to the LogBuffer because as we discussed earlier, the
LogBuffer is a mechanism to prevent excessive disk flushes whereas the LogObject is a logical object.

I also recently wrote an architectural overview of the ATS logging subsystem. As of the time of this writing, the pull request is sitting in a review queue in the
upstream project. Hopefully when it's merged, I'll remember to update it here.

Now to the details of the issue. Every LogObject holds on to at most one LogBuffer at any given time. The LogObject uses the LogBuffer to cache writes into the
filesystem/network/OS. It is safe to assume that at some point every LogBuffer will get full. When said LogBuffer is full, we need to 1) flush the LogBuffer
to some underlying structure and 2) allocate a new LogBuffer for the LogObject and do an atomic swap. The following code is used to swap the old LogBuffer
with the freshly allocated LogBuffer:


``` {#function .cpp .numberLines startFrom="407"}
 case LogBuffer::LB_FULL_NO_WRITERS:
      // no more room in current buffer, create a new one
      new_buffer = new LogBuffer(this, Log::config->log_buffer_size);

      // swap the new buffer for the old one
      INK_WRITE_MEMORY_BARRIER;
      head_p old_h;

      do {
        INK_QUEUE_LD(old_h, m_log_buffer);
        if (FREELIST_POINTER(old_h) != FREELIST_POINTER(h)) {
          ink_atomic_increment(&buffer->m_references, -1);

          // another thread should be taking care of creating a new
          // buffer, so delete new_buffer and try again
          delete new_buffer;
          break;
        }
      } while (!write_pointer_version(&m_log_buffer, old_h, new_buffer, 0));

     if (FREELIST_POINTER(old_h) == FREELIST_POINTER(h)) {
        ink_atomic_increment(&buffer->m_references, FREELIST_VERSION(old_h) - 1);

        int idx = m_buffer_manager_idx++ % m_flush_threads;
        Debug("log-logbuffer", "adding buffer %d to flush list after checkout", buffer->get_id());
        m_buffer_manager[idx].add_to_flush_queue(buffer);
        Log::preproc_notify[idx].signal();
        buffer = nullptr;
      }

      ...
```

If we look closely at line 417, we notice that we identify potential race conditions very strangely. So the current thread, thread 1, attemps to swap
the old LogBuffer with the new LogBuffer. Suppose that another thread, thread 2, is also doing the same thing at the same time. Only one of these
operations should succeed because otherwise we start leaking memory. So naturally, thread 1 needs a mechanism to detect when a LogBuffer has been pulled
out from underneath it. On line 417, we see that we identify this pulling out by comparing pointers. It makes the implicit assumption that if the pointer
value has changed, then the LogBuffer must have changed. While this logic is sound in the "forward direction" (ie if the pointer changes then the buffer
has changed), the converse is not true. Suppose we free the old LogBuffer and then immediately allocate another LogBuffer. It is conceivable that the
pointer will remain the same. That is to say, if the LogBuffer has changed, there is no guarantee that the pointer to the new LogBuffer will be any
different than the old pointer. You might imagine this can cause a few issues. And while you're probably not wrong, the truth is equally unappealing.

As it turns out, we always allocate a new LogBuffer before we free the old LogBuffer. On line 409, we allocate the new LogBuffer. Later on in line
432, we free the old LogBuffer. If these operations are sequential (and we have no reason to believe otherwise), then no race condition is possible.
While this was an interesting examination into the LogBuffer swapping code, it was ultimately another dead end.


## The end... for now?

It's been about a week and a half into the investigation. I thought it might be good to get some fresh samples of the log corruption to see if anything
new or novel has happened to further my debugging, so I asked for some new samples. As it turns out, the issue has stopped happening in production.
What this means is that even if I found a potential fix, there would be no way to verify the effectiveness of the fix. That means if there's not an obvious
problem, then there isn't an obvious solution.

So until this issue crops up again, work on this bug has to be put on hold.




[0]: https://github.com/apache/trafficserver
[1]: https://github.com/apache/trafficserver/tree/master/proxy/logging
[2]: https://github.com/apache/trafficserver/blob/master/proxy/logging/Log.h#L47
[3]: https://github.com/apache/trafficserver/blob/master/proxy/logging/LogBuffer.h
[4]: https://en.wikipedia.org/wiki/Compare-and-swap
[5]: https://github.com/apache/trafficserver/blob/master/proxy/logging/LogBuffer.cc#L270
[6]: https://github.com/google/sanitizers/wiki/ThreadSanitizerCppManual
