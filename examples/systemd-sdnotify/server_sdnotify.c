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
