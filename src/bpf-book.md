% The case for a community maintained BPF book

It's widely accepted that it's hard to get started with eBPF (BPF). It's an
extremely fast moving ecosystem and the state-of-the-art today is not the same
as it was the last year. For new and perhaps more casual developers who cannot
afford to follow along closely, this is a problem. How do we expect them to
become productive while also avoiding potential stumbling blocks?

You have to understand C. You have to understand the kernel. You have to
understand the verifier and what it tries to tell you. There is a lot to
understand and no one good place to learn it all. Reading the kernel selftests
is not a good answer for someone who is not involved with kernel development
(consider how you'd explain making and running a change). Sure, there are some
BPF books these days. But they're out of date as soon as the ink is dry.

There is, however, precedence for solving this problem. Take a look at the Rust
[ecosystem][0] of [community][1] maintained [books][2]. They're all version
controlled and contributions are made through pull requests. A large, gaping
hole exists in the BPF ecosystem for such a community maintained book.  Having
an open book allows us all to share best practices as well as learned
experience and keep it up to date over time.  It'd be in a digestible format --
something someone new can read cover to cover and feel up to speed.

Yeah, it's a lot of work. But we've done much harder things before.

[0]: https://doc.rust-lang.org/book/
[1]: https://doc.rust-lang.org/cargo/
[2]: https://doc.rust-lang.org/rustc/what-is-rustc.html
