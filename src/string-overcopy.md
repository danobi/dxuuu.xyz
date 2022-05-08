% Kernel string overcopying

While on a long roadtrip the past weekend, I somehow remembered one of the fun
bugs I fixed in the kernel a couple years ago.

A bug was reported to me about bpftrace: somehow there were duplicate map
entries for the same string key. So something like:

```
# sudo bpftrace -e '...  END { print(@) }'
^C

@[asdf]: 1
@[asdf]: 2
```

At first glance, this seemed like a bpftrace bug. I remember being confused for
a few hours until finally narrowing down the issue to the kernel. It turns out
the kernel had an optimized `strncpy()` routine where instead of copying a
single byte at a time and stopping when it saw a zero (NUL), it was doing
word-sized strides. This also meant that there could be extra bytes copied
_after_ the NUL.  This actually works perfectly fine for C use cases, but BPF
is a slightly environment and it turns out it _does_ matter.

### BPF maps

BPF hash maps store arbitrarily sized keys and values. In other words, they're
completely string agnostic. To simulate a string keyed map, bpftrace creates
the map with key size 64 bytes (configurable with `BPFTRACE_STRLEN` environment
variable) and just shoves bytes inside. Since the kernel map implementation is
string-oblivious, any hashing or comparison doesn't stop at a NUL byte -- it
keeps going for all 64 bytes.

Herein lies the issue. The BPF map implementation calls into
`strncpy_from_user()` which can overcopy. To C string functions (eg.
`strncmp()`, `strnlen()`), this is all kosher. However for a string-oblivious
hash table, semantically equivalent strings can occupy different hash table
entries.

### The fix

The fix was rather simple conceptually but took several iterations to reach.
Starting from [`6fa6d28051e9 ("lib/strncpy_from_user.c: Mask out bytes after NUL
terminator."`)][0], the kernel's `strncpy_from_user()` masks out trailing bytes
after the NUL terminator.

And since map keys should always start out zeroed, the above fix solves the
issue.


[0]: https://github.com/torvalds/linux/commit/6fa6d28051e9
