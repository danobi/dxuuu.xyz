% Generating kernel patchset

### Background

The linux kernel famously has an [email based workflow][0]. Ignoring whether
it's a good thing or not, it still remains a fact that it can be tricky to
generate the correct emails to send.

One problem I've been dealing with over the years is wrangling
`git-format-patch` and `git-send-email` to do the right thing. Any time I've
wanted to send out a patchset in the past, I've had to search through my shell
history to find the last invocation. If it was a respun patchset, I'd increment
the `-v N` flag. If it was a new patchset, I'd use the old command as
inspiration and manually change all the `--to` and `--cc` flags.

Obviously this gets old pretty fast and it becomes a source of friction when
sending out small fixes (which I rarely did).

### Problem

Automation is clearly the answer. A big wrapper script over `git` is always
on the table, but those can become overcomplicated over time. I'd rather
integrate with `git` as best I can so it's easier to maintain the automation.

The above is not a new idea. There's been previous writing on this:

* https://mudongliang.github.io/2021/06/21/git-send-email-with-cc-cmd-and-to-cmd.html
* https://www.marcusfolkesson.se/blog/get_maintainers-and-git-send-email/

### Solution

XXX: talk about generating emails at format-patch time instead of send-email time


[0]: https://www.kernel.org/doc/html/latest/process/submitting-patches.html
