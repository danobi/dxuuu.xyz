% GDB hacking

This past weekend I spent some time hacking on GDB where the overall goal was
to improve rust debugging support. I considered hacking on LLDB but their
upstream stance on language support is [fairly conservative][0]. The current
state is that they want non C-family languages to be maintained out of tree.
Anyone who's done open source software maintenance knows that's an enormous
burden on a team -- to say nothing of a single person. In fact, such an out of
tree LLDB has been attempted before but the sole maintainer got a new job and
the patches ended up being [too difficult to rebase][2]. So support was
dropped. And that's how I decided to hack on GDB.

Despite being released [over 30 years ago][1], the codebase is still in fairly
great shape. In fact, the entire GDB codebase uses C++11. The current
developers/maintainers have obviously taken great care to keep things modern.
This is evidenced by the tasteful use of C++11 features (smart pointers,
`override`, limited use of templates).

It took about a day, but I did manage to get a couple small patches out. The
only patch worth noting is [adding support for raw identifiers][3], a fairly
useless but educational feature. My hope is to steadily send patches for at
least a little while.

To end this post, I'll share one cool trick: debugging GDB with GDB:

```
$ cd ~/dev/gdb/build/gdb
$ make GDBFLAGS=./gdb run
[...]

(top-gdb) b print_command_1
Breakpoint 3 at 0xd7374: print_command_1. (2 locations)
(top-gdb) r

Starting program: /home/daniel/dev/gdb/build/gdb/gdb
[...]
(gdb) p 0

Thread 1 "gdb" hit Breakpoint 3, print_command_1 (During symbol reading: incomplete CFI data; unspecified registers (e.g., rax) at 0x55555589f362
args=0x555555eb9682 "0", voidprint=1) at ../../gdb/printcmd.c:1198
1198    {
(top-gdb) set print frame-info short-location
(top-gdb) where
#0  print_command_1 (args=..., voidprint=...)
#1  0x00005555556d4592 in cmd_func (cmd=..., args=..., from_tty=...)
#2  0x0000555555994aac in execute_command (p=..., from_tty=...)
#3  0x000055555578e65d in command_handler (command=...)
#4  0x000055555578e9dd in command_line_handler (rl=...)
#5  0x000055555578efde in gdb_rl_callback_handler (rl=...)
#6  0x0000555555a15d90 in rl_callback_read_char ()
#7  0x000055555578dc1e in gdb_rl_callback_read_char_wrapper_noexcept ()
#8  0x000055555578ee91 in gdb_rl_callback_read_char_wrapper (client_data=...)
#9  0x000055555578d950 in stdin_event_handler (error=..., client_data=...)
#10 0x0000555555ade2b6 in gdb_wait_for_event (block=...)
#11 0x0000555555ade55d in gdb_wait_for_event (block=...)
#12 gdb_do_one_event ()
#13 0x00005555558509f5 in start_event_loop ()
#14 captured_command_loop ()
#15 0x0000555555852c75 in captured_main (data=...)
#16 gdb_main (args=...)
#17 0x000055555563aa2c in main (argc=..., argv=...)
```

The neat part is that _something_ is detecting the gdb-gdb debug and
changing the top level gdb prompt to `top-gdb`. This is quite helpful
when debugging changes or tracing code paths.


[0]: http://lists.llvm.org/pipermail/lldb-dev/2018-January/013171.html
[1]: https://en.wikipedia.org/wiki/GNU_Debugger
[2]: https://github.com/rust-lang/llvm-project/pull/19
[3]: https://sourceware.org/pipermail/gdb-patches/2020-July/170140.html
