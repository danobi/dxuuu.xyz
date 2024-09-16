% Big strings in bpftrace

If you’ve ever written BPF code you’ve probably run into the limits of the BPF
programming model. bpftrace, a high level tracing language built on BPF, does
its best to hide these limits – usually through language design but sometimes
through clever code generation. Even so, some fundamental issues still leak
through.

One such example is with the string type.

## BPF memory

The BPF programming model makes an implicit distinction between kernel memory,
user memory, and BPF memory. BPF memory is memory the verifier knows is safe to
access. This includes the BPF stack, map allocated memory, etc. In order to
work with data originating from the first two categories, a BPF program first
needs to pull it into BPF memory through `bpf_probe_read_[user|kernel][_str]()`
helper functions. Sometimes the line is blurry. With BPF Type Format (BTF) the
verifier can type check and ensure the memory being read is safely paged-in and
in-bounds. For example, if the verifier can prove you have a pointer to valid
kernel memory, the program can directly use the pointer in string functions
without the aforementioned helpers.

To paper over such differences and expose a unified interface, bpftrace users
are required to call `str()` on C-strings to get a handle to a string they can
interact with. For example, assuming `foo` is a kernel function declared as
`void foo(char *s)`, the following bpftrace program counts the occurrences of
every string passed to `foo` and stores the results into map `@occurences`:

```
# bpftrace -e ‘kfunc:foo { @occurences[str(args.s)] = count() }’
```

## BPF memory limitations

Because there is no way to efficiently allocate dynamic BPF memory (at least,
until [very recently][1]) in BPF program context, all strings need to be read
onto the stack. However, the verifier imposes both a 512 byte stack limit as
well as a prohibition on variable sized stack allocations. The implication
being that all strings need to be pre-sized at compile time - a classic
right-sizing problem.

As a result, bpftrace now has the now infamous `BPFTRACE_MAX_STRLEN`
configuration. This knob defaulted to 64 bytes and was tunable up to around 200
bytes, at which point both LLVM and the verifier would start complaining. In
the above example, any string over 63 bytes (excluding NUL) would be summarily
truncated. Any experienced programmer can tell you 63 bytes (or even 200) is
not nearly enough for real world strings.

Even so, this has been the status quo for some number of years… until now.


## Everything is a map

Astute readers might wonder: in the above example, why does the string need to
exist on the stack at all? Can’t it just be read directly into `@occurences`’s
storage, where there are no size limitations? The answer is perhaps one day, if
the kernel adds support for it. The current CRUD interface to BPF maps requires
pointers for keys and values, thus implying storage _somewhere_ in BPF memory.

This is a good thought - is there somewhere else in BPF memory without size
limitations we can read the string into? The answer, as with many questions in
BPF, is to use a map.

The 512 byte stack limit is a well known limitation in BPF. While there are new
[developments in this area][0], the long-standing workaround has been to use an
array map sized to a single entry as scratch space. At load time, the verifier
rewrites array map lookups to regular memory accesses (without the source-level
helper call). This means that performance remains good despite appearances.

So what if we created an array map with a single entry of arbitrary
`BPFTRACE_MAX_STRLEN`? Well, it wouldn’t work as a single global, as the same
program can concurrently run on different CPUs. So it would have to be a
per-cpu array map, which the kernel already supports. But consider the
following case:

```
# bpftrace -e ‘kfunc:bar { @m[str(args.s)] = str(args.s2) }’
```

The update to map `@m` is a single `bpf_map_update_elem()` call. Meaning both
strings have to be alive at the same time. In other words, we have an
overlapping lifetime problem for that single array entry. This is the key issue
that has blocked progress on big strings for so long.


## One entry per node

Fortunately  there is a rather simple and general (as you’ll see) solution
lurking in plain sight. Consider the following script:

```
$s = str(args.s);                           // (1)
@m[$s, str(args.s2)] = str(args.s3);        // (2), (3)
$i = 0;
while ($i < 5) {
    @m2[$i] = str(args.s4);                 // (4)
    $i++;
}
```

This program creates 8 strings at runtime but only 4 unique storage slots are
necessary to avoid all potential lifetime conflicts. To accurately count and
assign slots, a compiler need only walk the abstract syntax tree (AST) and
assign each `str()` node an increasing index or id. As long as bpftrace is
careful to load strings into their assigned slot, the whole thing will work.
Note it is _very_ simple for
any compiler to walk and count nodes like this.

Once all the slots are tallied up, we can create a per-cpu array map with
`nr_slots` entries sized to any arbitrary `BPFTRACE_MAX_STRLEN` size. Note this
size is basically unlimited now - multiple gigabytes should work as long as you
have enough memory. The downside to this approach is that we are
pessimistically pre-allocating memory. Strings can be smaller than
`BPFTRACE_MAX_STRLEN` or worse: the strings may never be created. In this case,
we’ve found in practice that our users are happy to trade some wasted memory for
smoother debugging sessions.


## Printing big strings

While assigning storage slots to each string is a novel trick, its
implementation exposed a few long standing assumptions. The biggest challenge
was printing big strings. For example, consider:

```
# bpftrace -e ‘kfunc:foo { printf(“%s, %s\n”, str(args.s), str(args.s)) }’
```

In this case, the program will allocate a two string “tuple” on the stack, copy
both strings into the tuple, and then copy the tuple into a ring buffer for
userspace to consume. Note that the stack allocation is not strictly necessary
for [BPF ring buffer][2] (as it allows reserving and committing buffer space),
but for simplicity we try to keep perf and BPF ring buffer support on as
similar codepaths as possible.

However, `printf()` (and also `print()`) were implemented assuming that all the
arguments can fit neatly on the stack. Now that strings do not necessarily fit
on the stack, we must rework the printing code.

Fortunately, we can just reuse the same per-cpu buffer trick as with big
strings. Except this time there are no lifetime overlap issues so we can simply
give the per-cpu array one entry sized to the largest tuple the program can
create. Determining the largest tuple is also easy for a compiler - walk the
AST again and keep the running max.


## Conclusion

Now that the big issues (as well as some smaller ones we’ve glossed over) are
solved, bpftrace plans to change the default value of `BPFTRACE_MAX_STRLEN` up
to 1024 bytes. Assuming all goes well, scripts limited by small strings will
magically start working better and new users will be none the wiser about any
underlying BPF limitations.


[0]: https://lore.kernel.org/all/20240716011647.811746-1-yonghong.song@linux.dev/
[1]: https://lore.kernel.org/all/20240308010812.89848-1-alexei.starovoitov@gmail.com/
[2]: https://docs.kernel.org/6.6/bpf/ringbuf.html
