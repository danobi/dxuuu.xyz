% Generating kernel patchsets

### Background

The linux kernel famously has an [email based workflow][0]. Ignoring whether
it's a good thing or not, it still remains a fact that it can be tricky to
generate the correct emails to send.

One problem I've been dealing with over the years is wrangling
`git-format-patch` and `git-send-email` to do the right thing. Any time I've
wanted to send out a patchset, I've had to search through my shell history to
find the last invocation. If it was a respun patchset, I'd increment the `-v N`
flag. If it was a new patchset, I'd use the old command as inspiration and
manually change all the `--to` and `--cc` flags.

Obviously this gets old pretty fast and it becomes a source of friction when
sending out small fixes (which I rarely did).

### Problem

Automation is clearly the answer. A big wrapper script over `git` is always
on the table, but those can become overcomplicated over time. I'd rather
integrate with `git` as best I can so it's easier to maintain the automation.

The above is [not a new idea][1]; there's been plenty of [previous writing][2]
on this. Basically what the two linked approaches suggest is to hook into
git-send-email to automatically generate `To:` and `Cc:` headers using
`scripts/get_maintainer.pl`.

However, there are multiple drawbacks of the above approaches:

1. They do not correctly generate To: and Cc: headers for cover letters
1. It is painful to realize the generation is incorrect halfway through sending
   a large patchset. Half the emails are already sent -- you cannot take them
   back.


After some experimenting, I offer a slightly improved solution.

### Solution

The improvement I'm offering is to generate the To: and Cc: headers during
git-format-patch time. This allows you to eyeball the headers while doing a
final check over the formatted patches. This also gives you a way to add more
emails incrementally.

To that end, I ended up using [Git aliases][3] to "create" my own git command:
`git patchset`. The alias is defined as follows:

```
[alias]
        patchset = "!linux-patchset.sh"
```

In other words, all it does is run a script. Note that all arguments after `git
patchset` are forwarded to the script as positional arguments.

`linux-patchset.sh` is more or less the following:

```
ROOT=$(git rev-parse --show-toplevel)
OUTDIR="${ROOT}/outgoing/$(git rev-parse --abbrev-ref HEAD)"

git format-patch -o "$OUTDIR" -s --to "" --cc "" "$@"
LINUX_ROOT="$ROOT" linux-address-patchset.py "$OUTDIR"
```

Basically what this script does is first git-format-patch the patchset
and place the output into `linux/outgoing`. The output location is not
that important -- I just prefer it there.

After the patches are formatted, it runs `linux-address-patchset.py`
over the formatted patches. The logic in linux-address-patchset.py is
fairly mechanical and might be subject to change over time. But the
basic idea is for each patch file:

* If the patch file is a cover letter, iterate through the rest of the series
  and collect all the _mailing list_ addresses for the To: and Cc: headers
* For non-cover letter patches, collect maintainer addresses for the To: header
* Similarly for Cc:, collect reviewer and mailing list addresses
* Once all the addresses are collected and deduped, append addresses to the
  appropriate header line.

The full scripts are available [here][4].

### Conclusion

All together, this how my latest patchset was generated:

```
$ git patchset -v2 --subject-prefix "PATCH bpf-next" bpf-next/master
~/linux/outgoing/ip_check_defrag-v2/v2-0000-cover-letter.patch
~/linux/outgoing/ip_check_defrag-v2/v2-0001-ip-frags-Return-actual-error-codes-from-ip_check_.patch
~/linux/outgoing/ip_check_defrag-v2/v2-0002-bpf-verifier-Support-KF_CHANGES_PKT-flag.patch
~/linux/outgoing/ip_check_defrag-v2/v2-0003-bpf-net-frags-Add-bpf_ip_check_defrag-kfunc.patch
~/linux/outgoing/ip_check_defrag-v2/v2-0004-net-ipv6-Factor-ipv6_frag_rcv-to-take-netns-and-u.patch
~/linux/outgoing/ip_check_defrag-v2/v2-0005-bpf-net-ipv6-Add-bpf_ipv6_frag_rcv-kfunc.patch
~/linux/outgoing/ip_check_defrag-v2/v2-0006-bpf-selftests-Support-not-connecting-client-socke.patch
~/linux/outgoing/ip_check_defrag-v2/v2-0007-bpf-selftests-Support-custom-type-and-proto-for-c.patch
~/linux/outgoing/ip_check_defrag-v2/v2-0008-bpf-selftests-Add-defrag-selftests.patch
```

And if you take a peek at the first two patches:

```
$ head -n7 outgoing/ip_check_defrag-v2/v2-0000-cover-letter.patch
From 99ddd1e2b35f6133c1f49a0245340e1a8aaaf32f Mon Sep 17 00:00:00 2001
Message-Id: <cover.1677526810.git.dxu@dxuuu.xyz>
From: Daniel Xu <dxu@dxuuu.xyz>
Date: Mon, 27 Feb 2023 12:40:10 -0700
Subject: [PATCH bpf-next v2 0/8] Support defragmenting IPv(4|6) packets in BPF
To: bpf@vger.kernel.org,linux-kselftest@vger.kernel.org,netdev@vger.kernel.org,linux-doc@vger.kernel.org,linux-kernel@vger.kernel.org
Cc: bpf@vger.kernel.org

$ head -n11 outgoing/ip_check_defrag-v2/v2-0001-ip-frags-Return-actual-error-codes-from-ip_check_.patch
From bf4afe3484836972f94c1b7738845ba69d7008f5 Mon Sep 17 00:00:00 2001
Message-Id: <bf4afe3484836972f94c1b7738845ba69d7008f5.1677526810.git.dxu@dxuuu.xyz>
In-Reply-To: <cover.1677526810.git.dxu@dxuuu.xyz>
References: <cover.1677526810.git.dxu@dxuuu.xyz>
From: Daniel Xu <dxu@dxuuu.xyz>
Date: Tue, 6 Dec 2022 17:47:16 -0700
Subject: [PATCH bpf-next v2 1/8] ip: frags: Return actual error codes from
 ip_check_defrag()
To: kuba@kernel.org,edumazet@google.com,willemdebruijn.kernel@gmail.com,davem@davemloft.net,pabeni@redhat.com,dsahern@kernel.org
Cc: netdev@vger.kernel.org,linux-kernel@vger.kernel.org,bpf@vger.kernel.org
```

Happy hacking!

[0]: https://www.kernel.org/doc/html/latest/process/submitting-patches.html
[1]: https://mudongliang.github.io/2021/06/21/git-send-email-with-cc-cmd-and-to-cmd.html
[2]: https://www.marcusfolkesson.se/blog/get_maintainers-and-git-send-email/
[3]: https://git-scm.com/book/en/v2/Git-Basics-Git-Aliases
[4]: https://github.com/danobi/bin
