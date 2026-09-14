# Project test loading

The first external Jett application exposed two driver problems. Relative paths
such as `src/main.jett` can locate `jett.proj` through an empty parent path, then
fail to scan that empty path. Testing a source file also moves all its sibling
files before it, making a dependency appear after callers when its own verify
blocks are selected.

Project tests compile the same file order as the declared project entry:
stdlib first, project siblings in lexical source-path order, then the `jett.proj`
entry file. Selecting a file chooses which verify/property blocks execute;
it does not make that file the compilation entry. Declarations inside each
file remain in source order, and ordinary name resolution, ownership, types,
and verification remain authoritative. Project testing reports each block
once. The existing project parser owns the entry field; the test driver does
not introduce another manifest parser. Project discovery begins from an
absolute path so a relative input never produces an empty scan root.

Canonical paths identify the entry and selected file, but do not determine
declaration order. A supported in-root source-file symlink keeps its logical
source position even when its target is under an excluded directory such as
`target/`. Sorting its canonical target instead would make tests disagree with
the same project's build order.
