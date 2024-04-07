% Reading local variables with bpftrace

Most tracers are designed around functions. You attach a probe to a function
and then when the function is called and the probe is run, you access the
function arguments and/or the return value. Function arguments are sufficient
for the majority of use cases, as function bodies typically process and
manipulate the arguments. But sometimes they are not enough. Sometimes we want
to access the local variables defined in the function body.

This post walks through a hypothetical example of accessing local variables.
More specifically, we will use `bpftrace` to trace bpftrace.

### Async IDs

Whenever bpftrace needs userspace to take an action (such as printing a message
to console), we must have kernelspace send a message to userspace. In order to
coordinate actions, we assign unique IDs to all possible actions.  For example,
each `print()` or `printf()` will have its own unique ID. Userspace and
kernelspace share this map of ID to action. Thus, when kernelspace sends up a
message, it prefixes the blob with the ID so userspace can know how to
interpret the blob.

Now that we know that async IDs are critical to bpftrace machinery, let's find
a way to trace it. For legacy reasons, in source it's called `printf_id`.  The
editorialized source follows:

```c++
void perf_event_printer(void *cb_cookie, void *data, int size)
{
  std::vector<uint8_t> data_aligned;
  data_aligned.resize(size);
  memcpy(data_aligned.data(), data, size);

  int err;
  auto bpftrace = static_cast<BPFtrace *>(cb_cookie);
  auto arg_data = data_aligned.data();
  auto printf_id = *reinterpret_cast<uint64_t *>(arg_data);

  if (bpftrace->finalize_)
    return;
  if (bpftrace->exitsig_recv) {
    bpftrace->request_finalize();
    return;
  }

  if (printf_id == asyncactionint(AsyncAction::exit)) {
    bpftrace->request_finalize();
    return;
  } else if (printf_id == asyncactionint(AsyncAction::print)) {
    [...]
  } else if (printf_id == asyncactionint(AsyncAction::print_non_map)) {
    [...]
  } else if (printf_id == asyncactionint(AsyncAction::clear)) {
    [...]
  } [...]
```

Notice how it'd be tricky to derive the ID only from function arguments.
If we tried to, we'd risk taking page faults or accessing unaligned memory.

### Technique

The basic technique for accessing a local variable is to:

1. Find where in the function the variable is live
1. Find which register the variable is live in
1. Attach a probe at correct offset reading the correct register

The hand-wavy explanation for this (see [register allocation][0] for a more
detailed discussion) is that variables, when used in an operation, must be in a
register. And since there can be a lot more variables than registers, the
compiler only keeps a subset of variables in registers at any given time and
leaves the rest on the stack. As such, _when_ (offset) a variable is "live" is
just as important as _where_ (register) is live.

### Finding our register

I wouldn't say I'm particularly good at reading assembly, so don't worry
if you're not good either. Here I'll show off a neat trick that helps cut
through all the noise.

But first we must disassemble. There's many ways to do this, but I'll be using
`nm` and `objdump`.

```
$ nm /usr/bin/bpftrace | grep perf_event_printer
00000000000ecb00 t _ZN8bpftrace18perf_event_printerEPvS0_i
0000000000020765 t _ZN8bpftrace18perf_event_printerEPvS0_i.cold
```

We'll ignore the `.cold` function since I think that's where the compiler keeps
code unlikely to be executed (in order to reduce icache footprint).

```
$ objdump --disassemble=_ZN8bpftrace18perf_event_printerEPvS0_i /usr/bin/bpftrace

/usr/bin/bpftrace:     file format elf64-x86-64


Disassembly of section .init:

Disassembly of section .text:

00000000000ecb00 <_ZN8bpftrace18perf_event_printerEPvS0_i>:
   ecb00:       f3 0f 1e fa             endbr64
   ecb04:       41 57                   push   %r15
   ecb06:       41 56                   push   %r14
   ecb08:       41 55                   push   %r13
   ecb0a:       41 54                   push   %r12
   ecb0c:       55                      push   %rbp
   ecb0d:       53                      push   %rbx
   ecb0e:       48 81 ec 00 10 00 00    sub    $0x1000,%rsp
   ecb15:       48 83 0c 24 00          orq    $0x0,(%rsp)
   ecb1a:       48 81 ec e8 06 00 00    sub    $0x6e8,%rsp
   ecb21:       66 0f ef c0             pxor   %xmm0,%xmm0
   ecb25:       64 48 8b 04 25 28 00    mov    %fs:0x28,%rax
   ecb2c:       00 00
   ecb2e:       48 89 84 24 d8 16 00    mov    %rax,0x16d8(%rsp)
   ecb35:       00
   ecb36:       31 c0                   xor    %eax,%eax
   ecb38:       48 63 ea                movslq %edx,%rbp
   ecb3b:       49 89 ff                mov    %rdi,%r15
   ecb3e:       48 89 f3                mov    %rsi,%rbx
   ecb41:       48 c7 44 24 70 00 00    movq   $0x0,0x70(%rsp)
   ecb48:       00 00
   ecb4a:       49 89 ec                mov    %rbp,%r12
   ecb4d:       31 ff                   xor    %edi,%edi
   ecb4f:       0f 29 44 24 60          movaps %xmm0,0x60(%rsp)
   ecb54:       48 85 ed                test   %rbp,%rbp
   ecb57:       0f 85 5b 03 00 00       jne    eceb8 <_ZN8bpftrace18perf_event_printerEPvS0_i+0x3b8>
   ecb5d:       48 89 ea                mov    %rbp,%rdx
   ecb60:       48 89 de                mov    %rbx,%rsi
   ecb63:       ff 15 47 8e 33 00       call   *0x338e47(%rip)        # 4259b0 <memcpy@GLIBC_2.14>
   ecb69:       4c 8b 6c 24 60          mov    0x60(%rsp),%r13
   ecb6e:       45 0f b6 77 28          movzbl 0x28(%r15),%r14d
   ecb73:       49 8b 45 00             mov    0x0(%r13),%rax
   ecb77:       45 84 f6                test   %r14b,%r14b
   ecb7a:       0f 85 20 03 00 00       jne    ecea0 <_ZN8bpftrace18perf_event_printerEPvS0_i+0x3a0>
   ecb80:       8b 2d 22 ac 33 00       mov    0x33ac22(%rip),%ebp        # 4277a8 <_ZN8bpftrace8BPFtrace12exitsig_recvE>
   ecb86:       85 ed                   test   %ebp,%ebp
   ecb88:       0f 85 72 02 00 00       jne    ece00 <_ZN8bpftrace18perf_event_printerEPvS0_i+0x300>
   ecb8e:       48 3d 30 75 00 00       cmp    $0x7530,%rax
   ecb94:       0f 84 46 05 00 00       je     ed0e0 <_ZN8bpftrace18perf_event_printerEPvS0_i+0x5e0>
   ecb9a:       48 3d 31 75 00 00       cmp    $0x7531,%rax
   ecba0:       0f 84 f2 04 00 00       je     ed098 <_ZN8bpftrace18perf_event_printerEPvS0_i+0x598>
   ecba6:       48 3d 37 75 00 00       cmp    $0x7537,%rax
   ecbac:       0f 84 1e 03 00 00       je     eced0 <_ZN8bpftrace18perf_event_printerEPvS0_i+0x3d0>
   ecbb2:       48 3d 32 75 00 00       cmp    $0x7532,%rax
   ecbb8:       0f 84 82 05 00 00       je     ed140 <_ZN8bpftrace18perf_event_printerEPvS0_i+0x640>
   ecbbe:       48 3d 33 75 00 00       cmp    $0x7533,%rax
   ecbc4:       0f 84 de 0a 00 00       je     ed6a8 <_ZN8bpftrace18perf_event_printerEPvS0_i+0xba8>
   ecbca:       48 3d 34 75 00 00       cmp    $0x7534,%rax
   ecbd0:       0f 84 0a 09 00 00       je     ed4e0 <_ZN8bpftrace18perf_event_printerEPvS0_i+0x9e0>
   ecbd6:       48 3d 35 75 00 00       cmp    $0x7535,%rax
   ecbdc:       0f 84 fe 0c 00 00       je     ed8e0 <_ZN8bpftrace18perf_event_printerEPvS0_i+0xde0>
   ecbe2:       48 3d 36 75 00 00       cmp    $0x7536,%rax
   ecbe8:       0f 84 fb 0f 00 00       je     edbe9 <_ZN8bpftrace18perf_event_printerEPvS0_i+0x10e9>
   ecbee:       48 3d 39 75 00 00       cmp    $0x7539,%rax
   ecbf4:       0f 85 3e 0e 00 00       jne    eda38 <_ZN8bpftrace18perf_event_printerEPvS0_i+0xf38>
   [...]
```

Experienced assembly programmers can probably read this and make sense of the
structure. But if you're like me and can only nod along to explanations, you
can use this trick:

Basically, you're going to look for a constant value that a variable is being
compared to. This creates a mapping between source to assembly.

In our example, notice the following code:

```c++
  if (printf_id == asyncactionint(AsyncAction::exit)) {
```

Also notice `AsyncAction::exit` is a constant (`30000`):

```c++
enum class AsyncAction {
  printf  = 0,     // printf reserves 0-9999 for printf_ids
  syscall = 10000, // system reserves 10000-19999 for printf_ids
  cat     = 20000, // cat reserves 20000-29999 for printf_ids
  exit    = 30000,
  print,
  clear,
  zero,
  time,
  join,
  helper_error,
  print_non_map,
  strftime,
  watchpoint_attach,
  watchpoint_detach,
  skboutput,
};
```

Converting that to hex, we get:

```
>>> hex(30000)
'0x7530'
```

And when we check the assembly for that constant, we see:

```
   ecb8e:       48 3d 30 75 00 00       cmp    $0x7530,%rax
   ecb94:       0f 84 46 05 00 00       je     ed0e0 <_ZN8bpftrace18perf_event_printerEPvS0_i+0x5e0>
```

This tells us both _when_ and _where_ our `printf_id` is live. To get the offset,
we do:

```
>>> 0xecb8e - 0xecb00
142
```

`0xecb8e` is the offset of the `cmp` instruction. And `0xecb00` is the offset of
the function (see `objdump` output).

To get the register the variable is live in, look at the other operand in
`cmp`: `%rax`.

### Putting it all together

We have two probes: `BEGIN` and our accessor. We use the `BEGIN` probe as a
trigger to actually run `BPFtrace::perf_event_printer`. This makes the example
a nice one liner. The accessor probe uses all the information we gathered.

```
$ sudo bpftrace - <<EOF
    BEGIN { print("trigger!") }
    uprobe:/usr/bin/bpftrace:_ZN8bpftrace18perf_event_printerEPvS0_i+142 { print(reg("ax")); exit() }
EOF
Attaching 2 probes...
trigger!
30007
```

The script is telling us the value of our local variable is `30007`. Consulting
the source, we see `30007` maps to `AsyncAction::print_non_map`. This checks
out -- the `BEGIN` probe indeed uses a non-map print.


[0]: https://en.wikipedia.org/wiki/Register_allocation
