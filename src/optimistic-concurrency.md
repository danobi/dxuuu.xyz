% Optimistic concurrency control in ATS

[Optimistic concurrency control][0] is one of the tools ATS uses to create a high performance logging subsystem.
[For various reasons][1], mutexes were deemed to be too expensive to be used in the logging fast path. Instead,
ATS uses an optimistic commit/rollback strategy to synchronize shared memory. The most typical use of the
commit/rollback strategy in the logging subsystem is when the current [LogBuffer][2] is full and we need to allocate
a new LogBuffer. Since multiple threads could be writing to the current LogBuffer at any given time, ATS needs a
way to synchronize access to the current LogBuffer.

The pointer to the current LogBuffer is defined in [here][3]:

``` {#function .cpp .numberLines startFrom="298"}
volatile head_p m_log_buffer; // current work buffer
```

`head_p` is defined [in lib/ts/ink_queue.h][4]:

``` {#function .cpp .numberLines startFrom="86"}
#if (defined(__i386__) || defined(__arm__) || defined(__mips__)) && (SIZEOF_VOIDP == 4)
  typedef int32_t version_type;
  typedef int64_t data_type;
#elif TS_HAS_128BIT_CAS
  typedef int64_t version_type;
  typedef __int128_t data_type;
#else
  typedef int64_t version_type;
  typedef int64_t data_type;
#endif

  struct {
    void *pointer;
    version_type version;
  } s;

  data_type data;
} head_p;
```

where `s.pointer` is a pointer we want to serialize access to and `s.version` is a way to tell when `head_p` has
been modified. The version is necessary because `s.pointer` may retain the same value but refer to a different object
after an update. (Yes, it's technically possible if allocations and frees are done fast enough)

But why a union? The key insight here is that `data_type` and `struct s` are the same size. This means that we can
do an atomic [CAS][5] on `s` by simply referring to `head_p.data`. This lets us avoid complicated bit fiddling in favor
of being able to still do accesses like `head_p.s.pointer`. But wait, isn't this undefined? As it turns out, according
to the [C++ spec][6], it is in fact

> undefined behavior to read from the member of the union that wasn't most recently written.

However, that sentence is quickly followed by

> Many compilers implement, as a non-standard language extension, the ability to read inactive members of a union.

ATS is relying on non-standard language extensions, whoopee. That being said, ATS has been in use for the better part
of two decades, so if I were you I wouldn't start losing sleep over this just yet.

When we actually want to change the values held in `head_p`, we obey a pattern similar to this:

``` {#function .cpp .numberLines startFrom="415"}
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
```

There's a lot of macro magic going on here. To spare you the details, here's a quick summary of what each macro does:

`INK_QUEUE_LD(x, y)`: Atomic copy-by-value of `y` into `x`.

`FREELIST_POINTER(x)`: Maps to `x.s.pointer`.

`ink_atomic_increment(x, y)`: Atomically increments `x` by `y`.

`write_pointer_version(a, b, c, d)`: Atomic CAS between `a` and `b` with the new value being a  `head_p` with `s.pointer = c`
and `s.version = d`.

The entire do-while loop of goodness essentially guarantees that anything executed inside of the loop body is done atomically.
This is opportunistic because if another thread comes along and `m_log_buffer` right after we call `INK_QUEUE_LD()`, the
CAS inside `write_pointer_version(..)` will catch the change and abort the write. The loop repeats until we can atomically perform
the actions inside the loop body.

At first this may seem like a better, more lightweight solution over locks, it does come with certain drawbacks.

1. If the critical section is highly contested, then performance quickly degrades. This is because every failed transaction generates
more work, and more work will generate more failed transactions. On the other hand, mutexes will put the thread to sleep with the added
cost of a system call.

2. It is easy to create a [time to check to time of use][7] bug with this method. If we forget to wrap the expression in a do-while with
the correct terminating condition, we expose ourselves to a TOCTTOU bug. As with the C++ language itself, this form of concurrency
control gives the programmer a lot of power at the expense of naive safety.


[0]: https://en.wikipedia.org/wiki/Optimistic_concurrency_control
[1]: http://stackoverflow.com/questions/15056237/which-is-more-efficient-basic-mutex-lock-or-atomic-integer
[2]: https://github.com/apache/trafficserver/blob/master/proxy/logging/LogBuffer.h
[3]: https://github.com/apache/trafficserver/blob/master/proxy/logging/LogObject.h#L298
[4]: https://github.com/apache/trafficserver/blob/master/lib/ts/ink_queue.h#L86
[5]: https://en.wikipedia.org/wiki/Compare-and-swap
[6]: http://en.cppreference.com/w/cpp/language/union
[7]: https://en.wikipedia.org/wiki/Time_of_check_to_time_of_use
