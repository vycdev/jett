# User structs inside enum equality

Direct comparison of user-defined structs requires an explicit, exact
`Equatable.equals` implementation. The typechecker currently permits an enum
equality expression whose payload contains a user struct, and the interpreter's
recursive `Value` equality compares that struct's fields. This bypasses the
direct comparison rule. Native aggregate enum equality therefore rejects such
payloads rather than introducing another structural equality path.

The language rule needs to decide whether enum equality recursively requires
`Equatable` for every nested struct, invokes its exact method for each struct
value, or rejects these enum comparisons. The frontend and interpreter should
enforce the chosen rule before native codegen adds support. A differential
fixture should cover equal and unequal nested structs, a custom equality
method whose result differs from field equality, and nested collection paths.
