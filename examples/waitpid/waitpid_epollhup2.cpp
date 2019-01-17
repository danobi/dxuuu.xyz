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

  if (snprintf(&buf[0], sizeof(buf), "/proc/%d/mounts", pid) < 0) {
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

  ev.events = EPOLLERR | EPOLLHUP | EPOLLPRI;  // wait for procfs entry to disappear
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
