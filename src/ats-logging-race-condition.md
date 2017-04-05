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

In this manner, since no two threads are writing in another thread's segment, we avoid race conditions on the actual logging. This also makes LogBuffer's critical
section extremely small. In fact, the only time we need to enter a critical section is when we do the book keeping to keep track of which segments are checked out.

When a thread wants to write a log entry, it does so by calling the global `Log::access(...)` function. `Log::access(...)` in turn does a bunch of work eventually
culminating in checking out a buffer segment from an active LogBuffer, serializing the log entry into the LogBuffer segment, and checking the LogBuffer segment
back in. Between the inclusive checkout and checkin operations is where I currently believe the bug to be.

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



[0]: https://github.com/apache/trafficserver
[1]: https://github.com/apache/trafficserver/tree/master/proxy/logging
[2]: https://github.com/apache/trafficserver/blob/master/proxy/logging/Log.h#L47
[3]: https://github.com/apache/trafficserver/blob/master/proxy/logging/LogBuffer.h
[4]: https://en.wikipedia.org/wiki/Compare-and-swap
[5]: https://github.com/apache/trafficserver/blob/master/proxy/logging/LogBuffer.cc#L270
[6]: https://github.com/google/sanitizers/wiki/ThreadSanitizerCppManual
