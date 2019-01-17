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
