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
