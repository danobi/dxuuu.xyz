% Sudo and signal propagation

This week I spent some time debugging a curious case where signals were not
being delivered to a `sudo`'ed process.

Consider this shell script:

```
#!/bin/bash

sudo timeout 5 dd if=/dev/urandom of=/dev/null &
sudo_pid=$!
sudo kill $sudo_pid

if ps -p $sudo_pid &>/dev/null; then
  echo Still running
else
  echo Dead
fi
```

What should be printed? If you guessed "Dead", then you would be correct:

```
$ ./sudo_kill.sh
Dead
```

However, when run in CI, I was seeing the opposite. The following is run
from a recreated CI environment (through docker):

```
docker@cd71223430f4$ ./sudo_kill.sh
Still running
```

## Signal propagation

Before going deeper, we need a little background on how `sudo` handles signals.

According to the man page:

> When the command is run as a child of the sudo process, sudo will relay
> signals it receives to the command.  The SIGINT and SIGQUIT signals are only
> relayed when the command is being run in a new pty or when the signal was
> sent by a user process, not the kernel.  This prevents the command from
> receiving SIGINT twice each time the user enters control-C.  Some signals,
> such as SIGSTOP and SIGKILL, cannot be caught and thus will not be relayed to
> the command.

This makes sense. Signals need to be relayed by `sudo` to the process it's
managing. Otherwise, scripts would need to calculate the child PID or send
signals to the appropriate process group (a rather arcane concept). It is
much simpler to run a backgrounded `sudo` instance and send signals to `$!` (as
we did in the test script).

However, there are some drawbacks to this approach. The documentation goes on
to say:

> As a special case, sudo will not relay signals that were sent by the command
> it is running. This prevents the command from accidentally killing itself.
> On some systems, the reboot(8) utility sends SIGTERM to all non-system
> processes other than itself before rebooting the system.  This prevents sudo
> from relaying the SIGTERM signal it received back to reboot(8), which might
> then exit before the system was actually rebooted, leaving it in a half-dead
> state similar to single user mode.

While this note is not directly related to our curious issue (b/c we are not
getting `dd` to send signals), it is important to note there are special cases
carved out. This is relevant later.

## Changes in sudo

Keeping in mind there are various corner cases and carve-outs for `sudo` signal
propagation behavior, the obvious suspect is a change in `sudo`. From comparing
the local version to the CI version (`1.9.13p2` vs `1.8.21p2`),
we discover [this commit][0] which landed in `1.9.13p1`.

The old behavior (before the change) was to:

> Only forward user-generated signals not sent by a process in the command's
> own process group. [...]

In my opinion this is surprising and unexpected, if only because this behavior
is undocumented. That being said, I did manage to find a zero point [stack
overflow][3] answer for a `setsid` workaround. But I think that just proves my
point.

Fortunately, as of November 2022, the new behavior is to:

> [...] forward signals from a process in the same pgrp if the pgrp leader is not
> either sudo or the command itself. [...]

which fixes the script use case and obviates the need for a `setsid` workaround.

## Complexity

I had remarked a few months ago I was surprised [the sudo project][1] has over
12,000 commits. Now I am no longer surprised. Behind a rather simple interface
(from the common use case perspective) lies a mountain of complexity. I'm
actually quite shocked it took [over 40 years][2] for this surprising and
undocumented behavior to be fixed.



[0]: https://www.sudo.ws/repos/sudo/rev/d1bf60eac57f
[1]: https://github.com/sudo-project/sudo
[2]: https://www.sudo.ws/about/history/
[3]: https://stackoverflow.com/a/68879134
