# Type aliases used as constructor callees

The design permits transparent destination type aliases, namespace aliases,
and alias owners for method lookup. It does not select whether a transparent
type alias can also spell a constructor call.

The frontend currently accepts this bitfield example:

```jett
namespace app
bitfield Header:
    value: 8 bits
type Alias = Header
function main(stdout: Stdout) returns nothing:
    Alias header = Alias(value: 7)
    Stdout.write(view stdout, "{header.value}\n")
```

Reference execution reports an undefined function for the alias callee; native
lowering has no checked constructor argument order for it. Construction through
`Header(...)` followed by an `Alias` destination is a separate, supported case.
Reflected `TypeConstruction` metadata also does not decide this call spelling.

Two policies need comparison before changing either backend:

- Permit an alias callee and resolve it to the exact underlying declaration.
  Argument order, generic arguments, validation/result shape, nominal identity,
  and source provenance must come from that selected declaration.
- Require the declaration's constructor spelling and reject an alias callee
  during checking. Destination aliases and alias method lookup remain valid.

Nominal refinements require their own boundary and must not be treated as
transparent aliases merely because they share a runtime layout. An accepted
transparent-alias policy would also need an explicit scope for struct,
bitfield, enum and machine constructors, including generic and qualified forms.

Until the call policy is selected, do not infer constructor permission from
destination-type compatibility or add backend name rewriting for arbitrary
aliases. This is an unresolved source contract beyond the fixed native fixture
inventory, not evidence of complete native parity.
