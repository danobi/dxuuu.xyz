% Ahead-Of-Time compiled bpftrace programs

This page serves as a design document for bpftrace AOT compilation support.
Design is currently a work-in-progress and will be (somewhat) regularly updated.

## Current architecture

![](../examples/aot-bpftrace/old-architecture.png){ width=100% }

## Proposed architecture

TBD

## Overall design

* Ship a fully executable runtime shim with bpftrace
* When compiling a AOT bpftrace program:
  * Build the metadata
  * Build the bytecode
  * Make a copy of runtime shim and store metadata + bytecode into a special
    ELF section (this is the final executable)
* When the shim runs, it knows to look inside itself for the metadata + bytecode
  and start execution

## Unsolved problems

* `CodegenLLVM` modifies runtime state in `BPFtrace`
  * Shared IDs must be saved into AOT executable, but how to keep in sync?
    * `printf_id_`
    * `cat_id_`
    * `system_id_`
    * `time_id_`
    * `strftime_id_`
    * `join_id_`
    * `helper_error_id_`
    * `non_map_print_id_`
    * Any others?
* `CodegenLLVM` relies on runtime state in `BPFtrace`
  * Codegen for `elapsed` embeds map FD
  * Positional parameters are hardcoded into bytecode
  * Any other?

## Notes

* Can save metadata into special ELF section; fortunately we don't need to
  worry about compatability as an AOT executable is hermetic
* Must ship a stubbed (no bytecode) AOT executable that knows to look inside
  itself for bytecode
  * Should be simple enough with cmake
* Can create a ConstantData abstraction that holds data that is only known
  at runtime but progs need to access too (`elapsed` builtin, positional params)
  and is backed by multiple maps (for different data types)
* Will need to relocate pseudo-map-FDs at runtime to FDs of created maps
  (see BPF_PSEUDO_MAP_FD in libbpf)

## Future goals

* User can select features to enable in codegen
  * eg. "tell codegen that the target host has XXX feature"
* Emitted bytecode takes advantage of CO-RE to be more compatible on other
  hosts
