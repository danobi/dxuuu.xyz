% systemd and sd_notify(3)

A little bit of background first: I've been spending a lot of time recently
mucking around with systemd and its process management features. The latest
project I was working on involved implementing hot upgrades on a running
server. This means that when used, clients of said server don't experience any
disruption other than a very small latency bubble. No connections are torn down
and no sockets are closed client-side. Fancy right?  I'll save the details for
how that was done in a later post.  For now, we discuss onto the systemd side
of the equation.

So implementing 0-downtime was fun and tricky, but the feature itself was
largely contained in C++ land, far away from systemd. The systemd interaction
with the server binary works a little something like this.

```
         Process 1                       Process 2
----------------------------|---------------------------------
systemctl start server

<serving clients>

                                   <want to upgrade server>
                                     ./server -upgrade

<hands over client conns>


                                   <receives client conns>
                                            (1)

<exits>
  (2)
```

Pretty straight forward right? Assuming that Process 1 is a systemd controlled
process (meaning systemd started the process), as point (2), systemd is going
to detect Process 1 exited, and will `systemctl restart` the process, negating
the hot upgrade we worked so hard to implement. How do we prevent this?

As it turns out, systemd has a little know (to me) service type called
`Type=notify`. This means that the process will link with systemd headers and
explicitly tell systemd about the service state and optionally, _gasp_, the
main PID of the running process, typically at point (1).

## Proof of concept (aka show me the code)

### server.c
``` {#function .cpp .numberLines startFrom="1"}
#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <unistd.h>

int main() {
    pid_t pid = fork();
    if (!pid) {
        printf("We're the new server process!\n");
        sleep(30);  // so we can examine systemd state
    } else if (pid == -1) {
        printf("Fork failed :(\n");
        perror("fork");
    } else {
        printf("We're the parent\n");
        exit(0);
    }
}
```
Build: `cc server.c -o server`

### example_server.service
```
[Unit]
Description=Example server process

[Service]
Type=simple
ExecStart=/home/daniel/dev/dxuuu.xyz/examples/systemd-sdnotify/server
Restart=always

[Install]
WantedBy=multi-user.target
```

Running this service will cause `example_server.service` to flap. You can
confirm that's happening by checking `systemctl status example_server`:

```
● example_server.service - Example server process
   Loaded: loaded (/etc/systemd/system/example_server.service; disabled; vendor preset: disabled)
      Active: failed (Result: start-limit-hit) since Mon 2017-11-13 07:28:29 PST; 4s ago
     Process: 4723 ExecStart=/home/daniel/dev/dxuuu.xyz/examples/systemd-sdnotify/server (code=exited, status=0/SUCCESS)
   Main PID: 4723 (code=exited, status=0/SUCCESS)

Nov 13 07:28:29 maharaja systemd[1]: example_server.service: Service hold-off time over, scheduling restart.
Nov 13 07:28:29 maharaja systemd[1]: example_server.service: Scheduled restart job, restart counter is at 5.
Nov 13 07:28:29 maharaja systemd[1]: Stopped Example server process.
Nov 13 07:28:29 maharaja systemd[1]: example_server.service: Start request repeated too quickly.
Nov 13 07:28:29 maharaja systemd[1]: example_server.service: Failed with result 'start-limit-hit'.
Nov 13 07:28:29 maharaja systemd[1]: Failed to start Example server process.
```

However, if we use `sd_notify(3)`, we get much better results.

### server_sdnotify.c
``` {#function .cpp .numberLines startFrom="1"}
#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <unistd.h>

#include <systemd/sd-daemon.h>

int main() {
    pid_t pid = fork();
    if (!pid) {
        printf("We're the new server process!\n");

        // tell systemd we're ready
        sd_notify(0, "READY=1\n");
        sleep(30);  // so we can examine systemd state
    } else if (pid == -1) {
        printf("Fork failed :(\n");
        perror("fork");
    } else {
        printf("We're the parent\n");

        // tell systemd the child is the main process now
        sd_notifyf(0, "MAINPID=%lu",
                      (unsigned long) pid);
        exit(0);
    }
}
```

Build: `cc server_sdnotify.c -o server_sdnotify -lsystemd`

### example_server_sdnotify.service
```
[Unit]
Description=Example server process with sd_notify

[Service]
Type=notify
ExecStart=/home/daniel/dev/dxuuu.xyz/examples/systemd-sdnotify/server_sdnotify
Restart=always
NotifyAccess=all

[Install]
WantedBy=multi-user.target
```

And now when you run `systemctl start server_sdnotify`, you'll see everything
works nicely:

```
● example_server_sdnotify.service - Example server process
   Loaded: loaded (/etc/systemd/system/example_server_sdnotify.service; disabled; vendor preset: disabled)
   Active: active (running) since Mon 2017-11-13 07:40:08 PST; 3s ago
 Main PID: 11445 (server_sdnotify)
    Tasks: 1 (limit: 4915)
   CGroup: /system.slice/example_server_sdnotify.service
           └─11445 /home/daniel/dev/dxuuu.xyz/examples/systemd-sdnotify/server_sdnotify

Nov 13 07:40:08 maharaja systemd[1]: Starting Example server process...
Nov 13 07:40:08 maharaja server_sdnotify[11444]: We're the parent
Nov 13 07:40:08 maharaja systemd[1]: example_server_sdnotify.service: Supervising process 11445 which is not our child. We'll most likely not notice when it exits.
Nov 13 07:40:08 maharaja systemd[1]: Started Example server process.
```

Note that the "Supervising process 11445 which is not our child" warning is a
bit bogus.  Since the parent dies after forking the child, the child now
belongs to PID 1. As such, systemd can listen to SIGCHLD. If you read through
the systemd source code, you can confirm this behavior is true.
