% Ahead-Of-Time compiled bpftrace programs

This page serves as a design document for bpftrace AOT compilation support.
Design is currently a work-in-progress and will be (somewhat) regularly updated.

## Current architecture

![](../examples/aot-bpftrace/old-architecture.png){ width=100% }

## Proposed architecture

TBD

## Problems

* `CodegenLLVM` modifies runtime state in `BPFtrace`
  * IDs like printf_id_ must be saved into AOT executable
    * What other IDs are there?
  * Can save metadata into special ELF section; fortunately we don't need to
    worry about compatability as an AOT executable is hermetic
* Must ship a stubbed (no bytecode) AOT executable that knows to look inside
  itself for bytecode
  * Should be simple enough with cmake
* Bytecode must be saved into AOT executable
  * Special ELF section should do it

## Future goals

* User can select features to enable in codegen
  * eg. "tell codegen that the target host has XXX feature"
* Emitted bytecode takes advantage of CO-RE to be more compatible on other
  hosts
