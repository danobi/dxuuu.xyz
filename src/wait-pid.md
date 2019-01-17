% Waiting on process exit

How does a process wait for another process to exit? Simple question, right?

I've been recently working on bpftrace and I had to find an answer for this
problem. Ideally, the solution would not do any polling. First, let's try the
most naive solution: `waitpid(2)`:

### waitpid(2)

``` {#function .cpp .numberLines startFrom="1"}
#include <iostream>
#include <cstdlib>
#include <thread>

#include <sys/epoll.h>
#include <sys/eventfd.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <stdio.h>
#include <unistd.h>

#define MAX_EVENTS 10

void waiter(int pid, int efd) {
  int wstatus;
  if (waitpid(pid, &wstatus, 0) < 0) {
    perror("waitpid");
    return;
  }

  int one = 1;
  if (write(efd, &one, sizeof(one)) < 0) {
    perror("write");
    return;
  }

  return;
}

int main(int argc, const char** argv) {
  int pid;

  if (argc < 2) {
    std::cerr << "usage: ./waiter <pid>" << std::endl;
    return 1;
  }

  pid = std::atoi(argv[1]);

  // create eventfd in semaphore mode
  int efd = eventfd(0, EFD_CLOEXEC | EFD_SEMAPHORE);
  if (efd < 0) {
    perror("eventfd");
    return 1;
  }

  // set up epoll
  struct epoll_event ev, events[MAX_EVENTS];
  int epollfd = epoll_create1(EPOLL_CLOEXEC);
  if (epollfd < 0) {
    perror("epoll_create1");
    return 1;
  }
  ev.events = EPOLLIN;
  ev.data.fd = efd;
  if (epoll_ctl(epollfd, EPOLL_CTL_ADD, efd, &ev) < 0) {
    perror("epoll_ctl");
    return 1;
  }

  auto t = std::thread([&]() {
    waiter(pid, efd);
  });

  while (1) {
    int nfds = epoll_wait(epollfd, events, MAX_EVENTS, -1);
    if (nfds == -1) {
      perror("epoll_wait");
      return 1;
    }

    for (int i = 0; i < nfds; ++i) {
      if (events[i].data.fd == efd) {
        std::cerr << pid << " has exited" << std::endl;
        break;
      }
    }
  }

  t.join();
  close(efd);
  close(epollfd);
}
```

In short, our first program creates a thread, passes an eventfd handle to it,
and writes to the eventfd handle once the thread exits `waitpid(2)`. What
happens if we run it?

In one window:
```
$ python3
>>> import os
>>> os.getpid()
1573
```

In another:
```
$ g++ waitpid-waitpid.cpp -lpthread
$ ./a.out 1573
waitpid: No child processes
^C

```

Unfortunately, `waitpid(2)` only works on child processes.

Let's try a different strategy.

### epoll(2) on /proc/pid

``` {#function .cpp .numberLines startFrom="1"}
#include <iostream>
#include <cstdlib>

#include <fcntl.h>
#include <sys/epoll.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <stdio.h>
#include <unistd.h>

#define MAX_EVENTS 10

int main(int argc, const char** argv) {
  int pid;
  char buf[1024];

  if (argc < 2) {
    std::cerr << "usage: ./waiter <pid>" << std::endl;
    return 1;
  }

  pid = std::atoi(argv[1]);

  if (snprintf(&buf[0], sizeof(buf), "/proc/%d/status", pid) < 0) {
    std::cerr << "snprintf failed" << std::endl;
    return 1;
  }

  std::cerr << "trying to open=" << buf << std::endl;

  int pidfd = open(&buf[0], 0);
  if (pidfd < 0) {
    perror("open");
    return 1;
  }

  // set up epoll
  struct epoll_event ev, events[MAX_EVENTS];
  int epollfd = epoll_create1(EPOLL_CLOEXEC);
  if (epollfd < 0) {
    perror("epoll_create1");
    return 1;
  }

  ev.events = EPOLLERR | EPOLLHUP;  // wait for procfs entry to disappear
  ev.data.fd = pidfd;
  if (epoll_ctl(epollfd, EPOLL_CTL_ADD, pidfd, &ev) < 0) {
    perror("epoll_ctl");
    return 1;
  }

  while (1) {
    int nfds = epoll_wait(epollfd, events, MAX_EVENTS, -1);
    if (nfds == -1) {
      perror("epoll_wait");
      return 1;
    }

    bool exited = false;
    for (int i = 0; i < nfds; ++i) {
      if (events[i].data.fd == pidfd) {
        std::cerr << pid << " has exited" << std::endl;
        exited = true;
      }
    }

    if (exited) {
      break;
    }
  }

  close(pidfd);
  close(epollfd);
}
```

Hey, a clever idea! Let's poll on `/proc/pid/status` and wait for an EPOLLHUP
event. Let's see if it works.

```
$ ./a.out 1573
trying to open=/proc/1573/status
epoll_ctl: Operation not permitted

```

It turns out that epoll does not support pseudo-fs kernel interfaces.

An interesting side note, according to `proc(5)`:

```
/proc/[pid]/mounts (since Linux 2.4.19)
...
Since kernel version 2.6.15, this file is pollable: after
opening the file for reading, a change in this file (i.e., a
filesystem mount or unmount) causes select(2) to mark the file
descriptor as having an exceptional condition, and poll(2) and
epoll_wait(2) mark the file as having a priority event (POLL‐
PRI)
```

Let's make this change

```
--- waitpid_epollhup.cpp        2019-01-16 20:16:29.078024749 -0800
+++ waitpid_epollhup2.cpp       2019-01-16 20:17:25.766842080 -0800
@@ -21,7 +21,7 @@

   pid = std::atoi(argv[1]);

-  if (snprintf(&buf[0], sizeof(buf), "/proc/%d/status", pid) < 0) {
+  if (snprintf(&buf[0], sizeof(buf), "/proc/%d/mounts", pid) < 0) {
     std::cerr << "snprintf failed" << std::endl;
     return 1;
   }
@@ -42,7 +42,7 @@
     return 1;
   }

-  ev.events = EPOLLERR | EPOLLHUP;  // wait for procfs entry to disappear
+  ev.events = EPOLLERR | EPOLLHUP | EPOLLPRI;  // wait for procfs entry to disappear
   ev.data.fd = pidfd;
   if (epoll_ctl(epollfd, EPOLL_CTL_ADD, pidfd, &ev) < 0) {
     perror("epoll_ctl");
```

and see if it works. Run

```
$ ./a.out 9001
trying to open=/proc/9001/mounts

```

and then kill the python process. Unfortunately (hard to show in text), it does
not work. `a.out` hangs.

### Final attempt: polling procfs

Even though I said I didn't want to poll, we might still be able to get away
with polling if we do it infrequently enough. Consider:

``` {#function .cpp .numberLines startFrom="1"}
#include <iostream>
#include <stdexcept>

#include <fcntl.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <stdio.h>

bool is_pid_alive(int pid) {
  char buf[1024];
  int ret = snprintf(&buf[0], sizeof(buf), "/proc/%d/status", pid);
  if (ret < 0) {
    throw std::runtime_error("failed to snprintf");
  }

  int fd = open(&buf[0], 0);
  if (fd < 0 && errno == ENOENT) {
    return false;
  }
  close(fd);

  return true;
}

int main(int argc, const char** argv) {
  if (argc < 2) {
    std::cerr << "usage: ./poll <pid>" << std::endl;
    return 1;
  }

  int pid = std::atoi(argv[1]);

  while (1) {
    if (!is_pid_alive(pid)) {
      std::cerr << pid << " has died" << std::endl;
      break;
    }

    sleep(1);
  }
}
```

When run with the same python setup:
```
$ ./a.out 11643
11643 has died

```

I guess we'll have to live with this.

### Unattempted solutions

There were a few ideas I knew about but didn't try for various reasons:

* inotify
  * This doesn't work for the same reason as `epoll`ing on procfs didn't:
    inotify requires changes through a userspace filesystem.

* netlink
  * There does exist a netlink interface for task stats. However, creating a
    netlink socket and monitoring it takes quite a bit of boilerplate. For
    bpftrace's use case, it was far cleaner and less bug prone to simply poll.

* bpf
  * We could have really nested the turtles here and created another bpf
    program to watch for process exit. There's no real reason I didn't take
    this route other than it was going to take a lot of code.

* ptrace with PTRACE_SEIZE
  * It wasn't clear to me this would be overhead-free on the target process.
    Perhaps in the future I can run some tests.
