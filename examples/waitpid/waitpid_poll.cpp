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
