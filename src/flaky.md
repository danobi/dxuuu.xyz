% Flaky tests, or: why not to ignore mysteries

I spent a few weeks earlier this year [tracking down][1] a set of flaky
end-to-end tests where bpftrace would occasionally cease to print output.  I
had gotten as far as figuring out `std::cout` had [badbit][0] set after a write
but had run out of ideas on how to debug it. At the time, because I could not
reproduce it locally, I had assumed it was an oddity with pipes and CI and
given up. 

Except bugs never go away. They only lay dormant.

### The Return of the ~~King~~ Bug

This week the issue re-appeared. Except in the form of new flakiness after a
seemingly harmless [test reliability change][2]. I'll spare you the details -
just tedious printf bisection through many CI runs - but I essentially picked
up where I left off in March and isolated the buggy code down to:

```c++
LOG(DEBUG) << "before std::cout good=" << std::cout.good();
std::cout << fmt.format_str(arg_values) << std::endl;
LOG(DEBUG) << "after std::cout good=" << std::cout.good();
```

Note `stderr` (`LOG(DEBUG)`) continued to work while `stdout` did not. The
above code would show the following:

```
before std::cout good=1
after std::cout good=0
```

I'm not sure why I didn't think to drop down to syscall level and actually read
[the errno][3] last time, but fortunately I had a fresh chance to redeem myself:

```c++
auto ret = write(STDOUT_FILENO, fmted.c_str(), fmted.size());
if (ret > 0) {
    LOG(DEBUG) << "write was ok";
} else if (ret == 0) {
    LOG(DEBUG) << "write wrote nothing?";
} else {
    LOG(DEBUG) << "write failed: errno=" << errno << ": " << strerror(errno);
}
```

The above revealed:

```
write failed: errno=9: Bad file descriptor
```

`EBADF` is interesting. According to the `write(2)` man page:

> EBADF  fd is not a valid file descriptor or is not open for writing.

To not rule out either case out of hand, I opted to `lsof` on bpftrace startup
as well as right before the `write(2)`. I used this chunk of code:

```c++
char buf[256];
snprintf(buf, sizeof(buf), "lsof -p %d 1>&2", getpid());
system(buf);
```


On startup, we see:

```
[..]
bpftrace 7526 root   0r  FIFO   0,14       0t0   16880 pipe
bpftrace 7526 root   1w  FIFO   0,14       0t0   28011 pipe
bpftrace 7526 root   2w  FIFO   0,14       0t0   28011 pipe
[..]
```

but later at the write:

```
[..]
bpftrace 7526 root   0r     FIFO   0,14       0t0   16880 pipe
bpftrace 7526 root   2w     FIFO   0,14       0t0   28011 pipe
[..]
```

This can only mean something inside the process is closing FD 1.

### Clear and present data

bpftrace works with raw file descriptors quite often. So it wasn't really
practical to audit every `close(2)` callsite. Not to mention all the
dependencies. Therefore, I opted for some runtime instrumentation by scripting
GDB. I originally had bpftrace to do this, but due to all the debug logs the
output started getting confusing - multiple copies of everything was getting
logged.

Regardless, I switched the end-to-end test invocation to this:

```
RUN gdb -q -ex "set confirm off" -ex "break close if \$rdi==1" -ex "commands;backtrace;kill;end" -ex run --args {{BPFTRACE}} -e 'uprobe:/proc/{{BEFORE_PID}}/root/uprobe_test:uprobeFunction1 { printf("func %s\n", func); exit() }'
```

What this snippet does is set a breakpoint on `close(2)` but only dump
backtrace if the argument value is `1`.

Running that in CI reveals:

```
Attaching 1 probe...
__BPFTRACE_NOTIFY_PROBES_ATTACHED

Thread 1 "bpftrace" hit Breakpoint 1.2, 0x00007fffe4504910 in close ()
   from /nix/store/wn7v2vhyyyi6clcyn0s9ixvl7d4d87ic-glibc-2.40-36/lib/libc.so.6
(gdb)
```

The lack of a stack seemed to indicate missing debug information. However,
these CI jobs configure bpftrace to be built with DWARF. Therefore, I started
to suspect one of bpftrace's dependencies.

### Phantom thread

The idea that it could be one of bpftrace's dependencies sparked an interesting
question: what do all these failing tests have in common? Given that the above
test case was really only using the `func` builtin, I checked how that feature
is implemented. To my surprise, it's implemented through userspace
symbolication!

That makes the common thread userspace symbolication, as the original 4
disabled tests involve symbolizing userspace stacks. So the obvious next place
to look is `bcc`.

Upon inspection, there's indeed a bit of suspicious looking code in
`ProcSyms::ModulePath::fd_` where `fd_` is declared without a default value and
`ModulePath()` has an early return that does not set `fd_`. In turn,
`~ModulePath()` does `if (fd_ > 0) close(fd_)` which would be buggy if `fd_` is
not initialized to `-1`.

It's also more clear why [the above reliability change][2] started triggering
flakiness. It’s b/c before the change, `uprobe_test` probably exited before
symbolization occurred, so we were not tickling the bad codepath in bcc. After
the change the process hangs around, so symbolization does occur.

### The Validator

Continuing with the GDB approach, I thought if I could turn on debug
information for bcc it could give me a second opinion on the source. Through
our [Nix based CI][4], this was as easy as adding to the bcc package overlay:

```
cmakeFlags = old.cmakeFlags ++ [ "-DCMAKE_BUILD_TYPE=Debug" ];
```

Unfortunately, stacks still did not work. I'm guessing it's b/c glibc doesn't
have frame pointers enabled. And changing that would probably take too much compute
to rebuild everything. So it was a matter of luck that I started thinking about
dependencies - the lack of stacks is unrelated.

So I opted to check a different way: through printf debugging the dependency.
Through the power of Nix, I applied a patch to bcc:

```diff
From e5609182cf0ede7c9bb32bf54670cbfba754a6c0 Mon Sep 17 00:00:00 2001
Message-ID: <e5609182cf0ede7c9bb32bf54670cbfba754a6c0.1737135820.git.dxu@dxuuu.xyz>
From: Daniel Xu <dxu@dxuuu.xyz>
Date: Fri, 17 Jan 2025 10:43:33 -0700
Subject: [PATCH] XXX: Log ~ModulePath close()

Signed-off-by: Daniel Xu <dxu@dxuuu.xyz>
---
 src/cc/syms.h | 2 ++
 1 file changed, 2 insertions(+)

diff --git a/src/cc/syms.h b/src/cc/syms.h
index 3dffdda7..415fe28f 100644
--- a/src/cc/syms.h
+++ b/src/cc/syms.h
@@ -16,6 +16,7 @@
 #pragma once

 #include <algorithm>
+#include <iostream>
 #include <memory>
 #include <string>
 #include <sys/types.h>
@@ -144,6 +145,7 @@ class ProcSyms : SymbolCache {
       return proc_root_path_.c_str();
     }
     ~ModulePath() {
+      std::cerr << "XXX BCC: trying to ~ModulePath() close fd_=" << fd_ << std::endl;
       if (fd_ > 0)
         close(fd_);
     }
--
2.47.1
```

And to the flake:

```
patches = old.patches ++ [
    # Need to use pkgs.fetchpatch for remote patches or pkgs.writeText for local ones
    (pkgs.writeText "my-patch" (builtins.readFile ./0001-XXX-Log-ModulePath-close.patch))
];
```

From this, we finally get:

```
XXX BCC: trying to ~ModulePath() close fd_=1
```

So mystery solved. To validate, I applied this patch through the same mechanism:

```diff
From 02fffe05dec4eca98da053d8f6d4741a758c184d Mon Sep 17 00:00:00 2001
Message-ID: <02fffe05dec4eca98da053d8f6d4741a758c184d.1737137000.git.dxu@dxuuu.xyz>
From: Daniel Xu <dxu@dxuuu.xyz>
Date: Fri, 17 Jan 2025 11:02:55 -0700
Subject: [PATCH] XXX: FIX

Signed-off-by: Daniel Xu <dxu@dxuuu.xyz>
---
 src/cc/syms.h | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)

diff --git a/src/cc/syms.h b/src/cc/syms.h
index 415fe28f..b9563e87 100644
--- a/src/cc/syms.h
+++ b/src/cc/syms.h
@@ -131,7 +131,7 @@ class ProcSyms : SymbolCache {
     // process by storing a file descriptor created from openat(2) if possible
     // if openat fails, falls back to process-dependent path with /proc/.../root
    private:
-    int fd_;
+    int fd_ = -1;
     std::string proc_root_path_;
     std::string path_;

--
2.47.1
```

A 90 minute bake (failures usually appeared after 5 minutes) shows no failures.

### Conclusions

This is a really nasty bug. Arbitrarily closing file descriptors as a library
dependency is very not nice. Anything can happen. For bpftrace, this is
particularly risky, as bcc could've been closing probe file descriptors which
would lead to silent data loss.

One takeaway I have is that as a tool's developer, we are best positioned to
debug crazy mysteries like this. Our users likely have very little chance of
figuring this stuff out. So when we find an alarming mystery (like truncated
output!), we should be doing our best to chase it to its conclusion.

Also that making puns out of movie titles got pretty hard after 2 titles.


[0]: https://en.cppreference.com/w/cpp/io/ios_base/iostate
[1]: https://github.com/bpftrace/bpftrace/issues/3080
[2]: https://github.com/bpftrace/bpftrace/commit/c3cb6d6d1295316bf877dce33922c467d467de37
[3]: https://dxuuu.xyz/errno.html
[4]: https://dxuuu.xyz/bpftrace-nix.html
