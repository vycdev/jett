# Jett Language Design Document

## Vision

Jett is a programming language designed from the ground up to be **optimized for LLM consumption and generation**. While most languages were designed for humans typing on keyboards, Jett recognizes that a growing share of code is written by large language models. Every design decision prioritizes token efficiency, predictability, and minimizing the patterns that cause LLMs to make mistakes.

Jett is not an "AI framework" or a language about AI — it is a general-purpose language whose syntax, semantics, and conventions are shaped by how LLMs tokenize, predict, and reason about code.

## Language Paradigm

Jett is a **statically-typed imperative language with enforced purity boundaries**. It is not a functional language, and it is not an object-oriented language.

You write straightforward procedural code — loops, mutable variables, sequential steps — but the type system and capability system enforce the safety guarantees that purely functional languages achieve through purity. Pure functions are guaranteed pure by the compiler. Side effects are tracked explicitly through capability parameters, not hidden behind monads or implicit state.

**What Jett borrows from each tradition:**

- **From imperative/procedural:** `for`/`while` loops, mutable `let` bindings, sequential control flow, straightforward step-by-step code.
- **From functional:** pure functions by default, pipeline operator, immutable data encouraged, composition over inheritance, no shared mutable global state.
- **From structural typing (Go/Rust style):** structs + interfaces, no classes, no inheritance, explicit interface implementation.
- **What Jett avoids:** no monads, no higher-kinded types, no class hierarchies, no method overriding, no implicit side effects.

The closest comparison in feel is **Go or Rust** — you write normal imperative code, but the compiler enforces strong guarantees about purity, side effects, and type safety. Jett is pragmatic, not academic.

## Designed for Agent Tooling

Jett is optimized not just for LLM code generation, but also for how coding agents **navigate, understand, and modify** existing codebases. Modern agents use standard tools — search, grep, file reading, text replacement, CLI commands — and Jett's design makes every one of these operations more reliable.

**Search and grep are deterministic.** One canonical form means searching for a pattern always finds it. An agent grepping for a function call will never miss it because someone wrote it differently — there is only one way to write it.

**Reading a function gives complete context.** Self-contained functions with inline imports mean an agent can read a single function and understand all its dependencies without searching for file-level imports or tracing class hierarchies.

**Text replacement is safe.** No function overloading and unique `namespace.function_name` identifiers mean renaming is a global text replacement across all files. There is no ambiguity about which function is being renamed.

**Diffs are clean.** One canonical formatting style (enforced by `jett format`) means diffs only contain logical changes, never formatting noise. When an agent changes one line of logic, the diff shows one line — not 50 lines of reformatting.

**The compile-fix loop is structured.** The Agent Server Protocol (Rule Set 21) outputs structured JSON errors that agents can parse and act on mechanically. The agent runs `jett build --agent`, reads the JSON error, fixes the issue, and repeats. Each error includes the file, line, column, expected type, got type, and a suggested fix.

**Discovering available code is a flat query.** The ASP provides `jett query --agent --namespaces` — a flat list of all available namespaces, functions, and types. An agent does not need to traverse directory trees or class hierarchies to find what it can call.

**New code can be added safely.** Jett's strict top-to-bottom ordering means an agent appending a new function at the end of a file cannot break existing code above it. Adding functionality is always additive.

**Builds are deterministic.** Content-addressed dependencies with SHA-256 hashes mean builds are reproducible across environments. An agent will never encounter "works on my machine" issues.

*Note: The `--agent` flag and the Agent Server Protocol (ASP) referenced above are defined in Rule Set 21. The ASP specifies how the compiler communicates structured JSON output to agents — including build errors, type queries, signature lookups, completions, and test results. The exact capabilities and query formats are still being refined and may evolve as the compiler is implemented. See Rule Set 21 for the current specification.*

---

## Foundational Rules

### Rule Set 1: Syntax and Tokenization Optimizations

#### 1. Strictly One Canonical Form

There must be exactly **one way** to express any given logic. No shortcuts, no aliases, no alternate syntax. Syntactic sugar exists in other languages to make humans type less, but for LLMs it creates ambiguity — the model wastes probability mass deciding *which* form to use and may inconsistently mix forms within a single file.

**What this means in practice:**

- No ternary operator alongside `if`/`else`. There is only `if`/`else`.
- No shorthand lambdas alongside named functions. There is only `function`.
- No implicit returns. Always use `return`.
- No optional parentheses. If a construct uses parentheses, it always uses them.
- No operator aliases (`&&` vs `and`, `||` vs `or`). Pick one. Jett uses `and`, `or`, `not`, `is`, `!=`.
- No multiple import styles. One `use` syntax, always.
- No string concatenation with `+` alongside interpolation. Pick one mechanism.

**Rationale:** When an LLM has seen training data with 5 ways to write a lambda in JavaScript, it may produce any of them unpredictably. When there is exactly one way, generation is deterministic and consistent.

#### 2. Tokenizer-Friendly Keywords

Jett uses **common English words** as keywords, not symbols or abbreviations. Every keyword should ideally map to a **single token** in major LLM tokenizers (GPT, Claude, LLaMA, etc.).

**Keyword design rules:**

- Use `function` not `fn`, `func`, `def`, or `λ`.
- Use `if`, `else`, `for`, `while`, `return` — universally recognized words.
- Use `and`, `or`, `not` instead of `&&`, `||`, `!`.
- Use `is` instead of `===` or `==` for equality. Use `!=` for inequality (a symbol exception, like the comparison operators).
- Use `let` for variable binding — short, common, single-token.
- Avoid abbreviations that may tokenize into subwords (e.g. `fmt` might become `f` + `mt`).

**Why obscure symbols are harmful:**

Language-specific symbols like `$`, `<=>`, `:=`, `>>=` are problematic because:
1. Tokenizers often split them into multiple tokens (e.g. `>>=` becomes `>>` + `=` or `>` + `>=`), wasting tokens.
2. LLMs may confuse similar-looking symbol sequences (e.g. `->` vs `=>` vs `<-`).
3. Obscure symbols carry no inherent semantic meaning — a model must memorize what `<>` means in each language, whereas `not equal` is self-documenting.

**Universal symbol exceptions:** Arithmetic (`+`, `-`, `*`, `/`), comparison (`>`, `<`, `>=`, `<=`), and inequality (`!=`) operators are universal across virtually all programming languages. Every LLM has seen them millions of times and every tokenizer handles them as 1-2 tokens. These are kept as symbols. All other operators use English keywords (`and`, `or`, `not`, `is`, `modulo`).

**Why `!=` is a symbol but `is` is a keyword:** `is not` was Jett's only two-word operator, creating parsing ambiguity (is `is` followed by `not x` or `is not` as a unit?) and breaking the rule that every keyword is a single token. `!=` eliminates this problem — it is universal across languages, every tokenizer handles it cleanly, and it is consistent with the existing symbol comparison operators. `is` stays as a keyword because it is a clean single token with no ambiguity. This pragmatic mix (keyword equality + symbol inequality) mirrors Python, the most common language in LLM training data.

#### 3. AST-Native Syntax

This is Jett's most distinctive design choice. **LLMs are fundamentally good at generating structured, nested data** — they produce JSON, XML, and nested property structures with high reliability. Jett's syntax should lean into this strength.

The language syntax is designed so that source code maps **directly and transparently** to its abstract syntax tree. There is minimal gap between what you write and the tree structure the compiler sees. This means:

**Structural properties:**

- Code reads as a **tree of labeled nodes**, not as a stream of tokens with implicit precedence rules.
- Every construct is explicitly delimited and named — the structure is always visible, never implicit.
- Nesting is semantic (reflects actual tree depth), not accidental (from operator precedence or bracket matching).
- The syntax can be losslessly round-tripped to/from a JSON AST representation.

**What AST-native looks like:**

Jett uses standard arithmetic operators (`+`, `-`, `*`, `/`) and comparison operators (`>`, `<`, `>=`, `<=`) with conventional precedence rules, plus the keyword operator `modulo` for remainder (`a modulo b`). These symbols are universal exceptions — every language uses them, every LLM tokenizer handles them as single tokens, and function-call alternatives like `add(a, multiply(b, c))` or keyword alternatives like `a greater_or_equal b` are significantly harder to read without saving tokens:

```
# Arithmetic uses standard operators:
let x = a + b * c - d / e
```

The operator precedence is standard (multiplication and division bind tighter than addition and subtraction), matching every other language the LLM has been trained on. Parentheses are used for explicit grouping when needed:

```
let x = (a + b) * (c - d) / e
```

**JSON AST equivalence:**

Any Jett program can be represented as a JSON AST, and that JSON can be converted back to Jett source without loss. This is powerful for LLMs because:

1. An LLM can generate the JSON AST directly if that is easier for a given task.
2. Tools can transform between Jett source and JSON AST freely.
3. The LLM never has to "guess" the tree structure — it is always explicit.

Example — a function in Jett source and its JSON AST:

```
function max(a: int, b: int) returns int:
    if a > b:
        return a
    return b
```

```json
{
    "type": "function",
    "name": "max",
    "params": [
        {"name": "a", "type": "int"},
        {"name": "b", "type": "int"}
    ],
    "returns": "int",
    "body": [
        {
            "type": "if",
            "condition": {"type": "compare", "op": ">", "left": {"type": "ref", "name": "a"}, "right": {"type": "ref", "name": "b"}},
            "then": [{"type": "return", "value": {"type": "ref", "name": "a"}}]
        },
        {
            "type": "return",
            "value": {"type": "ref", "name": "b"}
        }
    ]
}
```

The mapping between these two forms should be trivial and mechanical. If it is not, the syntax is not AST-native enough.

### Rule Set 2: Context Isolation and Flat Architecture

The core problem: LLMs work within finite context windows. If understanding a single function requires loading an entire codebase — inheritance trees, global state, implicit side effects — the LLM will either run out of context or make mistakes from incomplete information. Jett solves this by making every piece of code **self-describing and locally understandable**.

#### 1. Zero Implicit State

There must be **no spooky action at a distance**. A variable must never be silently changed by code in another file, another module, or a parent class. Every dependency, every state change, and every side effect must be **explicitly declared locally** — right where it happens.

**What this means in practice:**

- No global mutable variables. Global constants are allowed (they never change), but mutable global state is forbidden.
- No module-level side effects on import. `use math` loads definitions — it does not execute code, register handlers, or modify state.
- Side effects must be declared in the function signature via **capability parameters** (see Rule Set 16). If a function writes to a file, it receives a `Filesystem` capability. If it accesses the network, it receives a `Network` capability. The signature is the contract.
- All inputs to a function come through its parameters. No reading from ambient scope, no closures over mutable state, no thread-local storage. Anonymous functions can capture **immutable** values from the enclosing scope. Captured values are implicitly viewed — they are not consumed by the closure. Closures over **mutable** state are banned. This allows patterns like `list.find(users, function(u: User) returns bool: return u.id is target_id)` where `target_id` is an immutable value from the outer scope.

**Example — side effects are declared, not hidden:**

```
function save_user(fs: Filesystem, user: view User) returns result[nothing, string]:
    let data = json.serialize(user)
    Filesystem.write_file(fs, "users.json", data) handle error:
        return fail("could not save user")
    return ok(nothing)

function compute_tax(income: float, rate: float) returns float:
    return income * rate
```

`save_user` declares that it performs filesystem effects by taking a `Filesystem` capability parameter — the compiler auto-rebinds capability parameters, so no `with` clause is needed. Any caller can see this without reading the function body. `compute_tax` has no capability parameters, so the compiler guarantees it is pure. An LLM reading only the signatures knows exactly what each function does and does not do.

**Why this matters for LLMs:**

When an LLM sees a function, it can reason about it completely from its signature and body. It never needs to ask "but what if some other module mutated this variable before this function ran?" That question simply cannot arise.

#### 2. Pure Functions by Default

Functions in Jett are **pure by default**. They take explicit inputs through their parameters, return explicit outputs through their return type, and do not mutate anything outside their own scope.

**Rules:**

- A function without capability parameters is guaranteed pure by the compiler.
- Pure functions cannot call impure functions — the capability system propagates. A function that needs to call an I/O function must itself accept the required capability.
- Pure functions can be tested with nothing but their inputs and outputs. No mocks, no setup, no teardown, no dependency injection frameworks.
- Pure functions are safe to cache, parallelize, and reorder — the compiler and runtime can optimize aggressively.

**What this enables for LLMs:**

An LLM can write and test a pure function **entirely in isolation**. It does not need the rest of the codebase in its context window. Given only the function signature and the types of its parameters, the LLM has complete information to:

1. Implement the function.
2. Write tests for the function.
3. Reason about whether the function is correct.

This is a dramatic reduction in the context an LLM needs per task.

**Example — testing a pure function needs zero context:**

```
function calculate_discount(price: float, tier: string) returns float:
    if tier is "gold":
        return price * 0.8
    else if tier is "silver":
        return price * 0.9
    else:
        return price

verify calculate_discount:
    assert calculate_discount(100.0, "gold") is 80.0
    assert calculate_discount(100.0, "silver") is 90.0
    assert calculate_discount(100.0, "basic") is 100.0
```

The tests need nothing beyond the function itself. No database connection, no user session, no application state.

#### 3. No Deep Inheritance — Composition and Interfaces Only

Jett has **no class inheritance**. Object-oriented inheritance trees are one of the worst patterns for LLMs because understanding a single method may require tracing through 5+ levels of parent classes, mixins, and overrides scattered across many files.

Instead, Jett uses two mechanisms that keep relationships **flat and local**:

**Interfaces** (like Go interfaces or Rust traits):

```
interface Displayable:
    function display(self: view Displayable) returns string
```

An interface is just a contract — a list of function signatures. It carries no implementation, no state, no hidden behavior.

**Composition** (structs contain other structs):

```
struct EmailSender:
    config: SmtpConfig

    function send(self: EmailSender, stdout: Stdout, net: Network, message: Message) returns nothing:
        Stdout.write(stdout, "sending email")
        smtp.deliver(net, self.config, message)
        return
```

`EmailSender` uses an `SmtpConfig` by containing it — not by inheriting from it. Side effects (logging via `Stdout`, networking via `Network`) are declared as capability parameters. The relationship is visible in the struct definition and function signatures. An LLM can see every dependency by reading the struct fields and method signatures.

**Why this matters for LLMs:**

- To understand a struct, read its definition. That is all.
- To understand what functions a type supports, read its interface implementations. They are all in one place.
- There is no `super()` call chain to trace. No method resolution order. No diamond problem. No hidden overrides.
- An LLM can generate a correct struct implementation by looking at the struct definition and the interface definition — two flat, local pieces of information.

**Note on deep composition:** Composition can still produce deeply nested structures — a struct containing a struct containing a struct. This does not cause the same problems as deep inheritance. Each nested struct is self-describing: its fields are visible in its definition, it has no hidden overrides, and no method resolution order. Reading any struct's definition tells you everything it contains without tracing a parent chain. Deep nesting may be a code smell, but it is not a source of hidden behavior.

**Comparison with inheritance-based languages:**

```
# In a language with deep inheritance (what Jett avoids):
# To understand what Dog.speak() does, you must read:
#   Animal -> LivingThing -> Pet -> DomesticAnimal -> Dog
#   ...across 5 files, with possible overrides at each level.

# In Jett (flat, local, complete):
interface Speaker:
    function speak(self: view Speaker) returns string

struct Dog:
    name: string
    breed: string

implement Speaker for Dog:
    function speak(self: view Dog) returns string:
        return "woof"

# Calling a method — module syntax only:
let my_dog = Dog(name: "Rex", breed: "labrador")
let sound = Dog.speak(my_dog)
```

Structs define methods with `self` as the first parameter. Methods are called with module syntax: `Dog.speak(my_dog)`, `Point.distance(p1, p2)`. There is no `my_dog.speak()` form. This rule applies uniformly to ALL types, including capability types — `Stdout.write(stdout, msg)`, `Filesystem.read_file(fs, path)`, `Network.listen(net, addr, port)`. Capabilities are not an exception.

Everything about `Dog` is right here. No context needed from parent classes. No files to chase.

### Rule Set 3: Types as Guardrails

LLMs are probabilistic — they can and will hallucinate logic. The type system is the primary defense against this. If the type system is **strict and expressive enough**, writing correct code becomes a puzzle of making the types fit together. An LLM that satisfies the type checker has, by construction, produced code that meets the spec. The types *are* the spec.

#### 1. Strict Static Type System — Catching Hallucinations at Compile Time

Every value in Jett has a known type at compile time. There is no `any`, no untyped mode, no escape hatch. The type checker is deliberately strict: it rejects code that a more lenient system would allow, because for LLMs, a false rejection (compiler error the LLM can fix) is far cheaper than a false acceptance (hallucinated logic that silently passes and fails at runtime).

**What strict means in practice:**

- No implicit conversions. An `int` is not a `float` unless explicitly converted.
- No union types without exhaustive matching. All enums require exhaustive `match`, and all `result` types require `handle`.
- No null. Values are either present (`T`) or explicitly optional (`optional[T]`), and optionals must be unwrapped before use.
- No duck typing. A struct satisfies an interface only if it has an explicit `implement` block — accidental structural matches do not count.
- Function signatures are complete contracts. The parameter types, return type, and capability parameters fully describe what the function does. The compiler enforces this.

**Why this helps LLMs:**

When an LLM generates code that doesn't type-check, the compiler error tells it exactly what is wrong and what type was expected. The LLM can fix the error mechanically — it doesn't need to reason about runtime behavior or trace execution paths. The type system turns "is this code correct?" (a hard question) into "do these types match?" (an easy question).

**Example — the type system catches a hallucinated conversion:**

```
function format_price(cents: int) returns string:
    return "price is {cents}"
    # This works — int implements Displayable, so it can be used in string interpolation.

function add_to_price(price: string, tax: int) returns string:
    return price + tax
    # COMPILE ERROR: operator + is not defined for string and int
    # hint: use string interpolation "..." or convert types explicitly
```

In a dynamically typed language, type mismatches would silently produce garbage or crash at runtime. In Jett, the LLM gets an immediate, actionable error.

Jett uses string interpolation `"text {expr}"` as the single canonical mechanism for building strings. Expressions inside `{}` must implement the `Displayable` interface — the compiler calls `Displayable.display()` to produce the string representation. There is no `+` operator for strings and no `string.concat()` function.

#### Explicit Type Conversions

Jett has **no implicit type conversions**. An `int` is never silently promoted to a `float`, and a number is never silently coerced to a `string`. All type conversions are explicit function calls using the standard module function syntax: `TargetType.from_SourceType(value)`.

**Infallible conversions** (lossless — always succeed, return `T` directly):

```
let x = float.from_int(42)          # → 42.0
let s = string.from_int(42)         # → "42"
let s = string.from_float(3.14)     # → "3.14"
let s = string.from_bool(true)      # → "true"
```

**Fallible conversions** (can fail — return `result[T, string]`):

```
let n = int.from_string("42") handle error:
    return fail("not a number")

let f = float.from_string("3.14") handle error:
    return fail("not a float")

let n = int.from_float(3.14) handle error:
    return fail("not a whole number")
    # Fails because 3.14 is not exactly representable as int.
    # int.from_float(3.0) would succeed → 3
```

**Design rules:**

- Every conversion is a uniquely named function — no overloading. `int.from_string` and `int.from_float` are separate functions, not overloads of `int()`.
- Lossy numeric conversions return `result`. Converting `float` to `int` can fail because the float may not be a whole number. Converting `int` to `float` can fail for very large integers that lose precision. The compiler never silently truncates or rounds.
- The pattern is always `TargetType.from_SourceType(value)` — predictable and discoverable. An LLM can infer the correct function name from the types involved.
- String interpolation `"text {expr}"` requires the expression to implement `Displayable` — this is a compiler-stdlib coupling, not a general implicit conversion. Outside of interpolation, converting to string requires an explicit `string.from_int()` or `string.from_float()` call.

**What the compiler rejects:**

```
let x: float = 42
# COMPILE ERROR: expected float, got int
# hint: use float.from_int(42)

let y: int = 3.14
# COMPILE ERROR: expected int, got float
# hint: use int.from_float(3.14) and handle the possible error
```

**These are standard library functions, not language magic.** Primitive types (`int`, `float`, `string`, `bool`) serve as their own modules, exactly like structs do. When you define `struct Dog`, you call `Dog.speak(my_dog)` — `Dog` is both the type and the module. Primitive types work the same way: `int` is both the type (in `x: int`) and the module (in `int.from_string("42")`). The context disambiguates — type position vs expression position. There is no special compiler treatment for conversion functions; they are ordinary standard library functions that anyone could reimplement in a custom module.

#### 2. Intent-Based Refinement Types — Constraints in Plain Text

This is where Jett's type system becomes truly LLM-native. Standard types describe *what shape* data has (int, string, list). Refinement types describe *what rules* data must follow. The LLM can express business logic constraints directly as types, and the compiler enforces them automatically.

**Syntax:**

```
type Password = string where string.char_count(value) > 8
type Age = int where value >= 0 and value < 150
type Email = string where string.contains(value, "@")
type Port = int where value >= 1 and value <= 65535
type NonEmpty[T] = list[T] where list.length(value) > 0
type Percentage = float where value >= 0.0 and value <= 100.0
```

The `where` clause attaches a constraint to a base type. `value` refers to the value being constrained, and the clause accepts any pure expression (no capabilities, no mutation) that evaluates to `bool`. The compiler checks it wherever a value of that type is created.

**How refinement types work at runtime:**

Refinement types are checked at **type boundaries** — when a value enters the refined type from an unrefined one. Inside the type, the constraint is guaranteed. Assigning to a refinement type is a **fallible operation** — the value might not satisfy the constraint — so the compiler requires `handle`, just like any other operation that can fail:

```
function create_user(name: string, password: Password) returns User:
    # Inside this function, `password` is guaranteed to satisfy string.char_count > 8.
    # No need to check again. The type system already enforced it.
    return User(name: name, password: password)

# At the call site, the compiler forces you to handle the possible failure:
let user_password: Password = raw_input handle error:
    return fail("password must be at least 8 characters")
let user = create_user("alice", user_password)
```

This uses the same `handle error:` pattern as `result[T, E]` — no new syntax. The compiler **refuses to compile** a refinement type assignment without a `handle` block. The LLM is forced to consider and handle the case where the value does not satisfy the constraint.

**Why this is powerful for LLMs:**

1. **Constraints are readable English.** `type Password = string where string.char_count(value) > 8` is something an LLM can generate from a natural language requirement like "passwords must be at least 8 characters" with near-perfect accuracy.

2. **The compiler enforces what the LLM declares.** The LLM doesn't need to remember to add validation checks throughout the code — the type system does it automatically.

3. **Refinement types compose.** If a function takes a `Port` and a `NonEmpty[string]`, the LLM knows from the types alone that the port is valid and the list is non-empty. No defensive checks needed inside the function.

4. **Failure handling is mandatory.** The `handle` pattern forces the LLM to write error handling for every refinement type boundary. An LLM cannot silently assume a string is a valid password — the compiler requires an explicit `handle error:` block.

5. **Errors are caught early and described clearly:**

```
error at line 45: refinement type assignment requires error handling
  assigning int to Port may fail the constraint: value >= 1
  hint: add "handle error:" to handle the case where the value is invalid
```

**Complex refinement examples:**

```
type SortedList[T] = list[T] where list.is_sorted(value)
type BoundedList[T] = list[T] where list.length(value) <= 100
type PositiveFloat = float where value > 0.0

type HttpStatus = int where value >= 100 and value < 600
type JsonString = string where string.is_valid_json(value)

function parse_config(raw: JsonString) returns result[Config, string]:
    # `raw` is guaranteed to be valid JSON — the type says so.
    let config = json.parse(raw, Config) handle error:
        return fail("invalid config structure")
    return ok(config)
```

**Refinement types with struct fields:**

```
struct User:
    name: string where string.char_count(value) > 0
    email: Email
    age: Age

# Constructing a User validates all refined fields — and requires handle:
let user = User(name: name, email: email, age: age) handle error:
    return fail("invalid user data: {error}")
```

**Refinement type constraints must be self-contained.** The `where` clause can only reference `value` (the value being constrained) and call pure functions with literal or constant arguments. Constraints cannot take external parameters — there is no `type Password[min: int] = string where string.char_count(value) > min`. This keeps `[]` unambiguous: it always means generics, never parameterized constraints.

**For parameterized validation, use functions.** If validation rules depend on runtime values (e.g., a minimum password length from config), write a regular function that returns `result[T, string]`:

```
function validate_password(input: string, min_length: int) returns result[string, string]:
    if string.char_count(input) <= min_length:
        return fail("password must be longer than {min_length} characters")
    return ok(input)

# Usage:
let password = validate_password(raw_input, config.min_password_length) handle error:
    return fail(error)
```

The rule is simple: refinement types for fixed constraints, functions for dynamic validation.

**Refinement types are not implicitly usable as their base type.** A `Password` is not a `string` — it is a `Password`. You cannot pass a `Password` to a function that expects `string`. This follows the "no implicit conversions" rule (Rule Set 2) and keeps the LLM aware of type boundaries. To widen a refinement type to its base type, use the standard type conversion pattern:

```
let raw: string = string.from_Password(user_password)
```

This is the same pattern used for all other type conversions (`string.from_int()`, `int.from_string()`, etc.). The compiler **automatically generates** `BaseType.from_RefinedType()` for every refinement type — no manual implementation needed. If you need the base type multiple times in one function, assign it to a local variable once:

```
function process(password: Password, stdout: Stdout) returns nothing:
    let raw = string.from_Password(password)
    let len = string.char_count(raw)
    let upper = string.to_upper(raw)
    Stdout.write(stdout, "password length: {len}")
```

> **Why not implicit widening?** Implicit widening would hide information from the LLM. If `Password` silently becomes `string` wherever a string is expected, the LLM loses track of which values are validated and which are raw. Explicit conversion makes the type boundary visible in the source code — the LLM can see exactly where a validated value is being treated as a plain string, and can reason about whether that is intentional.

### Rule Set 4: Auto-Regressive Friendly Structure (Strict Linearity)

LLMs generate code **token-by-token, left-to-right, top-to-bottom**. They cannot look ahead. When an LLM writes a function call on line 10, it is committing to a name, argument list, and return type *right now* — if the actual definition doesn't appear until line 50, the LLM is guessing. By line 50, the model may have forgotten or drifted from what it assumed on line 10, producing mismatched signatures, wrong argument counts, or hallucinated parameter names.

Jett's structure must match the LLM's generation order exactly. Everything the LLM needs must already exist in its past context at the moment it is needed.

#### 1. No Forward Referencing — Strict Topological Order

The language enforces that **every variable, type, and function must be defined before it is used**. No exceptions, no forward declarations, no hoisting.

**Rules:**

- A function call on line N requires the function to be defined on some line M where M < N.
- A type annotation referencing `User` requires the `User` struct to be defined earlier in the file (or in an already-imported module).
- Mutual recursion (A calls B, B calls A) is handled with an explicit `mutual` block that declares both signatures upfront, keeping the forward reference contained and visible.
- Variables cannot be referenced before their `let` binding.

**What this looks like:**

```
# VALID — definition before use:
function double(x: int) returns int:
    return x * 2

function quadruple(x: int) returns int:
    return double(double(x))

# INVALID — forward reference:
function quadruple(x: int) returns int:
    return double(double(x))    # COMPILE ERROR: "double" is not defined yet

function double(x: int) returns int:
    return x * 2
```

**Mutual recursion — the only exception, explicitly declared:**

```
mutual:
    function is_even(n: int) returns bool
    function is_odd(n: int) returns bool

function is_even(n: int) returns bool:
    if n is 0:
        return true
    return is_odd(n - 1)

function is_odd(n: int) returns bool:
    if n is 0:
        return false
    return is_even(n - 1)
```

The `mutual` block puts both signatures into context before either body is written. This is the minimal, explicit escape hatch — no silent forward references allowed anywhere else.

**Why the keyword is `mutual`:** The only reason to forward-declare function signatures in Jett is mutual recursion — functions that depend on each other in a cycle. If function A needs function B, you simply define B first. The only case where that is impossible is when A calls B and B calls A. The keyword `mutual` communicates this intent directly: an LLM seeing `mutual:` immediately knows "these functions call each other." A more generic keyword like `declare` or `forward` would describe the mechanism without explaining why it exists.

**Why this matters for LLMs:**

This perfectly mirrors the auto-regressive generation process. When the LLM writes `return double(double(x))`, the definition of `double` is already in its past context — it knows the exact signature, parameter types, and return type. It is not guessing. The code generation order *is* the dependency order.

#### 2. Inline Dependency Declarations — Context Where Attention Is

Traditional languages put all imports at the top of a file. By the time the LLM is generating code on line 200, those imports are far away in its context — potentially outside its effective attention window. The LLM may forget which module a function came from, or hallucinate an import that doesn't exist.

Jett requires **all imports to be declared locally**, inside a function or block, right where they are used. File-level imports are banned. This keeps the relevant context exactly where the LLM's attention mechanism is focused.

**What the compiler rejects:**

```
namespace server

use auth          # COMPILE ERROR: imports must be inside a function or block
use models        # COMPILE ERROR: imports must be inside a function or block

function handle_login(stdout: Stdout) returns nothing:
    ...
```

**What you write instead — all imports inside functions:**

```
function fetch_data(net: Network, url: string) returns result[map[string, string], HttpError]:
    use net.http
    use json
    let response = http.get(net, url) handle error:
        return fail(error)
    let data = json.parse(response.body, map[string, string]) handle error:
        return fail(HttpError.status_error(0, error))
    return ok(data)

function compute_stats(values: list[float]) returns float:
    use math
    let total = math.sum(values)
    return total / float.from_int(list.length(values))
```

**What this achieves:**

- Each function is **self-contained**. Its dependencies are declared inside it, right before they are used.
- An LLM generating `fetch_data` sees `use net.http` and `use json` immediately in local context — not 150 lines away at the top of the file.
- When an LLM reads or modifies a single function, it has **complete information** without scrolling or searching. The function is a self-describing unit.
- Removing a function automatically removes its imports. No orphaned imports accumulating at the top of files.
- If 10 functions use `math`, you write `use math` 10 times. The token cost is trivial — and the compiler resolves the import once regardless.

**Scoping rules:**

- A `use` statement is scoped to the block it appears in. `use math` inside a function is not visible outside that function.
- All `use` statements in a function must appear at the top, before any other code. Imports scattered throughout the function body are a compile error. This gives every function a predictable structure: imports first, then logic.
- A module imported in two functions is resolved once by the compiler — no runtime cost to repeated `use` statements.

```
function good_example(stdout: Stdout) returns nothing:
    use math
    use json
    let x = math.sqrt(2.0)
    Stdout.write(stdout, json.serialize(x))

function bad_example(stdout: Stdout) returns nothing:
    let x = 42
    use math          # COMPILE ERROR: use statements must appear before any other code
    Stdout.write(stdout, "value: {x}")
```

**Why this matters for LLMs:**

The LLM attention mechanism works best on nearby tokens. Inline imports guarantee that every piece of information needed to understand a block of code is physically close to that block. The LLM never has to "remember" what was imported 200 lines ago — it is right there. If file-level imports were allowed, LLMs would default to them — because that is what all their training data does — defeating the purpose of inline imports entirely.

### Rule Set 5: Zero Hidden Control Flow (Errors as Values Only)

LLMs struggle immensely with non-linear execution. Exceptions — `try`/`catch`/`throw` — are the worst offender. An exception thrown inside a deeply nested function call can silently bubble up through 10 stack frames and get caught in a completely different file. An LLM generating code locally has no way to see this. It doesn't know that `user.save()` might throw a `DatabaseConnectionError` that skips the next 30 lines and lands in a catch block three functions up.

Jett eliminates this entirely. **There are no exceptions.** Control flow always goes explicitly downward through the code. Errors are values, and handling them is mandatory.

#### 1. Mandatory Explicit Error Handling — Handle It on the Next Line

Functions that can fail return a `result[T, E]` type. The caller **must** handle the error immediately — the compiler refuses to let an error result be ignored or silently propagated.

**The pattern:**

```
function read_config(fs: Filesystem, path: string) returns result[Config, string]:
    let raw = Filesystem.read_file(fs, path) handle error:
        return fail("could not read file: {path}")
    let config = json.parse(raw, Config) handle error:
        return fail("invalid config format")
    return ok(config)
```

The `handle` keyword is used at the call site of any function that returns a `result`. It is **not optional** — the compiler enforces it. If you call a function that can fail, you must handle the failure right there, on the very next line, while the context of what you just called is at the peak of the LLM's attention window.

**Why `handle` and not `catch`:**

`catch` implies exceptions — something was thrown and caught mid-flight. `handle` implies values — a result was returned and you are dealing with it. The naming reinforces the mental model: there is no throwing, no catching, no flight. There is only: call, check, continue.

**The compiler enforces handling:**

```
function bad_example(fs: Filesystem) returns string:
    let config = read_config(fs, "app.conf")
    # COMPILE ERROR: result[Config, string] must be handled
    # "read_config" can fail, but the error is not handled
    # hint: add a "handle" block after the call

    return config.name   # this line is never reached by the compiler
```

You cannot accidentally ignore an error. The compiler will not let the program compile until every `result` is explicitly handled.

**Result type structure:**

```
# result[T, E] is a built-in generic type with two variants:
#   ok(value: T)    — the operation succeeded
#   fail(error: E)  — the operation failed
```

`result[T, E]` is fully generic — both `T` and `E` can be any type. There is no built-in error type. `fail()` takes a value of whatever type `E` is. For simple cases, use `string` as the error type. For complex cases, define a custom error struct or enum:

```
# Simple — E is string:
function read_config(fs: Filesystem, path: string) returns result[Config, string]:
    let raw = Filesystem.read_file(fs, path) handle error:
        return fail("could not read file")
    return ok(json.parse(raw, Config))

# Complex — E is a custom enum:
enum DatabaseError:
    connection_failed(message: string)
    query_failed(query: string, reason: string)
    timeout

function query(net: Network, sql: string) returns result[list[Row], DatabaseError]:
    # ...

let rows = query(net, "select * from users") handle error:
    match error:
        connection_failed(msg):
            return fail("db down: {msg}")
        query_failed(q, reason):
            return fail("bad query: {q} — {reason}")
        timeout:
            return fail("db timed out")
```

**The `handle` keyword — the only way to unwrap a result or optional:**

The `handle` keyword is the **single canonical form** for unwrapping `result` and `optional` values. There is no alternative. You cannot use `match` on a `result` type — `match` is reserved for user-defined enums only.

The syntax form is **mandatory** and encodes the type being unwrapped:

- **`result[T, E]` MUST use `handle error:`** — the error variable is always bound. The `error` keyword is required because results carry error information, and the caller must have access to it.
- **`optional[T]` MUST use bare `handle:`** — no error variable, because there is no error. The value is simply absent.

```
# Return form — exit the function on error:
let config = read_config(fs, "app.conf") handle error:
    return fail("config load failed")

# Default form — provide a fallback value:
let config = read_config(fs, "app.conf") handle error:
    default Config(port: 8080)

# Default form with side effects — log the error, then provide a fallback:
let config = read_config(fs, "app.conf") handle error:
    Stdout.write(stdout, "config failed, using defaults: {error}")
    default Config(port: 8080)
```

The `handle error:` block executes only when the result is `fail`. If the result is `ok`, the unwrapped value is bound to the variable on the left (`config`). The error variable is always available inside the block.

**Every handle block must end with either `return` or `default`** — there is no implicit value. This rule applies to both `handle error:` (for `result`) and bare `handle:` (for `optional`). A handle block that does neither is a compile error:

```
# result — COMPILE ERROR:
let config = read_config(fs, "app.conf") handle error:
    Stdout.write(stdout, "something failed")
    # COMPILE ERROR: handle block must end with "return" or "default"
    # hint: add "return fail(...)" to exit, or "default <value>" to provide a fallback

# optional — COMPILE ERROR:
let first = list.first(items) handle:
    Stdout.write(stdout, "list was empty")
    # COMPILE ERROR: handle block must end with "return" or "default"

# optional — valid with default:
let first = list.first(items) handle:
    Stdout.write(stdout, "list was empty, using fallback")
    default Item(name: "unknown")
```

This is consistent with Jett's "no implicit returns" principle. In functions, you always write `return`. In handle blocks, you always write `return` or `default`. Nothing is ever silently inferred from the last expression.

**Return values must be consumed — no silent discards:**

A function that returns anything other than `nothing` cannot be called as a standalone statement. The return value must be assigned to a variable. This is enforced by Jett's linear type system (Rule Set 10) — every value must be consumed.

```
# returns nothing — OK as standalone statement:
Stdout.write(stdout, "hello")

# returns float — MUST assign:
math.sqrt(16.0)
# COMPILE ERROR: return value of math.sqrt (float) is not consumed
# hint: assign to a variable with "let x = math.sqrt(16.0)"

# returns result[T, E] — MUST assign AND handle:
read_config(fs, "app.conf")
# COMPILE ERROR: result[Config, string] is not consumed and not handled
# hint: assign and handle with "let config = read_config(...) handle error: ..."
```

This means there is always a variable on the left side of a `handle` block. The `default` keyword always has a target to assign to, and return values can never be silently ignored.

**Why `match` is not allowed on results:**

One canonical form means one way to unwrap. `match` on a `result` would create a second way to do the same thing as `handle`. By restricting `match` to user-defined enums, Jett enforces that all error handling looks identical everywhere. An LLM never has to decide between `match` and `handle` — there is only `handle`.

**`handle` also unwraps `optional[T]`:**

The `handle` keyword works for `optional[T]` values using the bare `handle:` form (no `error` keyword). If the value is `none`, the handle block executes:

```
let first_item = list.first(items) handle:
    return fail("list is empty")

let user = db.find_user(users, id) handle:
    return fail("user not found: {id}")
```

This means `handle` is the single canonical unwrap mechanism for both `result[T, E]` and `optional[T]`. The distinction between the two is encoded in the syntax form.

**The form of `handle` tells you what you're unwrapping -- and the form is mandatory:**

- **`result[T, E]` MUST use `handle error:`** -- the `error` keyword is required. The error variable is always bound inside the block:
  ```
  let config = read_config(fs, "app.conf") handle error:
      Stdout.write(stdout, error)
      return fail(error)
  ```
- **`optional[T]` MUST use bare `handle:`** with **no error variable**, because there is no error -- the value is simply absent:
  ```
  let user = find_user(id) handle:
      return fail("user not found")
  ```

This distinction is mandatory -- using the wrong form is a **compile error**. `handle:` on a `result[T, E]` is rejected. `handle error:` on an `optional[T]` is rejected. The syntax form encodes the type being unwrapped, and the compiler enforces it. When an LLM sees `handle error:`, it knows the expression returns `result[T, E]`. When it sees `handle:`, it knows the expression returns `optional[T]`.

#### 2. No Global Exits — Control Flow Goes Down, Never Sideways

Jett bans every construct that causes control flow to jump to a non-obvious destination.

**Banned constructs:**

| Construct | Why it is banned |
|-----------|-----------------|
| `throw` / exceptions | Invisible control flow jumping across stack frames. LLMs cannot track where an exception will be caught. |
| `goto` | Arbitrary jumps. Makes code flow unpredictable. |
| `exit()` / `abort()` | Global program termination from arbitrary locations. Hides the fact that a function can kill the entire process. |
| `panic` / unrecoverable errors | Same as `exit()` — a hidden control flow nuke. If something is truly unrecoverable, the `main` function returns an error. |
| Implicit exception propagation | In Python/Java, calling a function that throws can silently propagate. In Jett, errors are values — they don't propagate unless you explicitly return them. |

**What is allowed:**

| Construct | Why it is allowed |
|-----------|-----------------|
| `return` | Explicit, visible, goes to the caller. Always at the current function scope. |
| `break` | Exits the current loop. Scope is visible and local. |
| `continue` | Skips to the next loop iteration. Scope is visible and local. |
| Early `return` in guard clauses | Encouraged. Reduces nesting. The return target is always the current function. |

**The rule:** at any point in the code, you can determine where execution goes next by reading the **current line and the lines immediately below it**. There is never a hidden jump to a distant handler. Control flows down the AST, always.

**Example — error propagation is always visible:**

```
function process_order(net: Network, order_id: string) returns result[Receipt, string]:
    use db
    use payment

    let order = db.find_order(net, order_id) handle error:
        return fail("order not found: {order_id}")

    let charge = payment.charge(net, order.total, order.card) handle error:
        return fail("payment failed for order: {order_id}")

    let receipt = db.save_receipt(net, order, charge) handle error:
        return fail("could not save receipt")

    return ok(receipt)
```

Reading this function top to bottom, the LLM (or a human) can trace every possible execution path:
1. `db.find_order` succeeds → continue. Fails → return error.
2. `payment.charge` succeeds → continue. Fails → return error.
3. `db.save_receipt` succeeds → continue. Fails → return error.
4. Return success.

There are no hidden paths. No exception from `payment.charge` silently skipping the `db.save_receipt` line. No catch block 5 functions away. Every branch is explicit and local.

### Rule Set 6: Elimination of Attention-Splitting Ambiguity

LLMs use attention heads to link tokens together — to figure out what `user` refers to, what `process` means, what value `count` currently holds. When multiple things share similar names, or when a name can mean different things in different scopes, the attention mechanism gets confused. The model may merge logic from two different variables, hallucinate a function overload that doesn't exist, or lose track of a variable's current value after it was mutated 20 lines ago.

Jett eliminates every source of this ambiguity.

#### 1. Zero Variable Shadowing

If a variable named `user_id` exists in an outer scope, creating another variable named `user_id` in an inner scope is a **compile error**. There is never a question of "which `user_id`?" — there is only one, always.

**What the compiler rejects:**

```
function process_user(net: Network, user_id: string) returns result[User, string]:
    use db
    let user = db.find(net, user_id) handle error:
        return fail("not found")

    for item in user.orders:
        let user_id = item.buyer_id
        # COMPILE ERROR: "user_id" already exists in an outer scope
        # hint: use a distinct name, e.g. "buyer_id"
```

**What you write instead:**

```
function process_user(net: Network, user_id: string) returns result[User, string]:
    use db
    let user = db.find(net, user_id) handle error:
        return fail("not found")

    for item in user.orders:
        let buyer_id = item.buyer_id
        # Clear. Unambiguous. No confusion possible.
```

**Why this matters for LLMs:**

When an LLM sees `user_id` anywhere in a function, it resolves to exactly one binding. The attention head linking `user_id` on line 15 to its definition doesn't have to choose between two competing candidates. There is one `user_id`, defined in one place, with one value. The LLM cannot accidentally read from or write to the wrong one.

#### 2. No Function Overloading

Having `process(string)` and `process(int)` in the same codebase splits the LLM's understanding of what `process` means. When the LLM generates a call to `process`, it must infer from argument types which overload it intends — and it may get it wrong, especially when types are similar or when the function is being called with a variable whose type was defined many lines ago.

Jett bans function overloading entirely. **Every function has a unique name.**

**What the compiler rejects:**

```
function process(data: string) returns string:
    return parse_text(data)

function process(data: int) returns int:
    # COMPILE ERROR: function "process" is already defined
    # hint: use a distinct name, e.g. "process_int"
    return data * 2
```

**What you write instead:**

```
function process_text(data: string) returns string:
    return parse_text(data)

function process_number(data: int) returns int:
    return data * 2
```

**The rule extends to methods on structs:**

```
struct Parser:
    # NOT allowed:
    # function parse(self: view Parser, input: string) returns Ast
    # function parse(self: view Parser, input: list[Token]) returns Ast

    # Required — distinct names:
    function parse_text(self: view Parser, input: string) returns Ast:
        let tokens = tokenize(input)
        return Parser.parse_tokens(self, tokens)

    function parse_tokens(self: view Parser, input: list[Token]) returns Ast:
        return build_ast(input)
```

**Why this matters for LLMs:**

The word `process` maps to exactly one function. The word `parse` maps to exactly one function. When the LLM generates a function call, there is zero ambiguity about what will be invoked. The name *is* the identity — no type-based disambiguation needed.

#### 3. Immutable by Default — New Name for New Value

Variables in Jett cannot change value once assigned. If state must change, a **new variable with a new name** must be explicitly created. The `mutable` keyword exists as an opt-in escape hatch for performance-critical loops, but the default and idiomatic style is immutable bindings.

**Immutable (default and encouraged):**

```
function normalize_name(raw_name: string) returns string:
    let trimmed_name = string.trim(raw_name)
    let lower_name = string.lower(trimmed_name)
    let clean_name = string.replace(lower_name, "  ", " ")
    return clean_name
```

Each transformation gets a new name. At any point in this function, the LLM knows exactly what every variable holds — `raw_name` is the original input, `trimmed_name` is after trimming, `lower_name` is after lowering. There is no need to "scroll back" to figure out the current state of a variable that was reassigned 3 times.

**What the compiler rejects without `mutable`:**

```
function normalize_name(raw_name: string) returns string:
    let name = string.trim(raw_name)
    name = string.lower(name)
    # COMPILE ERROR: "name" is not mutable
    # hint: use "let mutable name" or create a new variable
```

**Mutable (opt-in, for when it is genuinely needed):**

```
function sum_list(items: list[int]) returns int:
    let mutable total = 0
    for item in items:
        total = total + item
    return total
```

The `mutable` keyword is a visible flag. When an LLM sees `let mutable total`, it knows this variable will change and must track its state. When it sees `let trimmed_name`, it knows this value is fixed forever. The distinction is explicit and permanent — no guessing.

**Why this matters for LLMs:**

A mutable variable that gets reassigned on lines 5, 12, and 23 requires the LLM to maintain a "mental timeline" of its value. By line 30, the LLM must remember that `count` was reassigned on line 23, not that it still holds the value from line 5. LLMs are bad at this — they attend to all occurrences of `count` simultaneously, not chronologically.

Immutable variables eliminate this entirely. `trimmed_name` has one value, forever, from the line it is defined. There is no timeline to track. The LLM's attention head links the name to one definition and one value — done.

**Mutability is local only — no mutable references.** There is no way for a function to modify the caller's data. When a value is passed to a function, it is either consumed (moved) or borrowed read-only via `view`. There is no `param: mutable T` — no mutable references exist in the language. If a function needs to transform a value, it takes ownership, transforms it, and returns the new value. The caller rebinds:

```
let mutable x = 5
x = transform(x)    # transform consumes x, returns new value, x is rebound
```

This guarantees that reading a mutable variable's current value never requires looking at function implementations. The only place `x` can change is at explicit rebinding statements in the same scope.

### Rule Set 7: Syntactically Enforced Modularity (Chunking)

LLM performance degrades as the amount of code in a single block grows. Attention gets diluted across thousands of tokens, and the model starts losing track of variables, control flow, and intent. The solution is not to hope the LLM writes small functions — it is to make the compiler **refuse to accept large ones**.

Jett enforces hard limits on function complexity. These are not style guidelines or linter warnings — they are **compile errors**. The language physically prevents monolithic code from existing.

#### 1. Strict Scope Limits — The Compiler Enforces Chunking

Every function in Jett has a maximum allowed complexity. If a function exceeds the limit, it does not compile. The LLM (or human) must break it into smaller, self-contained pieces.

**Enforced limits:**

| Metric | Limit | Rationale |
|--------|-------|-----------|
| Statements per function | 50 max | Keeps each function within a tight attention window. |
| Nesting depth | 4 levels max | Deeply nested code is hard for LLMs to track. Guards and early returns reduce nesting. |
| Parameters per function | 6 max | Too many parameters signals the function is doing too much. Use a struct. |
| Cyclomatic complexity | 10 max | Limits the number of branching paths the LLM must reason about simultaneously. |

**Capability bundles:**

When a function needs multiple capabilities, they can be grouped into a struct to conserve parameter slots:

```
struct AppCapabilities:
    fs: Filesystem
    net: Network
    stdout: Stdout
    stderr: Stderr

function deploy_service(caps: AppCapabilities, config: Config, target: Server) returns nothing:
    let manifest = Filesystem.read_file(caps.fs, "manifest.json") handle error:
        Stderr.write(caps.stderr, "failed to read manifest")
        return
    Stdout.write(caps.stdout, "deploying...")
    # 3 parameters instead of 7
```

Capability bundles are regular structs — they can be constructed, destructured, and passed around like any value. The compiler tracks the individual capabilities inside the bundle for lineage and purity analysis.

**What the compiler produces when limits are exceeded:**

```
error at line 45: function "process_all_orders" exceeds the statement limit
  current: 67 statements (max: 50)
  hint: extract related statements into smaller functions

error at line 12: function "validate_input" exceeds the nesting depth limit
  current: 5 levels (max: 4)
  hint: use guard clauses with early returns to reduce nesting
```

**Example — the compiler forces decomposition:**

This function is too large and the compiler rejects it:

```
function process_report(data: list[Record]) returns result[Report, string]:
    # ... 60+ statements doing validation, transformation,
    # aggregation, formatting, and output ...
    # COMPILE ERROR: exceeds 50 statement limit
```

The LLM must break it apart:

```
function validate_records(data: list[Record]) returns result[list[Record], string]:
    # validation logic (~10 statements)

function transform_records(records: list[Record]) returns list[TransformedRecord]:
    # transformation logic (~10 statements)

function aggregate_results(records: list[TransformedRecord]) returns Summary:
    # aggregation logic (~10 statements)

function format_report(summary: Summary) returns Report:
    # formatting logic (~10 statements)

function process_report(data: list[Record]) returns result[Report, string]:
    let valid = validate_records(data) handle error:
        return fail("invalid records")
    let transformed = transform_records(valid)
    let summary = aggregate_results(transformed)
    return ok(format_report(summary))
```

The result: `process_report` is now 4 lines. Each helper function is small, focused, and independently understandable. An LLM can generate, test, and reason about each one in isolation without its attention being diluted across a massive block.

**Why hard limits instead of soft warnings:**

Linter warnings are suggestions — LLMs (and humans) ignore them. A compile error is absolute. The LLM's code generation loop becomes: write function → compile → if too large, decompose → compile again. This loop naturally produces well-chunked code without any prompting or instructions. The language structure *forces* good architecture.

> **Note:** The limits target logic complexity, not data size. Struct construction is a single expression regardless of field count — a 100-field struct literal is one statement. Struct functions each have their own independent 50-statement limit, so a struct with many functions is not a problem. Heavy math or sequential I/O that appears to need 50+ statements is almost always decomposable into named sub-computations (`calculate_velocity`, `apply_drag`, `resolve_collision`) or grouped operations (`load_configs`, `load_assets`), which produces better code. These limits have no flags or per-function overrides — they are absolute. If the compiler rejects a function, the function is doing too much.

**Nesting depth enforcement — guards over nesting:**

```
# REJECTED — nesting depth 5:
function find_active_user(users: list[User], role: string) returns optional[User]:
    for user in users:
        if user.active:
            if user.role is role:
                if user.verified:
                    if user.age > 18:    # depth 5 — COMPILE ERROR
                        return some(user)
    return none

# ACCEPTED — flat with guards:
function find_active_user(users: list[User], role: string) returns optional[User]:
    for user in users:
        if not user.active:
            continue
        if user.role != role:
            continue
        if not user.verified:
            continue
        if user.age <= 18:
            continue
        return some(user)
    return none
```

Same logic, but flat. Each condition is a guard clause that skips to the next iteration. The LLM reads straight down — no bracket matching, no deep indentation tracking, no "which `if` does this `else` belong to?" ambiguity.

**Parameter count enforcement — structs over long signatures:**

```
# REJECTED — 8 parameters:
function create_user(name: string, email: string, age: int, role: string,
                     team: string, manager: string, office: string,
                     start_date: string) returns User:
    # COMPILE ERROR: exceeds 6 parameter limit

# ACCEPTED — grouped into a struct:
struct UserRequest:
    name: string
    email: string
    age: Age
    role: string
    team: string
    manager: string
    office: string
    start_date: string

function create_user(request: UserRequest) returns User:
    # clean, one parameter, all fields accessible via request.name etc.
```

### Rule Set 8: Extremely Dense, Opinionated Standard Library

Every token an LLM spends writing a sorting algorithm, a date parser, or an array filter is a token wasted — and a chance to hallucinate. Hand-written algorithmic logic is where LLMs make their worst mistakes: off-by-one errors, wrong loop bounds, missed edge cases. The solution is to **never let the LLM write that logic in the first place**.

Jett ships with a massive, opinionated standard library that covers virtually every common operation. The LLM's job is reduced from "write algorithms" to "connect modules" — turning Jett into a high-level orchestration language where the LLM writes plumbing, not logic.

#### 1. Macro-Primitives — High-Level Operations as Built-Ins

Instead of providing low-level building blocks and hoping the LLM assembles them correctly, Jett provides **hyper-specific, high-level standard functions** for common tasks. These are battle-tested, edge-case-handled implementations that the LLM simply calls by name.

**Principle: if an LLM would need a `for` loop to do it, there should be a standard function instead.**

**List operations — no manual loops:**

```
# Instead of writing a filter loop:
let adults = list.filter(users, function(u: User) returns bool: return u.age >= 18)

# Instead of writing a map loop:
let names = list.map(users, function(u: User) returns string: return u.name)

# Instead of writing a reduce loop:
let total = list.sum(prices)

# Instead of writing a search loop:
let found = list.find(users, function(u: User) returns bool: return u.id is target_id)

# Instead of writing a sort with comparator:
let sorted = list.sort_by(users, function(u: User) returns int: return u.age)

# Instead of writing deduplication logic:
let unique = list.unique(items)

# Instead of writing chunk/batch logic:
let batches = list.chunk(items, 100)

# Instead of writing zip logic:
let pairs = list.zip(names, scores)

# Group by a field:
let by_role = list.group_by(users, function(u: User) returns string: return u.role)
```

An LLM calling `list.filter` cannot produce an off-by-one error. It cannot forget to handle an empty list. It cannot accidentally mutate the original. The standard library handles all of this.

**String operations — no manual parsing:**

```
let trimmed = string.trim(input)
let parts = string.split(csv_line, ",")
let joined = string.join(names, ", ")
let replaced = string.replace(text, "old", "new")
let upper = string.upper(name)
let lower = string.lower(name)
let contains = string.contains(email, "@")
let starts = string.starts_with(url, "https")
let padded = string.pad_left(code, 6, "0")
let slug = string.slugify("Hello World!")        # "hello-world"
let truncated = string.truncate(bio, 100, "...") # cut with suffix
let extracted = string.between(html, "<title>", "</title>")
```

No regex for simple operations. No manual index arithmetic. Each function does one thing, is named obviously, and handles edge cases internally.

**Date and time — no manual formatting:**

```
use time

let now = Clock.now(clock)
let formatted = time.format(now, "YYYY-MM-DD")
let parsed = time.parse("2025-03-15", "YYYY-MM-DD") handle error:
    return fail("invalid date")
let diff = time.difference(start, end)
let tomorrow = time.add_days(now, 1)
let weekday = time.day_of_week(now)
let is_before = time.before(start, end)
let age = time.years_between(birth_date, now)
```

Date logic is one of the most error-prone areas in programming. An LLM should never be computing leap years or timezone offsets — the standard library does it correctly.

**JSON — zero boilerplate:**

```
use json

let config = json.parse(raw_string, Config) handle error:
    return fail("invalid json")                              # string to typed value
let text = json.serialize(config)                        # value to string
let pretty = json.serialize_pretty(config)               # value to formatted string

# For dynamic/untyped JSON access (when the schema is unknown):
let raw = json.parse_raw(raw_string) handle error:
    return fail("invalid json")                              # string to raw json value
let field = json.get(raw, "user.address.city") handle:
    return fail("field not found")                           # nested field access by path
let safe = json.get_or(raw, "user.nickname", "anon")     # with default
```

**HTTP — high-level client out of the box:**

The `net.http` module defines its own error type for HTTP operations:

```
# Defined by the net.http standard library module:
enum HttpError:
    connection_failed(message: string)
    timeout(message: string)
    status_error(code: int, message: string)
```

```
use net.http

let response = http.get(net, "https://api.example.com/users") handle error:
    # error is HttpError — match to handle specific cases:
    match error:
        HttpError.timeout(msg):
            return fail(HttpError.timeout(msg))
        other:
            return fail(other)

let body = json.parse(response.body, list[User]) handle error:
    return fail("invalid json")
let status = response.status

# POST with body:
let post_response = http.post(net, "https://api.example.com/users", json.serialize(new_user)) handle error:
    return fail(error)
```

**File system — simple and complete:**

```
let content = Filesystem.read_file(fs, "config.json") handle error:
    return fail("file not found")

Filesystem.write_file(fs, "output.txt", data) handle error:
    return fail("could not write")

let files = Filesystem.list_dir(fs, "./data") handle error:
    return fail("directory not found")

let exists = Filesystem.file_exists(fs, "config.json")
let size = Filesystem.file_size(fs, "data.bin") handle error:
    return fail("could not get file size")
Filesystem.copy_file(fs, "source.txt", "dest.txt") handle error:
    return fail("could not copy file")
Filesystem.delete_file(fs, "temp.txt") handle error:
    return fail("could not delete file")
```

**Math — common operations without manual implementation:**

```
use math

let clamped = math.clamp(value, 0, 100)
let rounded = math.round(price, 2)
let absolute = math.abs(difference)
let maximum = math.max(a, b)
let minimum = math.min(a, b)
let average = math.average(scores)
let median = math.median(scores)
let floored = math.floor(3.7)
let ceiled = math.ceil(3.2)
let power = math.pow(base, exponent)
```

**Hashing and encoding — no third-party dependencies:**

```
use crypto
use encoding

let hashed = crypto.sha256(password)
let b64 = encoding.base64_encode(data)
let decoded = encoding.base64_decode(b64)
let url_safe = encoding.url_encode(query)
let hex = encoding.hex_encode(bytes)
```

**Validation — standard library refinement types:**

The `validate` module provides common formats as refinement types. The type IS the validation — once assigned, the value is guaranteed valid:

```
use validate

# Assignment enforces validation via the refinement type constraint:
let email: validate.Email = user_input handle error:
    return fail("invalid email")

let url: validate.URL = link handle error:
    return fail("invalid url")

let id: validate.UUID = raw_id handle error:
    return fail("invalid uuid")

let addr: validate.IPv4 = ip_string handle error:
    return fail("invalid ip")

# Functions declare the validated type — no re-validation needed:
function send_email(net: Network, to: validate.Email, body: string) returns result[nothing, string]:
    # "to" is guaranteed to be a valid email by the type system
    # ...
```

#### 2. The Orchestration Principle

With a dense standard library, the LLM's role shifts fundamentally. It is no longer writing algorithms — it is **connecting well-tested components**. A typical Jett program written by an LLM looks like:

```
function process_csv_report(fs: Filesystem, clock: Clock, path: string) returns result[Report, string]:
    use string
    use list
    use time

    let raw = Filesystem.read_file(fs, path) handle error:
        return fail("could not read file")

    let lines = string.split(raw, "\n")
    let rows = list.map(lines, function(line: string) returns list[string]:
        return string.split(line, ","))

    let header = list.first(rows) handle:
        return fail("CSV file is empty")
    let data = list.skip(rows, 1)

    let filtered = list.filter(data, function(row: list[string]) returns bool:
        let cell = list.get(row, 2) handle:
            return false
        return string.is_not_empty(cell))

    let sorted = list.sort_by_index(filtered, 0)

    let report = Report(
        generated: Clock.now(clock),
        row_count: list.length(sorted),
        data: sorted
    )

    return ok(report)
```

This function reads a CSV file, parses it, filters empty rows, sorts by the first column, and builds a report. **Not a single `for` loop. Not a single manual index calculation. Not a single edge case to get wrong.** Every operation is a standard library call. The LLM is writing plumbing — declaring what should happen — not implementing how it happens.

**Why this matters for LLMs:**

1. **Fewer tokens generated.** `list.filter(data, predicate)` is far fewer tokens than a hand-written filter loop.
2. **Zero algorithmic hallucination.** The LLM cannot produce an off-by-one error in `list.sort_by` because it didn't write the sort.
3. **Predictable function names.** An LLM that has seen `list.filter` once will use it correctly every time. The name is the documentation.
4. **Testability.** Standard library functions are already tested. The LLM only needs to test its orchestration logic — the glue between calls.

### Rule Set 9: Native State-Machine Architecture

#### The Problem: "Boolean Soup" and LLM Amnesia

In traditional languages, developers track system state using multiple boolean flags or variables:

```
let mutable is_loading = true
let mutable is_logged_in = false
let mutable has_error = false
let mutable is_banned = false
```

Humans implicitly understand the rules between these variables — a user cannot be `is_logged_in = true` AND `is_banned = true` simultaneously. But those rules exist only in the programmer's head. They are nowhere in the code.

LLMs are terrible at maintaining implicit rules across long generations. Because an LLM generates text left-to-right, if it sets `is_banned = true` on line 20, by the time it reaches line 80 its attention mechanism may "forget" to set `is_logged_in = false`. The result: an illegal, contradictory state — the user is simultaneously logged in and banned. A bug that is invisible in the code because nothing in the language prevents it.

This problem gets worse as the number of flags grows. Four booleans create 16 possible combinations, but maybe only 5 of those combinations are valid. The other 11 are bugs waiting to happen, and an LLM has no way to know which is which from the code alone.

#### The Solution: Finite State Machines as a Core Language Primitive

Jett eliminates boolean soup by making **finite state machines (FSMs) a foundational syntax** — as native to the language as `if` statements or `for` loops. Instead of loose flags, you explicitly declare the allowed states and the allowed transitions between them. The compiler enforces that no illegal state can ever exist.

**Declaring a state machine:**

```
machine UserAuth:
    states:
        guest
        authenticating(user_id: string)
        logged_in(user_id: string)
        banned(user_id: string)

    transitions:
        guest to authenticating
        authenticating to logged_in
        authenticating to guest
        logged_in to guest
        logged_in to banned
```

This declaration is the **single source of truth** for the entire lifecycle of user authentication. It says:

- There are exactly 4 states. No fifth state can sneak in.
- A user can go from `guest` to `authenticating`, but never from `guest` directly to `logged_in`.
- A user can go from `logged_in` to `banned`, but never from `banned` back to `logged_in`.
- Every legal path is listed. Every unlisted path is illegal and will be rejected by the compiler.

**Using the state machine:**

```
function start_login(session: UserAuth at guest, user_id: string) returns UserAuth at authenticating:
    # This function can ONLY be called when session is in the "guest" state.
    # It MUST return the session in the "authenticating" state.
    # The compiler enforces both of these constraints.
    return UserAuth.transition(session, authenticating, user_id: user_id)

function complete_login(session: UserAuth at authenticating) returns UserAuth at logged_in:
    return UserAuth.transition(session, logged_in, user_id: session.user_id)

function ban_user(session: UserAuth at logged_in) returns UserAuth at banned:
    return UserAuth.transition(session, banned, user_id: session.user_id)
```

If the LLM tries to write a function that transitions from `banned` to `logged_in`:

```
function unban_user(session: UserAuth at banned) returns UserAuth at logged_in:
    return UserAuth.transition(session, logged_in, user_id: session.user_id)
    # COMPILE ERROR: transition from "banned" to "logged_in" is not declared
    # declared transitions from "banned": (none)
    # hint: add "banned to logged_in" to the transitions block if this is intended
```

The compiler catches the illegal transition immediately. The LLM gets a clear error and either fixes the code or adds the transition to the machine definition (making the design decision explicit).

#### Why This Is Perfect for LLMs

**1. Collapses context into a single point of truth.**

Instead of the LLM scanning its context window to check the values of 4 different boolean flags, the object holds a single state: `UserAuth at logged_in`. The LLM's attention mechanism only needs to look at **one token** to know exactly what is happening. No cross-referencing, no implicit rules to remember.

```
# Boolean soup — LLM must track 4 variables and their relationships:
let mutable is_loading = true
let mutable is_logged_in = false
let mutable has_error = false
let mutable is_banned = false

# State machine — LLM tracks one value:
let mutable session: UserAuth = UserAuth(guest)
```

**2. Makes invalid code impossible to write.**

The compiler physically prevents the LLM from hallucinating illegal logic. If a transition is not declared, it does not compile. There is no runtime check to forget, no `if` guard to miss. The legal paths are defined once, and the language **mathematically guarantees** they cannot be violated.

**3. State-restricted functions — no defensive checks needed.**

In traditional languages, the LLM must remember to write defensive checks at the start of every function:

```
# Traditional (bad for LLMs — requires remembering to check state):
function post_comment(user: User, text: string) returns result[Comment, string]:
    if user.is_banned:
        return fail("user is banned")
    if not user.is_logged_in:
        return fail("user not logged in")
    # ... actual logic ...
```

The LLM might forget one of these checks. It might check the wrong flag. It might check `is_loading` when it meant `is_logged_in`. Every forgotten check is a bug.

In Jett, the function signature declares which state is required. No checks needed — it is **impossible** to call the function in the wrong state:

```
function post_comment(clock: Clock, session: UserAuth at logged_in, text: string) returns result[Comment, string]:
    # No if-checks needed. This function can ONLY be called when
    # the session is in the "logged_in" state. The compiler enforces this
    # at every call site. The LLM cannot forget. The human cannot forget.
    let comment = Comment(author: session.user_id, text: text, created: Clock.now(clock))
    return ok(comment)
```

If the LLM tries to call `post_comment` with a session in the wrong state:

```
let session = UserAuth(guest)
let result = post_comment(clock, session, "hello")
# COMPILE ERROR: expected "UserAuth at logged_in" but got "UserAuth at guest"
# hint: transition the session to "logged_in" before calling post_comment
```

**4. State machines can carry data per state.**

States are not just labels — they can hold data that is only available in that state:

```
machine OrderProcess:
    states:
        draft(items: list[Item])
        submitted(items: list[Item], submitted_at: time.Timestamp)
        shipped(tracking: string, shipped_at: time.Timestamp)
        delivered(tracking: string, delivered_at: time.Timestamp)
        cancelled(reason: string)

    transitions:
        draft to submitted
        draft to cancelled
        submitted to shipped
        submitted to cancelled
        shipped to delivered
```

The `tracking` field only exists in the `shipped` and `delivered` states. It is impossible to access `tracking` on a `draft` order — the type system will not allow it. The LLM never has to write "check if tracking exists" because the type already guarantees it.

```
function get_tracking(order: OrderProcess at shipped) returns string:
    # order.tracking is guaranteed to exist — we are in the "shipped" state.
    return order.tracking

function ship_order(clock: Clock, order: OrderProcess at submitted, tracking: string) returns OrderProcess at shipped:
    return OrderProcess.transition(order, shipped, tracking: tracking, shipped_at: Clock.now(clock))
```

**5. The LLM defines reality once, then the compiler enforces it forever.**

The state machine declaration at the top of the file is a complete specification of the system's lifecycle. The LLM writes it once — listing every state and every legal transition. For the rest of the file (and every other file that uses this machine), the compiler guarantees that:

- Only declared states exist.
- Only declared transitions occur.
- Functions only run in the states their signatures require.
- State-specific data is only accessible in the correct state.

The LLM does not need to "remember" any of this. It defined the rules, and the language enforces them mechanically. The LLM is free to focus its attention on the logic of each individual function, knowing the machine definition prevents systemic errors.

#### Complex Example: Payment Processing

```
machine Payment:
    states:
        pending(amount: float, currency: string)
        authorized(amount: float, auth_code: string)
        captured(amount: float, auth_code: string, capture_id: string)
        refunded(original_amount: float, refund_id: string)
        failed(reason: string)

    transitions:
        pending to authorized
        pending to failed
        authorized to captured
        authorized to failed
        captured to refunded

enum PaymentOutcome:
    authorized(payment: Payment at authorized)
    declined(payment: Payment at failed)

enum CaptureOutcome:
    captured(payment: Payment at captured)
    declined(payment: Payment at failed)

function authorize_payment(net: Network, pay: Payment at pending) returns result[PaymentOutcome, string]:
    use payment_gateway
    let auth = payment_gateway.authorize(net, pay.amount, pay.currency) handle error:
        return fail("gateway error")
    if auth.declined:
        return ok(PaymentOutcome.declined(payment: Payment.transition(pay, failed, reason: auth.reason)))
    return ok(PaymentOutcome.authorized(payment: Payment.transition(pay, authorized, amount: pay.amount, auth_code: auth.code)))

function capture_payment(net: Network, pay: Payment at authorized) returns result[CaptureOutcome, string]:
    use payment_gateway
    let capture = payment_gateway.capture(net, pay.auth_code, pay.amount) handle error:
        return fail("capture failed")
    if capture.declined:
        return ok(CaptureOutcome.declined(payment: Payment.transition(pay, failed, reason: capture.reason)))
    return ok(CaptureOutcome.captured(payment: Payment.transition(pay, captured,
        amount: pay.amount,
        auth_code: pay.auth_code,
        capture_id: capture.id)))

function refund_payment(net: Network, pay: Payment at captured) returns result[Payment at refunded, string]:
    use payment_gateway
    let refund = payment_gateway.refund(net, pay.capture_id, pay.amount) handle error:
        return fail("refund failed: {error}")
    return ok(Payment.transition(pay, refunded,
        original_amount: pay.amount,
        refund_id: refund.id))
```

Every function operates on a payment in a specific state and transitions it to the next state. The compiler ensures that `capture_payment` can only be called on an `authorized` payment, and `refund_payment` can only be called on a `captured` payment. The LLM cannot accidentally refund a pending payment or capture an already-refunded payment. The state machine makes the illegal states unrepresentable.

### Rule Set 10: Native Performance Without LLM-Hostile Complexity

To achieve C/Zig/Rust-level execution speed while keeping the language optimized for auto-regressive generation, Jett completely rethinks how memory, concurrency, and meta-programming work. The traditional tools for high performance — manual `malloc`/`free`, pointer arithmetic, mutex locks, macro systems — all require long-term memory spanning thousands of lines. LLMs hallucinate memory leaks, forget to unlock mutexes, and get confused by complex pointer arithmetic because tracking those things exceeds their attention capacity.

Jett's approach: **offload everything that requires long-term memory onto the structural rules of the syntax itself.** The compiler manages what the LLM cannot.

#### 1. Memory Management: Linear Typing + Scope-Bound Arenas

Garbage collection is too slow for native-speed code. C-style manual memory (`malloc`/`free`) causes LLMs to hallucinate use-after-free bugs. Rust-style lifetimes (`&'a mut T`) introduce heavy syntactic noise that splits the LLM's attention. Jett uses two complementary mechanisms that give the compiler perfect knowledge of when every value dies, with zero hidden pointers.

**Linear typing — consume by default:**

When a variable is passed into a function, it is **consumed** (moved) and immediately becomes invalid in the current scope. If the LLM tries to use it again on the next line, the compiler rejects it. If the LLM wants to keep it, it must explicitly clone.

```
function send_message(net: Network, connection: Connection, payload: Payload) returns nothing:
    Network.send(net, connection, payload)
    # `payload` has been consumed by `send`. It no longer exists here.
    return

function example(net: Network, stdout: Stdout) returns nothing:
    let conn = Connection("localhost", 8080)
    let data = Payload("hello")

    send_message(net, conn, data)

    Stdout.write(stdout, data.content)
    # COMPILE ERROR: "data" was consumed by "send_message" on the previous line
    # hint: use Linear.clone(data) if you need to keep a copy

    Stdout.write(stdout, conn.status)
    # COMPILE ERROR: "conn" was consumed by "send_message"
```

**Why this works for LLMs:**

The rule is completely local. The LLM does not need to track lifetimes across functions or files. It only needs to know one thing: **after you pass a variable to a function, it is gone.** This is a single, simple rule that applies uniformly everywhere. The compiler enforces it mechanically — no long-term memory required.

**When the LLM needs to keep a value:**

```
function example(net: Network, stdout: Stdout) returns nothing:
    let conn = Connection(host: "localhost", port: 8080)
    let data = Payload("hello")

    send_message(net, conn, Linear.clone(data))
    # `Linear.clone(data)` creates a copy that gets consumed. The original `data` survives.

    Stdout.write(stdout, data.content)   # valid — `data` was never consumed
```

The `Linear.clone()` call is explicit and visible. The LLM (and any reader) can see exactly where copies are made. There is no hidden reference counting or invisible borrowing.

`Linear.clone(view_value)` creates an owned deep copy from a viewed value. This is a common pattern — you often want to make a mutable copy of borrowed data.

**Auto-view for field access:**

Field access on a value implicitly creates a view of the parent. `self.x` is equivalent to `(view self).x`. Accessing `user.name` does NOT consume `user` — it reads the field through an implicit view. Only passing the entire value to a function (as an owned parameter) consumes it.

This means code like the following is valid:

```
let dx = self.x - other.x
let dy = self.y - other.y
# Neither access consumes `self` or `other` — field access is a view operation.
```

Both `self.x` and `self.y` work because each field access creates an implicit view rather than consuming the struct. Similarly, `dx * dx` is valid because primitive types (`int`, `float`, `bool`, `string`) are implicitly copyable — they are not linear. Linear typing only restricts compound types (structs, lists, maps, etc.) that own heap-allocated resources.

**Rebinding semantics for mutable variables:**

The `mutable` keyword allows a variable name to be rebound after its previous value is consumed. This is not mutation — it is consume-and-rebind:

```
let mutable total = 0
for item in items:
    total = total + item
    # The old `total` is consumed by `+`, the result is rebound to `total`.
    # This is safe because linear typing ensures the old value is used exactly once.
return total
```

Without `mutable`, the compiler would reject `total = total + item` because `total` would be consumed on the right side and then the assignment would try to use the now-invalid name. The `mutable` keyword tells the compiler: "this name can be rebound after its value is consumed."

**For loop iteration semantics:**

- `for item in items:` — consumes `items`. Each `item` is owned and can be moved or used once.
- `for item in view items:` — borrows `items` as a view. Each `item` is a view element. `items` remains owned after the loop.

```
# Owned iteration (items consumed):
let mutable sum = 0
for item in items:
    sum = sum + item.price
# items is no longer available here

# View iteration (items preserved):
let mutable sum = 0
for item in view items:
    sum = sum + item.price
# items is still available here
submit_order(items)
```

**Scope-bound arenas — bulk memory management:**

Instead of managing individual allocations, memory is allocated into an **arena** — a block of memory that is freed all at once when the scope ends. The LLM defines an arena at the start of a function. All allocations go into it. When the function returns, the arena drops everything. The LLM never has to remember to call `free()` on line 90 for a variable it created on line 10.

```
function process_batch(records: list[Record]) returns Summary:
    let pool = arena()

    # All allocations in this function use `pool`.
    let mutable parsed = pool.allocate(list[ParsedRecord])
    for record in records:
        let parsed_record = pool.allocate(parse(record))
        parsed = list.append(parsed, parsed_record)

    let summary = compute_summary(parsed)

    return summary
    # `pool` is dropped here. All memory allocated through it is freed instantly.
    # No individual free() calls. No memory leaks possible.
```

**Why arenas are LLM-friendly:**

- **One line to set up, zero lines to tear down.** The LLM writes `let pool = arena()` and never thinks about memory again.
- **No dangling pointers.** Everything allocated in the arena dies together. There is no "free this but not that" decision tree.
- **Bulk efficiency.** Arenas are how high-performance systems (game engines, compilers) manage memory in practice. One deallocation per scope, not one per object.

#### 2. High-Performance Data Layout: Native Structure of Arrays (SoA)

Native performance relies heavily on CPU cache hits and SIMD operations. Data that is laid out contiguously in memory (all X coordinates together, all Y coordinates together) is dramatically faster to process than data interleaved in objects (X1, Y1, velocity1, X2, Y2, velocity2...).

LLMs naturally write "Array of Structs" (AoS) because it groups related concepts cleanly — which is good for the LLM's attention mechanism. Jett lets the LLM write clean, grouped structs but gives the compiler a hint to **transform the memory layout** for performance.

**The LLM writes this (clean, readable, AoS):**

```
struct Particle layout soa:
    x: float
    y: float
    velocity_x: float
    velocity_y: float
    mass: float
```

**The compiler generates this (fast, cache-friendly, SoA):**

Under the hood, `list[Particle]` is not stored as an array of Particle objects. It is stored as:
- One contiguous array of all `x` values
- One contiguous array of all `y` values
- One contiguous array of all `velocity_x` values
- One contiguous array of all `velocity_y` values
- One contiguous array of all `mass` values

**The LLM code looks the same either way:**

```
function update_positions(particles: list[Particle], dt: float) returns list[Particle]:
    return list.map(particles, function(p: Particle) returns Particle:
        return Particle(
            x: p.x + p.velocity_x * dt,
            y: p.y + p.velocity_y * dt,
            velocity_x: p.velocity_x,
            velocity_y: p.velocity_y,
            mass: p.mass))
```

The LLM writes simple, field-access-based code. The compiler handles the memory transformation. The LLM's attention stays focused on the logic. The CPU gets cache-friendly data.

**Why this matters:**

- The LLM writes exactly the code it would write without `layout soa` — zero cognitive overhead.
- The `layout soa` tag is a single annotation. The LLM either includes it or doesn't. No complex template meta-programming.
- The performance difference can be 5-10x for data-heavy workloads (physics, graphics, data processing).

#### 3. Multithreading: Strict Actor Model (Zero Shared Memory)

Shared mutable state — multiple threads modifying the same variable using locks or mutexes — is a disaster for LLMs. An LLM will reliably lock a mutex on line 5 and completely forget to unlock it on line 50 when an early return happens, causing a fatal deadlock.

Jett eliminates shared memory entirely. **Threads physically cannot see each other's memory.** Communication happens exclusively through message passing.

**The actor model:**

```
actor Counter(stdout: Stdout):
    let mutable count: int = 0

    receive increment:
        count = count + 1

    receive get_count responds int:
        respond count

    receive print_count:
        Stdout.write(stdout, string(count))

function main(stdout: Stdout) returns nothing:
    let counter = spawn Counter(Linear.clone(stdout))

    send counter increment
    send counter increment
    send counter increment

    let total = ask counter get_count
    Stdout.write(stdout, string(total))   # prints "3"
```

**Rules enforced by the compiler:**

- An `actor` has private state that **no other code can access directly**. There is no `counter.count` from outside. State is only modified through received messages.
- `send` delivers a message asynchronously. The sender does not wait.
- `ask` delivers a message and waits for a response. Used when the sender needs a value back.
- Because variables are linear (Rule Set 10.1), when a value is sent to an actor, it is consumed in the sender's scope. **No two threads can ever hold the same mutable data.** Race conditions are structurally impossible.

**How actors receive capabilities:**

Actors receive capabilities at spawn time. The capability is moved (or cloned) into the actor and becomes part of the actor's private state. The actor can then use the capability in its receive handlers without threading it through messages.

```
actor Logger(stdout: Stdout):
    receive log(message: string):
        Stdout.write(stdout, message)

function main(stdout: Stdout) returns nothing:
    let logger = spawn Logger(Linear.clone(stdout))
    send logger log("application started")
    # stdout is still available here because we cloned it
```

**Capability cloning for actors:**

Since capabilities are linear types, passing a capability to `spawn` would consume it. To share a capability between the main function and one or more actors, use `Linear.clone()`:

- `Linear.clone(stdout)` creates a second Stdout capability. Both the original and clone can write to stdout independently.
- `Linear.clone(fs)` creates a second Filesystem capability. Both can read/write files.
- Cloning is explicit — the programmer (or LLM) consciously decides which capabilities to share.
- The runtime serializes concurrent access to the same underlying resource (e.g., two Stdout clones writing to the same terminal are serialized to avoid garbled output).

**Sending data between actors:**

```
actor Processor:
    receive process(data: Payload) responds ProcessResult:
        let process_result = heavy_computation(data)
        respond process_result

function main(stdout: Stdout) returns nothing:
    let worker = spawn Processor()
    let data = Payload("input data")

    let response = ask worker process(data)
    # `data` has been consumed — it was sent to the actor.
    # The LLM cannot accidentally access it here.
    Stdout.write(stdout, response.summary)
```

**Why this works for LLMs:**

- **No locks, no mutexes, no semaphores.** The LLM never has to "remember" to unlock something.
- **No shared mutable state.** Each actor owns its state exclusively. The LLM reasons about one actor at a time — completely local.
- **Linear typing prevents data races.** When data is sent, it is gone from the sender. Two threads cannot hold the same data.
- **Message passing is explicit.** The LLM can see exactly what data flows where. No hidden side channels.

#### 4. Async/Await: Enforced Structured Concurrency

In languages like JavaScript or C#, you can spawn a background async task and forget about it. For an LLM, these "fire-and-forget" patterns create invisible ghost processes that it loses track of.

Jett uses **structured concurrency**: all async tasks are bound to a scope. The scope cannot exit until all child tasks are resolved.

```
function fetch_all_data(net: Network) returns result[DashboardData, HttpError]:
    let data = concurrent:
        let users = spawn http.get(net, "https://api.example.com/users")
        let orders = spawn http.get(net, "https://api.example.com/orders")
        let stats = spawn http.get(net, "https://api.example.com/stats")

        # All three requests run in parallel.
        # The `concurrent` block CANNOT exit until all three are resolved.

        let users_result = join users handle error:
            return fail(error)
        let orders_result = join orders handle error:
            return fail(error)
        let stats_result = join stats handle error:
            return fail(error)

        let users_data = json.parse(users_result.body, list[User]) handle error:
            return fail(HttpError.status_error(0, error))
        let orders_data = json.parse(orders_result.body, list[Order]) handle error:
            return fail(HttpError.status_error(0, error))
        let stats_data = json.parse(stats_result.body, Stats) handle error:
            return fail(HttpError.status_error(0, error))

        DashboardData(
            users: users_data,
            orders: orders_data,
            stats: stats_data
        )

    return ok(data)
```

**Rules enforced by the compiler:**

- `spawn` creates a concurrent task or an actor. Inside a `concurrent` block, it spawns a task that must be joined or cancelled. Outside a `concurrent` block, it spawns an actor (see Rule Set 10.3).
- `join` waits for a spawned task to complete. It returns a `result` that must be handled.
- The `concurrent` block **cannot exit** until every spawned task is either `join`ed or explicitly `cancel`led. If the LLM forgets to join a task, the compiler rejects the code.
- No orphaned tasks. No background processes silently running after the scope ends.

**What the compiler rejects:**

```
function bad_example(net: Network) returns result[string, string]:
    concurrent:
        let users = spawn http.get(net, "https://api.example.com/users")
        let orders = spawn http.get(net, "https://api.example.com/orders")

        let users_result = join users handle error:
            return fail("failed")

        # COMPILE ERROR: spawned task "orders" is never joined or cancelled
        # hint: add "join orders" or "cancel orders" before the concurrent block exits
```

**Why this works for LLMs:**

- **Task lifecycles are bound to indentation blocks.** The LLM's attention mechanism understands blocks and indentation. If it spawns a task inside a `concurrent` block, it must resolve that task within the same visual chunk of code.
- **No invisible background processes.** Every concurrent task has a visible `spawn`, a visible `join` or `cancel`, and both live in the same block.
- **The compiler catches forgotten tasks.** The LLM cannot "fire and forget."

#### 5. Meta-Programming: Comptime Over Macros

High-performance languages need meta-programming to optimize code at compile time. C++ uses templates. Rust uses macros. Both introduce **entirely new secondary syntaxes** that behave differently from the main language. This confuses LLM token probabilities — the model has to learn two different ways to write logic.

Jett borrows from Zig: there are **no macros**. Instead, there is a `comptime` keyword that marks normal Jett code to be executed at compile time.

**One syntax for everything:**

```
comptime function generate_lookup_table(size: int) returns list[int]:
    let mutable table = list[int]()
    let mutable i = 0
    while i < size:
        table = list.append(table, i * i)
        i = i + 1
    return table

# This runs at compile time. The result is baked into the binary.
let squares = comptime generate_lookup_table(256)
```

The LLM writes a normal function — same syntax, same rules, same keywords. The `comptime` keyword simply tells the compiler to run it during compilation rather than at runtime. The LLM does not need to learn a separate template language, a macro syntax, or a preprocessor. **One syntax, two execution times.**

**Comptime for generic specialization:**

```
comptime function type_name[T]() returns string:
    return T.name

comptime function is_numeric[T]() returns bool:
    return T is int or T is float

function print_value[T](stdout: Stdout, val: T) returns nothing:
    if comptime is_numeric[T]():
        Stdout.write(stdout, "number: {val}")
    else:
        Stdout.write(stdout, "value: {val}")
```

The `if comptime` branch is resolved at compile time. The compiled binary only contains the branch that applies. This gives the same power as C++ template specialization or Rust trait bounds, but the LLM writes it as a normal `if` statement.

**Why this works for LLMs:**

- **Zero new syntax to learn.** `comptime` functions use the same `function`, `if`, `for`, `while`, `return` keywords as runtime code.
- **No macro hygiene problems.** There are no text-substitution macros that can break scope rules or introduce invisible bugs.
- **Predictable behavior.** The LLM can reason about a `comptime` function exactly like a runtime function — because it *is* one, just executed earlier.

#### Summary: The Native Performance Contract

| Concern | Traditional approach (LLM-hostile) | Jett approach (LLM-friendly) |
|---------|-----------------------------------|------------------------------|
| Memory allocation | `malloc`/`free` (forget to free = leak) | Arenas (bulk free on scope exit) |
| Ownership tracking | Lifetimes with syntactic annotations (`&'a mut T`) | Linear types (consumed on use, clone to keep) |
| Data layout | Manual SoA transformations | `layout soa` annotation, compiler transforms |
| Thread safety | Mutexes and locks (forget to unlock = deadlock) | Actor model, zero shared memory, message passing |
| Concurrency | Fire-and-forget async (orphaned tasks) | Structured concurrency (compiler forces join/cancel) |
| Meta-programming | Macros / templates (secondary syntax) | `comptime` (same syntax, executed at compile time) |

The underlying philosophy: **isolate every responsibility that requires long-term memory onto the compiler.** Memory is managed in bulk (arenas) so the LLM doesn't micromanage pointers. Threads are mathematically isolated (actors) so the LLM doesn't manage locks. Concurrency is physically bound to indentation blocks so tasks can't escape the context window. Meta-programming uses the same syntax so the LLM doesn't learn two languages.

### Rule Set 11: Token-Optimized Syntax (Strict Semantic Whitespace)

Every token costs money, increases latency, and consumes context window. In C-style languages, braces `{}`, parentheses `()`, and semicolons `;` create enormous amounts of syntactic noise. Depending on the tokenizer (e.g., OpenAI's Tiktoken), `} else {` can consume up to 3 separate tokens — none of which carry semantic meaning. Over a 1,000-line file, this visual noise wastes thousands of tokens on structural punctuation instead of actual logic.

Jett eliminates this waste entirely through strict semantic whitespace.

#### 1. No Braces, No Semicolons — Indentation Is Structure

Jett uses **strict indentation** (like Python, Nim, or F#) to define scope. There are no braces and no semicolons anywhere in the language. Newlines terminate statements. Indentation levels define blocks.

**Token cost comparison:**

```
// C-style (17 syntactic noise tokens in this snippet):
function add(int a, int b) {
    if (a > 0) {
        return a + b;
    } else {
        return b;
    }
}
```

```
# Jett (0 syntactic noise tokens):
function max(a: int, b: int) returns int:
    if a > b:
        return a
    else:
        return b
```

The Jett version communicates identical logic with **zero tokens spent on braces, semicolons, or wrapping parentheses around conditions.** Every token in the Jett version carries semantic meaning.

**Why tokenizers love whitespace:**

Modern LLM tokenizers (Tiktoken, SentencePiece, etc.) are heavily optimized for leading whitespace. Common indentation patterns map to single tokens:

| Text | Typical token count |
|------|-------------------|
| 4 spaces (one indent level) | 1 token |
| 8 spaces (two indent levels) | 1 token |
| `{` | 1 token |
| `}` | 1 token |
| `;` | 1 token |
| `} else {` | up to 3 tokens |

With indentation, the structural information that braces and semicolons provide is encoded in whitespace tokens that the tokenizer already handles efficiently. The result: **same structural information, fewer tokens.**

**Quantified savings over a typical file:**

A 200-line C-style file might contain:
- ~200 semicolons (200 tokens)
- ~80 opening braces (80 tokens)
- ~80 closing braces (80 tokens)
- ~30 parentheses around `if`/`while` conditions (60 tokens)
- Total: ~420 tokens of pure syntactic noise

The equivalent Jett file: **0 tokens of syntactic noise.** The same 420 tokens are now available for actual code, comments, or — more importantly — fitting more of the program into the LLM's context window.

#### 2. Attention Alignment — Scope Is Visual

LLMs naturally group concepts based on **token proximity**. Tokens that are close together in the sequence receive stronger mutual attention. Strict indentation exploits this by making scope **both visual and mathematical** — the physical structure of the code on screen directly reflects its logical structure.

**How attention heads process indented code:**

```
function process_order(order: Order) returns result[Receipt, string]:
    let validated = validate(order) handle error:
        return fail("invalid order")
    let charged = charge(validated) handle error:
        return fail("payment failed")
    return ok(create_receipt(charged))
```

Every line inside this function is at the same indentation level. The LLM's attention mechanism naturally groups them together because they are physically close and share the same leading whitespace pattern. The function boundary is visually obvious — the next line at indentation level 0 is a different function.

**Contrast with brace-based scoping:**

```
// Brace-based: the LLM must match { to } across potentially hundreds of lines.
// A missing } on line 150 causes an error that manifests on line 300.
// The attention head linking the opening { to its closing } must span the entire function.
```

With indentation, there is **nothing to match**. The scope is defined by the indentation level itself. There is no opening delimiter that needs a closing delimiter 200 lines later. The most common class of syntax errors in brace-based languages — mismatched or missing brackets — is eliminated entirely.

#### 3. Strict Rules — Zero Ambiguity

Jett's whitespace rules are rigid. There is exactly one way to indent code, and any deviation is a compile error.

**Enforced rules:**

| Rule | Enforcement |
|------|-------------|
| Indent unit is exactly 4 spaces | Using tabs or 2 spaces is a compile error |
| Each nested block adds exactly one indent level | Skipping a level (0 to 8 spaces) is a compile error |
| No trailing whitespace | Trailing spaces on any line is a compile error |
| No mixed indentation | Mixing tabs and spaces anywhere is a compile error |
| Blank lines inside blocks are allowed | But they must have zero characters (no invisible whitespace) |
| Maximum indent depth: 4 levels (16 spaces) | Enforced by Rule Set 7 (nesting depth limit) |

**Why strict rules matter:**

Ambiguous whitespace (Python allows both 2 and 4 spaces, tabs, and mixes) creates the same problem as syntactic sugar — multiple ways to express the same structure. LLMs may inconsistently mix indentation styles within a file. Jett eliminates this by enforcing one style absolutely.

```
function example() returns nothing:
    let x = 1          # 4 spaces — valid
      let y = 2        # 6 spaces — COMPILE ERROR: expected 4 or 8 spaces
  let z = 3            # 2 spaces — COMPILE ERROR: indent must be multiple of 4

function another() returns nothing:
	let a = 1           # tab — COMPILE ERROR: tabs are not allowed, use 4 spaces
```

**Colon as block opener:**

Every block-level construct ends with `:` before the indented body. This serves as a clear, single-token signal to both the LLM and the parser that an indented block follows:

```
function ...:
if ...:
else:
for ...:
while ...:
struct ...:
enum ...:
match ...:
machine ...:
actor ...:
concurrent:
verify ...:
property ...:
mutual:
implement ...:
receive ...:
bitfield ...:
```

The colon is the **only** token that signals "the next line will be indented." This is completely predictable. When the LLM generates `:` followed by a newline, it knows to increase the indentation level by exactly 4 spaces. When it returns to the previous indentation level, the block is closed. No closing token needed.

### Rule Set 12: Opaque, Iterator-Only String Manipulation

This addresses a fundamental and often overlooked mismatch between how LLMs perceive text and how programming languages represent it.

#### The Problem: LLMs Cannot Count Bytes

LLMs do not see individual characters or bytes. They see **tokens** — variable-length chunks of text determined by the tokenizer. The word `apple` is one token. The character `語` might be 3 separate byte tokens. The emoji `🎉` might be 4 bytes.

When a language allows raw byte or integer indexing into strings (e.g., `string[7]`, `string.charAt(3)`, `&string[2..5]`), it assumes the programmer knows exactly which byte offset corresponds to which character. LLMs are **fundamentally incapable of this**. They will hallucinate byte offsets, producing code that:

- Slices a multi-byte UTF-8 character in half → runtime panic or segfault
- Returns the wrong character because the LLM miscounted bytes
- Produces off-by-one errors on strings containing any non-ASCII text
- Generates code that works on test strings (`"hello"`) but crashes on real data (`"こんにちは"`)

This is not a matter of training or prompting — the neural architecture itself does not have a byte-counting mechanism. It is a structural limitation.

#### The Solution: Strings Are Opaque

In Jett, the `string` type is an **opaque, high-performance byte array** that cannot be indexed by integer position. There is no `string[5]`. There is no `string.byte_at(3)`. There is no way to slice a string by byte offset. Period.

**What the compiler rejects:**

```
let name = "hello world"
let char = name[5]
# COMPILE ERROR: string does not support integer indexing
# hint: use string.char_at(name, 5) to get the 5th character,
# or use string.split(), string.take_chars(), string.find() for common operations
```

All string manipulation happens through **standard library functions** that operate on characters (Unicode grapheme clusters), never on raw bytes. The LLM never needs to know how many bytes a character occupies.

#### Iterator-Driven String Operations

Every common string task has a dedicated function that handles encoding, boundaries, and edge cases internally.

**Extracting parts of strings — by meaning, not by offset:**

```
use string

# Take the first N characters (not bytes):
let first_five = string.take_chars("こんにちは世界", 5)
# Result: "こんにちは" — correct regardless of byte width

# Take the last N characters:
let last_three = string.take_last_chars("hello world", 3)
# Result: "rld"

# Drop the first N characters:
let rest = string.drop_chars("hello world", 6)
# Result: "world"

# Get a character by position (returns optional, not a raw byte):
let third = string.char_at("hello", 2)
# Result: optional containing "l"

# Character count (not byte count):
let len = string.char_count("こんにちは")
# Result: 5 (not 15, which would be the byte count)
```

**Searching and splitting — the primary way to work with strings:**

```
use string

# Find a substring:
let position = string.find("hello world", "world")
# Result: optional containing a string iterator position (not a byte offset)

# Check containment:
let has_at = string.contains(email, "@")

# Split into parts:
let words = string.split("hello world foo", " ")
# Result: list["hello", "world", "foo"]

# Split with limit:
let parts = string.split_max("a.b.c.d", ".", 2)
# Result: list["a", "b.c.d"]

# Get text between delimiters:
let title = string.between(html, "<title>", "</title>")

# Get text before/after a delimiter:
let domain = string.after(email, "@")
let username = string.before(email, "@")
```

**Transforming strings — no manual character loops:**

```
use string

let upper = string.upper("hello")              # "HELLO"
let lower = string.lower("Hello")              # "hello"
let trimmed = string.trim("  hello  ")         # "hello"
let trim_left = string.trim_start("  hello  ") # "hello  "
let replaced = string.replace("hello", "l", "r") # "herro"
let reversed = string.reverse("hello")         # "olleh"
let repeated = string.repeat("ha", 3)          # "hahaha"
let padded = string.pad_left("42", 5, "0")     # "00042"
let slug = string.slugify("Hello World!")       # "hello-world"
let truncated = string.truncate("long text here", 8, "...") # "long tex..."
```

**Iterating over characters — when a loop is genuinely needed:**

```
use string

for char in string.chars("hello"):
    Stdout.write(stdout, char)
    # Yields: "h", "e", "l", "l", "o"
    # Each `char` is a single Unicode grapheme cluster, not a byte.

for word in string.words("the quick brown fox"):
    Stdout.write(stdout, word)
    # Yields: "the", "quick", "brown", "fox"

for line in string.lines(multiline_text):
    process(line)
```

The `string.chars()` iterator yields **grapheme clusters** — what a human would call "a character." The emoji `👨‍👩‍👧‍👦` (a family emoji composed of multiple Unicode code points joined by zero-width joiners) is yielded as a single element, not as 7 separate code points. The LLM never has to know about code points, surrogate pairs, or combining characters.

#### Why This Matters for LLMs

**1. Eliminates an entire class of hallucinations.**

The LLM cannot generate `string[7]` because the syntax doesn't exist. It cannot produce a wrong byte offset because byte offsets are not exposed. The bug class of "sliced a UTF-8 character in half" is structurally impossible.

**2. Every operation is semantic, not positional.**

`string.split(csv, ",")` expresses intent: "separate this string at commas." The LLM doesn't need to know that commas are 1 byte in ASCII but that the fields between them might contain multi-byte characters. The standard library handles it.

**3. LLM-generated string code works on all human languages.**

Because the API operates on grapheme clusters, not bytes, code that works on `"hello"` also works on `"こんにちは"`, `"مرحبا"`, and `"🎉🎊🎈"`. The LLM doesn't need to special-case Unicode — the language handles it uniformly.

**4. The API surface matches how LLMs think about text.**

LLMs think about text in terms of "words", "lines", "the part before the @", "the first 5 characters." These are exactly the operations Jett's string API provides. The API matches the LLM's natural abstraction level, not the machine's byte-level representation.

**Comparison with traditional string handling:**

| Task | C / Rust (LLM-hostile) | Jett (LLM-friendly) |
|------|----------------------|-------------------|
| Get first 5 characters | Manual byte counting, UTF-8 boundary checking | `string.take_chars(s, 5)` |
| Find a substring | Returns byte offset, manual boundary handling | `string.find(s, "target")` returns iterator position |
| Split by delimiter | Returns byte slices, must handle empty cases | `string.split(s, ",")` returns `list[string]` |
| Reverse a string | Must reverse by grapheme clusters, not bytes | `string.reverse(s)` |
| Get string length | `.len()` returns bytes, `.chars().count()` returns code points, neither returns graphemes | `string.char_count(s)` returns grapheme count |
| Index into string | `s[5]` may panic on multi-byte character | Not possible — compile error |

### Rule Set 13: Inline, Contract-Based Testing (Context Preservation)

#### The Problem: Split Context Kills Test Quality

In standard development, code lives in `src/auth.jett` and tests live in `tests/test_auth.jett`. For an LLM, this split is devastating. When the LLM opens the test file, the function implementation is no longer in its immediate context. It must either:

1. Hold the entire implementation in memory from a previous read (attention degrades with distance), or
2. Work with only the function signature (insufficient for thorough tests), or
3. Have both files in context simultaneously (wastes half the context window on non-test code).

All three options produce worse tests. The LLM forgets edge cases it saw in the implementation. It hallucinates parameter names. It writes tests that check the wrong behavior because it can't see the code it's testing.

#### The Solution: Tests Live Next to the Code

Jett borrows from Zig's approach: **tests are written directly below the function they verify**, in the same file, in the same visual block. The LLM writes the function, and immediately writes the tests while the implementation is at the absolute peak of its attention window.

#### 1. Co-located `verify` Blocks

Every function can have a `verify` block immediately after its definition. The `verify` block contains test cases that are **contracts** — they are not optional quality checks, they are compiler-enforced proofs that the function behaves correctly.

**Basic example:**

```
function calculate_discount(price: float, tier: string) returns float:
    if tier is "gold":
        return price * 0.8
    else if tier is "silver":
        return price * 0.9
    else:
        return price

verify calculate_discount:
    assert calculate_discount(100.0, "gold") is 80.0
    assert calculate_discount(100.0, "silver") is 90.0
    assert calculate_discount(100.0, "bronze") is 100.0
    assert calculate_discount(0.0, "gold") is 0.0
    assert calculate_discount(50.0, "silver") is 45.0
```

The `verify` block is attached to `calculate_discount` by name. It appears directly below the function — zero distance between implementation and tests. When the LLM generates the `verify` block, it just wrote the function body. Every branch, every edge case, every constant is fresh in its context.

**Why `verify` and not `test`:**

The word `test` implies something optional — something you run separately, maybe in CI, maybe later. `verify` implies a contract — the compiler will not accept this function unless the verification passes. The naming reinforces the semantics.

#### 2. Compiler-Enforced Contracts (Comptime Verification)

`verify` blocks are not regular tests that run at runtime. They are executed by the **comptime engine** (Rule Set 10.5) during compilation. If any assertion in a `verify` block fails, the program **does not compile**.

**What happens during compilation:**

```
function add_positive(a: int, b: int) returns int:
    return a + b

verify add_positive:
    assert add_positive(2, 3) is 5       # passes at compile time
    assert add_positive(0, 0) is 0       # passes at compile time
    assert add_positive(-1, 1) is 0      # passes at compile time
    assert add_positive(1, 1) is 3       # COMPILE ERROR:
    # verify add_positive failed:
    #   assert add_positive(1, 1) is 3
    #   left:  2
    #   right: 3
    #   hint: the function returned 2 but the assertion expected 3
```

The binary is **never emitted** if a `verify` block fails. This means:

- Every function with a `verify` block is **proven correct** (for the tested inputs) before the program ever runs.
- Bugs are caught at compile time, not at runtime, not in CI, not in production.
- The LLM gets immediate feedback: write function → write verify → compile → if verify fails, fix → compile again.

**Comptime verification limitations:**

`verify` blocks can only call **pure functions** (no capability parameters). This is enforced by the compiler. A function that takes a `Filesystem` or `Network` capability cannot be verified at compile time because it would require actual I/O during compilation. For impure functions, Jett provides `property` blocks that run during `jett test` (see Rule Set 25).

#### 3. The Full Pattern: Function → Verify → Next Function

The idiomatic Jett file follows a strict rhythm: define, verify, define, verify. Each function and its proof live together as a unit.

```
function celsius_to_fahrenheit(c: float) returns float:
    return c * 1.8 + 32.0

verify celsius_to_fahrenheit:
    assert celsius_to_fahrenheit(0.0) is 32.0
    assert celsius_to_fahrenheit(100.0) is 212.0
    assert celsius_to_fahrenheit(-40.0) is -40.0

function fahrenheit_to_celsius(f: float) returns float:
    return (f - 32.0) / 1.8

verify fahrenheit_to_celsius:
    assert fahrenheit_to_celsius(32.0) is 0.0
    assert fahrenheit_to_celsius(212.0) is 100.0
    assert fahrenheit_to_celsius(-40.0) is -40.0

function is_boiling(c: float) returns bool:
    return c >= 100.0

verify is_boiling:
    assert is_boiling(100.0) is true
    assert is_boiling(99.9) is false
    assert is_boiling(200.0) is true
```

Each function is immediately followed by its contract. When the LLM generates `celsius_to_fahrenheit`, it writes the verify block while the formula `c * 1.8 + 32.0` is still the most recent thing in its context. By the time it moves on to `fahrenheit_to_celsius`, the previous function is fully verified and can be trusted.

#### 4. Verify Blocks and Refinement Types

`verify` blocks work with refinement types (Rule Set 3) to create a powerful proof chain:

```
type Percentage = float where value >= 0.0 and value <= 100.0

function calculate_grade(score: int, total: int) returns Percentage:
    return float(score) / float(total) * 100.0

verify calculate_grade:
    assert calculate_grade(85, 100) is 85.0
    assert calculate_grade(0, 100) is 0.0
    assert calculate_grade(50, 50) is 100.0
    assert calculate_grade(1, 3) is_near 33.33 within 0.01
```

The return type `Percentage` guarantees the result is between 0 and 100. The `verify` block proves specific input/output pairs. Together, the type system and the verification contracts provide two layers of correctness: the type constrains the range, the verify proves specific behaviors.

**Float comparison with `is_near`:**

LLMs cannot reliably predict exact IEEE 754 floating point representations (e.g., `33.333333333333336`). For float comparisons, Jett provides `is_near ... within ...` syntax:

```
assert calculate_grade(1, 3) is_near 33.33 within 0.01
# Passes if the result is within 0.01 of 33.33
```

- `is` — exact comparison. Use for `int`, `string`, `bool`, and exact float values like `0.0` or `100.0`.
- `is_near X within Y` — approximate comparison. Use for float results that involve division or irrational numbers. The tolerance `Y` is mandatory — there is no implicit epsilon.

#### Why This Matters for LLMs

**1. Zero context distance between code and tests.**

The LLM writes the function body, then immediately writes the verify block. The implementation is literally the previous few lines — maximum attention, maximum accuracy. No file switching, no context splitting.

**2. Compile-time feedback loop.**

The LLM generates code → the compiler runs verify blocks → if they fail, the LLM gets a precise error ("expected 3, got 2 on line 15") → the LLM fixes the code. This loop happens at compile time, not at test-runner time. Faster feedback means fewer wasted tokens.

**3. Tests are contracts, not afterthoughts.**

Because verify blocks are compiler-enforced, the LLM cannot "skip" testing. Every pure function is proven correct before the binary exists. This matches the LLM workflow perfectly — generate function, generate proof, move on.

**4. The LLM never writes a test for code it can't see.**

In traditional testing, the LLM might be asked to "write tests for the auth module" — a vague, context-heavy task. In Jett, the LLM always writes verify blocks for the function it just defined. The task is always local, always immediate, always fully in context.

### Rule Set 14: Anti-Hallucination Dependency Management

#### The Problem: LLMs Hallucinate Libraries

One of the most frequent and dangerous LLM code generation failures is **inventing third-party libraries that do not exist**. An LLM asked to parse JSON might confidently generate `use super_fast_json` or `import imaginary_auth_lib`. The library name sounds plausible. The API the LLM generates for it looks reasonable. But the package does not exist.

This happens because:

1. **Package registries are black boxes.** npm has 2+ million packages. PyPI has 500k+. Unless a specific package was prominent in training data, the LLM is guessing.
2. **Short names are guessable.** Registry names like `fast-json`, `auth-helper`, `string-utils` follow predictable patterns. The LLM's probability distribution happily generates plausible-sounding names that may or may not map to real packages.
3. **No verification at generation time.** The LLM has no way to check whether a package exists while it is generating tokens. It commits to a name and moves on.

The result: code that looks correct, passes a casual review, but fails on the first `jett build` because the dependency doesn't exist — or worse, it exists but is a completely different library than what the LLM assumed.

#### The Solution: Content-Addressable Imports with Cryptographic Hashes

Jett has **no centralized package manager with short, guessable names**. There is no `jett install cool-lib`. External code is imported via an **absolute URL** paired with a **mandatory cryptographic hash**.

**Syntax:**

```
use "https://packages.jett-lang.org/v1.2/json_extra.jett" as json_extra
    hash "sha256:a1b2c3d4e5f6..."
```

Every external import has two components:
1. **A full URL** — not a short name, not a registry identifier, a complete address.
2. **A cryptographic hash** — the SHA-256 of the exact file contents. If the content at the URL changes, the hash won't match and the compiler rejects it.

**What the compiler does:**

1. Fetches the file at the URL.
2. Computes the SHA-256 hash of the downloaded content.
3. Compares it to the declared hash.
4. If they match → import succeeds.
5. If the URL returns 404 → **compile error** ("dependency not found at URL").
6. If the hash doesn't match → **compile error** ("dependency content has changed, expected hash X got hash Y").

**What happens when an LLM hallucinates a dependency:**

```
use "https://packages.jett-lang.org/v3.0/super_fast_auth.jett" as auth
    hash "sha256:fake1234..."

# COMPILE ERROR: dependency not found
#   URL "https://packages.jett-lang.org/v3.0/super_fast_auth.jett" returned 404
#   hint: this dependency does not exist. Use the standard library
#   or provide a valid URL to an existing package.
```

The hallucinated library is caught instantly. The compiler doesn't try to guess what the LLM meant. It doesn't search a registry for close matches. It simply fails with a clear message: this does not exist.

#### Why This Design Eliminates Hallucinations

**1. Zero magic identifiers.**

In npm, `use "left-pad"` works because a registry resolves the short name. The LLM can guess short names. In Jett, `use "https://exact-url/exact-file.jett"` requires the LLM to know the exact URL. LLMs cannot hallucinate valid URLs with correct hashes — the probability of generating a valid SHA-256 hash by chance is effectively zero.

**2. The hash is an unforgeable proof.**

Even if the LLM guesses a real URL, it must also provide the correct SHA-256 hash. The hash cannot be guessed. It must come from actually downloading the file and computing the hash. This means:

- A human developer adds dependencies (they can verify URLs and compute hashes).
- The LLM uses dependencies that are already declared in the project.
- The LLM writes logic itself using the standard library instead of reaching for packages.

This is the **intended behavior**. The massive standard library (Rule Set 8) covers most needs. External dependencies should be rare and deliberately chosen.

**3. Forces the LLM to use what it knows.**

When an LLM encounters a task that would typically require a third-party library, it has two options:

1. Use the Jett standard library (which is in its context/training data).
2. Write the logic itself using Jett primitives.

Both options produce code that actually works. Neither option involves guessing at package names.

#### Dependency Lock File

For projects with external dependencies, the `jett.lock` file records all resolved URLs and their hashes:

```
dependencies:
    json_extra:
        url = "https://packages.jett-lang.org/v1.2/json_extra.jett"
        hash = "sha256:a1b2c3d4e5f6..."
        fetched = "2025-03-15T10:30:00Z"

    websocket:
        url = "https://packages.jett-lang.org/v2.0/websocket.jett"
        hash = "sha256:b2c3d4e5f6a1..."
        fetched = "2025-03-15T10:30:00Z"
```

The lock file is committed to version control. It is the source of truth for reproducible builds. The LLM can **read** the lock file to see which dependencies are available and use them by name within the project:

```
# Inside a project with json_extra in jett.lock:
use json_extra

# The compiler resolves "json_extra" to the URL and hash in jett.lock.
# No guessing. No registry lookup. The dependency is pinned.
```

This gives the LLM a safe path: it reads `jett.lock`, sees what's available, and uses those. It cannot add new dependencies — only a human (or an LLM with explicit instructions and the actual URL + hash) can do that.

#### Adding Dependencies: The Human Workflow

```
# Human adds a dependency (fetches, verifies, computes hash):
jett dep add https://packages.jett-lang.org/v1.2/json_extra.jett

# The CLI:
# 1. Downloads the file
# 2. Computes SHA-256
# 3. Adds the entry to jett.lock
# 4. Prints: added json_extra (sha256:a1b2c3d4e5f6...)
```

The LLM never runs `jett dep add`. It works with what is already in `jett.lock`. This division of responsibility — humans manage dependencies, LLMs write code — plays to each party's strengths.

#### Supply Chain Security: A Free Bonus

Content-addressable imports with hashes provide supply chain security as a side effect:

- **No dependency confusion attacks.** There is no registry namespace to squat. A URL is unique.
- **No silent updates.** If a package author publishes a new version at the same URL, the hash won't match and the build fails. Updates are always explicit.
- **No typosquatting.** There is no short name to misspell. You either have the exact URL or you don't.
- **Reproducible builds.** The lock file pins exact content hashes. Building the same project on two machines produces identical results.

### Rule Set 15: Explicit Data Masking for Security Contexts

#### The Problem: LLMs Leak Secrets by Pattern Matching

LLMs are pattern matchers. When generating backend code — APIs, database queries, logging — they follow patterns seen in training data. And training data is full of code that:

- Logs entire request objects (including `Authorization` headers with bearer tokens)
- Returns full database rows to API responses (including password hashes, SSNs, internal IDs)
- Passes API keys through string formatting into error messages
- Stores secrets in plain-text variables that end up in debug output

The LLM doesn't understand that a password hash is different from a username. Both are strings. Both get passed to functions that accept strings. The LLM generates `Stdout.write(stdout, user)` and the user struct contains `password_hash` and suddenly secrets are in the logs.

This is not a hypothetical — it is one of the most common security vulnerabilities in LLM-generated code. The LLM treats all data uniformly because, at the type level, it *is* all the same: `string`.

#### The Solution: Security Sensitivity at the Type Level

Jett introduces a `secret` type wrapper that **taints** data at the type level. Once a value is marked as secret, the compiler tracks it through every operation and **structurally prevents** it from being passed to any output function — `Stdout.write`, `log`, `http.respond`, `Filesystem.write_file`, or any function that is not explicitly authorized to handle secrets.

**Declaring secret data:**

```
struct User:
    id: string
    name: string
    email: string
    password_hash: secret[string]
    api_key: secret[string]
    ssn: secret[string]
```

The `secret[string]` type is not just a label — it is a distinct type that the compiler enforces differently from `string`. A `secret[string]` **cannot** be used anywhere a `string` is expected.

**What the compiler rejects:**

```
function get_user_debug(user: User) returns string:
    return "user: {user.name} hash: {user.password_hash}"
    # COMPILE ERROR: cannot use secret[string] in string interpolation
    # "password_hash" is marked as secret and cannot be exposed
    # hint: use secret.redact(user.password_hash) to get a masked representation
```

```
function log_user(stdout: Stdout, user: User) returns nothing:
    use log
    Stdout.write(stdout, "user logged in: {json.serialize(user)}")
    # COMPILE ERROR: cannot serialize struct containing secret fields
    # "User" contains secret fields: password_hash, api_key, ssn
    # hint: use json.serialize_public(user) to serialize only non-secret fields
```

```
function handle_login(net: Network, request: Request) returns result[Response, string]:
    use net.http
    let user = authenticate(request) handle error:
        return ok(http.response(400, "invalid credentials"))
    return ok(http.response(200, json.serialize(user)))
    # COMPILE ERROR: cannot pass struct containing secret fields to http.response
    # hint: create a public view of User without secret fields
```

The compiler catches every path where a secret value could reach an output boundary. The LLM is **physically blocked** from generating code that leaks secrets.

#### How Secret Tainting Works

**1. Taint propagation — secrets are contagious.**

Any operation on a secret value produces another secret value. The taint cannot be washed off by accident:

```
let key: secret[string] = load_api_key()

let upper_key = string.upper(key)
# upper_key is secret[string] — the taint propagates through string operations

let combined = string.join(list("prefix", key, "suffix"), "-")
# COMPILE ERROR: cannot pass secret[string] to string.join with non-secret arguments
# hint: the result would leak the secret value
```

**2. Explicit declassification — the only way to unwrap a secret.**

When code genuinely needs to use a secret value (e.g., to send it in an authentication header, to compare against a hash), it must use the `declassify` keyword. This is a deliberate, auditable action:

```
function authenticate(stored_hash: secret[string], input_password: string) returns bool:
    let input_hash = crypto.sha256(input_password)
    return declassify stored_hash is input_hash
    # `declassify` explicitly unwraps the secret for this comparison.
    # This is auditable — grep for "declassify" to find every place secrets are accessed.
```

```
function call_external_api(net: Network, api_key: secret[string], payload: string) returns result[Response, string]:
    use net.http
    let headers = map("Authorization": "Bearer {declassify api_key}")
    return http.post(net, "https://api.example.com/data", payload, headers: headers)
```

Every use of `declassify` is a **visible, searchable marker** in the codebase. A security audit can grep for `declassify` and review every place where secrets are accessed. If an LLM generates `declassify`, it is making an explicit choice that a reviewer can catch.

**3. Safe alternatives for common operations.**

The standard library provides functions that work with secret-containing types safely:

```
# Serialize only non-secret fields:
let public_json = json.serialize_public(user)
# Result: {"id": "123", "name": "alice", "email": "alice@example.com"}
# password_hash, api_key, ssn are omitted automatically.

# Redact for logging:
let masked = secret.redact(user.api_key)
# Result: "sk-****...****3f2a" (shows only last 4 characters)

let log_safe = secret.redact_all(user)
# Result: User with all secret fields replaced by "[REDACTED]"

# Compare secrets without exposing them:
let match = secret.compare(stored_hash, computed_hash)
# Constant-time comparison that returns bool without declassifying either value.
```

#### Secret Types with Refinement Types

Secret types compose with refinement types (Rule Set 3) for validated, secure data:

```
type ApiKey = secret[string] where string.char_count(value) is 40
type PasswordHash = secret[string] where string.starts_with(value, "$2b$")
type Ssn = secret[string] where string.char_count(value) is 11 and string.char_at(value, 3) is "-"
```

The type system enforces both the security constraint (cannot be leaked) and the format constraint (must match the expected pattern). An `ApiKey` is guaranteed to be exactly 40 characters long AND is guaranteed to never appear in logs, responses, or error messages.

#### Secret Types with State Machines

Secrets integrate with state machines (Rule Set 9) for lifecycle management:

```
machine ApiKeyLifecycle:
    states:
        active(key: secret[string], created: time.Timestamp)
        rotated(old_key: secret[string], new_key: secret[string], rotated_at: time.Timestamp)
        revoked(revoked_at: time.Timestamp)

    transitions:
        active to rotated
        active to revoked
        rotated to active
        rotated to revoked
```

The `key` field only exists in the `active` state. It is `secret[string]`, so even in the active state it cannot be logged or serialized. The revoked state carries no key at all — the secret is structurally gone.

#### Why This Matters for LLMs

**1. The LLM cannot accidentally leak secrets.**

The most common LLM security mistake — `print(user)` where user contains a password — is a compile error. The LLM doesn't need to "remember" which fields are sensitive. The type system remembers.

**2. Security is the default, not an opt-in.**

In traditional languages, everything is public by default and security is added after the fact. In Jett, once a field is `secret[string]`, every path to exposure is blocked by default. The LLM must explicitly `declassify` to access secrets — a visible, auditable action.

**3. `declassify` is a searchable audit point.**

Every place where a secret is unwrapped is marked with the `declassify` keyword. Security reviewers can `grep declassify` across the entire codebase to find every secret access point. This is trivially automatable.

**4. Safe alternatives are easier to use than unsafe ones.**

`json.serialize_public(user)` is fewer tokens and less effort than manually constructing a response without secret fields. The path of least resistance for the LLM is the secure path.

**Summary — the compiler enforces a simple rule:**

```
secret[T] ──→ Stdout.write()  BLOCKED
secret[T] ──→ log()           BLOCKED
secret[T] ──→ http.respond()  BLOCKED
secret[T] ──→ json.serialize() BLOCKED
secret[T] ──→ Filesystem.write_file() BLOCKED
secret[T] ──→ string concat   BLOCKED
secret[T] ──→ declassify ──→  ALLOWED (auditable)
secret[T] ──→ secret.redact() ALLOWED (masked output)
secret[T] ──→ secret.compare() ALLOWED (constant-time comparison)
```

### Rule Set 16: Capability-Based I/O (Zero Hidden Side Effects)

#### The Problem: Side Effects Hide in the Call Stack

In high-performance languages like C++ or Rust, any function can open a file, connect to a network socket, or spawn a process. The function signature says `fn process(data: Vec<u8>) -> Result<Output>` — nothing in the signature reveals that this function writes to disk, sends network packets, or reads environment variables.

For an LLM, this is a severe problem. To know whether calling `process(data)` has side effects, the LLM must read the entire implementation — and the implementation of every function it calls, recursively. This requires holding the entire call stack in the context window. For a function 5 levels deep in a call chain, the LLM would need thousands of lines of context to determine whether a call is pure or effectful.

The result: the LLM **hallucinates side effects** (assumes a function is pure when it isn't) or **hallucinates purity** (adds unnecessary I/O to a function that should be pure) because it cannot see deep enough into the call chain.

Rule Set 2 established that side effects must be visible in the function signature. Rule Set 16 makes this **concrete and enforceable** by requiring **capability objects** to be physically threaded through the program.

#### The Solution: Capability Objects as Function Parameters

Jett completely bans global I/O access. There is no global `Stdout.write()`, no global `file.open()`, no implicit access to the network, file system, or operating system. Instead, I/O operations require a **capability object** — a value that grants permission to perform a specific kind of side effect.

Capability objects are created **only in `main()`** and must be explicitly passed down to every function that needs them.

**The capability types:**

```
# Built-in capability types:
# Filesystem   — read/write files, list directories
# Network      — open connections, send/receive data
# Stdout       — write to standard output
# Stderr       — write to standard error
# Stdin        — read from standard input
# Clock        — read the current time
# Random       — generate random numbers
# Process      — spawn child processes
# Environment  — read environment variables
```

**Capabilities are a closed, built-in set.** Users cannot define custom capability types. Capabilities represent primitive OS-level side effects (file I/O, networking, stdout, etc.) — these are a finite, well-known set. Higher-level abstractions like database access or HTTP clients are built on top of primitive capabilities (e.g., a database module takes a `Network` parameter internally). This keeps the capability system simple: the compiler knows the full set, auto-rebinding and purity tracking are straightforward, and LLMs have a small, fixed list to learn rather than an open-ended set that varies per project. Capability types are not syntactically distinguished from other types in function signatures — they follow the same pass-and-propagate pattern as any other parameter.

**How `main()` receives capabilities:**

```
function main(stdout: Stdout, stderr: Stderr, fs: Filesystem, net: Network, env: Environment) returns nothing:
    let config_path = Environment.get(env, "CONFIG_PATH") handle error:
        Stderr.write(stderr, "CONFIG_PATH not set")
        return

    let config = load_config(fs, config_path) handle error:
        Stderr.write(stderr, "failed to load config")
        return

    run_server(fs, net, stdout, config)
    Stdout.write(stdout, "server stopped")
```

`main()` is the **only** function that receives capabilities from the runtime. Every other function in the program gets its capabilities by having them passed in as parameters. If a function doesn't have a `Filesystem` parameter, it **cannot** touch the file system. Period. The compiler enforces this.

#### Capabilities Are Linear — Threading Makes Side Effects Visible

Because capabilities are linear types (Rule Set 10.1), they are **consumed** when passed to a function. This means:

```
function example(fs: Filesystem, stdout: Stdout) returns nothing:
    let config = read_config(fs, "app.conf") handle error:
        Stdout.write(stdout, "failed")
        return
    # `fs` is auto-rebound because the compiler recognizes Filesystem as a capability type.
    # The compiler handles returning capabilities transparently.
```

**Capability threading — functions borrow and return capabilities:**

```
function read_config(fs: Filesystem, path: string) returns result[Config, string]:
    let raw = Filesystem.read_file(fs, path) handle error:
        return fail("could not read {path}")
    let config = json.parse(raw, Config) handle error:
        return fail("invalid config format")
    return ok(config)
    # The compiler recognizes Filesystem as a capability type and auto-rebinds
    # `fs` after each call, returning it to the caller alongside the result.
```

The compiler recognizes `Filesystem` as a capability type and automatically borrows and returns it. The compiler auto-rebinds the capability in the caller's scope — no manual destructuring needed:

```
function main(stdout: Stdout, fs: Filesystem) returns nothing:
    let config = read_config(fs, "app.conf") handle error:
        Stdout.write(stdout, "failed")
        return
    # `fs` is still available — the compiler auto-rebound it because Filesystem is a capability type.
    let data = read_data(fs, config.data_path) handle error:
        Stdout.write(stdout, "failed")
        return
    process(data)
    Stdout.write(stdout, "done")
```

Every function that touches the filesystem has `fs: Filesystem` in its parameters. Every function that writes output has `stdout: Stdout`. **By reading only the function signature**, the LLM (or a human) knows exactly which side effects a function can perform.

**Auto-rebinding of capability parameters:**

The compiler recognizes capability types (Filesystem, Network, Stdout, Stderr, Stdin, Clock, Random, Process, Environment) and **automatically rebinds** their parameters after each method call. You do NOT need to write `stdout = Stdout.write(stdout, msg)` — just `Stdout.write(stdout, msg)`. The compiler handles rebinding implicitly. No `with` clause is needed — the compiler infers this from the parameter types.

```
function log_message(stdout: Stdout, message: string) returns nothing:
    Stdout.write(stdout, message)
    Stdout.write(stdout, "\n")
    # stdout is still alive — the compiler recognizes Stdout as a capability type and auto-rebinds after each method call
```

On the error path, the compiler automatically returns all borrowed capability parameters:

```
function read_config(fs: Filesystem, path: string) returns result[Config, string]:
    let raw = Filesystem.read_file(fs, path) handle error:
        return fail("could not read {path}")
        # The compiler automatically returns fs alongside the fail value
    let config = json.parse(raw, Config) handle error:
        return fail("invalid config format")
        # The compiler automatically returns fs here too
    return ok(config)
    # The compiler automatically returns fs alongside the ok value
```

**Error path semantics:** Because the compiler recognizes capability types, it ensures that every `return` statement (whether `ok(...)`, `fail(...)`, or bare `return`) automatically includes the borrowed capabilities. The programmer never writes `return fail(...), fs` — the compiler handles it based on the parameter types.

**How auto-rebinding works:**

When a function has a capability parameter like `fs: Filesystem`, every call to a method on `fs` (like `Filesystem.read_file(fs, path)`) is automatically rebound by the compiler. The programmer writes the call without an assignment prefix, and the compiler silently updates `fs` to the returned capability value. This is the ONLY case where a parameter is implicitly rebound. The compiler recognizes capability types (Filesystem, Network, Stdout, Stderr, Stdin, Clock, Random, Process, Environment) and automatically threads them through the function and returns them to the caller. No `with` clause is needed.

#### What the Compiler Rejects

**A function trying to do I/O without a capability:**

```
function sneaky_logger(message: string) returns nothing:
    Stdout.write(stdout, message)
    # COMPILE ERROR: "stdout" is not defined
    # "sneaky_logger" does not have a Stdout capability in its parameters
    # hint: add "stdout: Stdout" to the function parameters
```

**A function trying to access the network without a capability:**

```
function fetch_data(url: string) returns result[string, string]:
    use net.http
    return http.get(url)
    # COMPILE ERROR: "http.get" requires a Network capability
    # but "fetch_data" does not have one in its parameters
    # hint: add "net: Network" to the function parameters
```

**A pure function guaranteed by its signature:**

```
function calculate_tax(income: float, rate: float) returns float:
    return income * rate
    # No capability parameters. The compiler GUARANTEES this function:
    # - Does not read or write files
    # - Does not access the network
    # - Does not print to stdout
    # - Does not read the clock
    # - Does not use randomness
    # - Has ZERO side effects of any kind
```

#### Scoped Capabilities — Restricting What a Function Can Do

Capabilities can be **narrowed** before being passed down. This lets `main()` grant only the minimum permissions needed:

```
function main(fs: Filesystem, net: Network, stdout: Stdout) returns nothing:
    # Create a read-only filesystem view:
    let read_fs = Filesystem.read_only(fs)

    # Pass only read access to the config loader:
    let config = load_config(read_fs, "app.conf") handle error:
        Stdout.write(stdout, "failed")
        return

    # load_config physically cannot write files — it only has read_only access.
```

**Narrowing options:**

```
let read_fs = Filesystem.read_only(fs)           # can read, cannot write
let scoped_fs = Filesystem.scoped(fs, "/data/")    # can only access files under /data/
let local_net = Network.allow(net, "localhost")  # can only connect to localhost
let limited_stdout = Stdout.buffered(stdout) # writes are buffered, not immediate
```

Capability narrowing **consumes** the original capability. `let read_fs = Filesystem.read_only(fs)` consumes `fs` — only `read_fs` remains. To keep both full and restricted access, clone first: `let read_fs = Filesystem.read_only(Linear.clone(fs))`.

This gives fine-grained control over what each function can do, and it's all visible in the function signature and the narrowing call.

#### How Capabilities Declare Effects

The presence of a capability parameter **is** the effect declaration. There is no separate `effects` keyword — the signature tells you everything:

| Signature | What it tells you |
|-----------|------------------|
| `function read(fs: Filesystem, path: string)` | Reads/writes files |
| `function send(net: Network, data: string)` | Accesses the network |
| `function log(stdout: Stdout, msg: string)` | Writes to stdout |
| `function compute(x: int) returns int` | Pure — no capability, no side effects |

A `Filesystem` parameter tells you "this function reads/writes files specifically." A `Network` parameter tells you "this function accesses the network." The capability is the effect declaration, made concrete.

#### Why This Is Perfect for LLMs

**1. Side effects are visible in the signature — zero call-chain analysis needed.**

The LLM reads `function send_report(net: Network, stdout: Stdout, report: Report)` and knows instantly: this function uses the network and writes to stdout. No implementation reading required. No recursive call-chain analysis. The signature is a complete contract.

**2. Pure functions are provably pure.**

If a function has no capability parameters, it is pure. Not "probably pure" or "assumed pure" — the compiler has mathematically proven it cannot perform side effects. The LLM can trust this guarantee completely.

**3. The LLM can't hallucinate side effects.**

In traditional languages, an LLM might add a `log.info()` call inside a utility function, silently introducing a side effect. In Jett, that call requires a `Stdout` capability. If the function doesn't have one, the code doesn't compile. The LLM is forced to either add the capability to the signature (making the effect visible) or remove the logging call.

**4. Capability threading mirrors auto-regressive generation.**

The LLM generates `main()` first, which has all capabilities. As it generates child functions, it must explicitly pass down the capabilities each one needs. This is a natural top-down flow that matches the LLM's left-to-right generation process. The LLM never needs to "go back" and add a capability — it threads them forward as it writes.

**5. Testing is trivial.**

To test a function that takes a `Filesystem` capability, pass a mock filesystem. The function doesn't know the difference — it just calls methods on the capability object. No dependency injection framework, no global state to reset, no monkey-patching:

```
property load_config_parses_valid_json:
    use test.mock
    given port: int where port > 0 and port < 65536
    let mock_fs = test.mock.filesystem(map(
        "app.conf": "{{\"port\": {port}}"
    ))
    let config = load_config(mock_fs, "app.conf") handle error:
        assert false "should not fail"
    assert config.port is port
```

### Rule Set 17: Cross-Platform Compilation (Agnostic Capability Lowering)

#### The Problem: OS-Specific Code Multiplies Hallucination Surface

In C++ or Rust, writing cross-platform code means writing OS-specific branches:

```
// Rust — the LLM must know 3 different OS APIs for one operation:
#[cfg(target_os = "windows")]
fn open_socket() { /* Win32 Winsock API */ }

#[cfg(target_os = "linux")]
fn open_socket() { /* POSIX socket API */ }

#[cfg(target_os = "macos")]
fn open_socket() { /* BSD/kqueue API */ }
```

This forces the LLM to understand and memorize Win32 APIs, POSIX syscalls, Apple's BSD/Cocoa frameworks, and the conditional compilation syntax to switch between them. Each OS API is a separate hallucination surface — the LLM may confidently generate a Windows function call with the wrong argument order, or a Linux syscall that doesn't exist on the target kernel version.

The problem compounds: for N platforms and M I/O operations, the LLM must know N × M platform-specific implementations. For 3 platforms and 20 operations, that's 60 different API calls to get right. An LLM will not.

#### The Solution: The Compiler Is the HAL

Jett's capability system (Rule Set 16) naturally solves cross-platform compilation. The LLM writes **platform-agnostic code** using universal capability objects. The compiler maps those capabilities to the correct OS-specific implementation at compile time based on the target platform.

**The LLM writes this — once, for all platforms:**

```
function start_server(net: Network, stdout: Stdout, port: int) returns nothing:
    let listener = Network.listen(net, "0.0.0.0", port) handle error:
        Stdout.write(stdout, "failed to bind port")
        return

    Stdout.write(stdout, "listening on port {port}")

    while true:
        let connection = Network.accept(net, listener) handle error:
            Stdout.write(stdout, "accept failed")
            continue
        handle_connection(net, stdout, connection)
```

This code does not contain a single OS-specific reference. No `#ifdef`, no `cfg!()`, no `#[target_os]`. The LLM writes against the `Network` capability interface. The compiler does the rest.

**What the compiler does at build time:**

```
jett build server.jett --target linux-x86_64
# Compiler maps: Network.listen() → POSIX socket() + bind() + listen()
# Compiler maps: Network.accept() → POSIX accept()
# Output: native Linux ELF binary

jett build server.jett --target windows-x86_64
# Compiler maps: Network.listen() → Winsock WSASocket() + bind() + listen()
# Compiler maps: Network.accept() → Winsock accept()
# Output: native Windows PE binary

jett build server.jett --target macos-arm64
# Compiler maps: Network.listen() → BSD socket() + bind() + listen()
# Compiler maps: Network.accept() → BSD accept() with kqueue
# Output: native macOS Mach-O binary

jett build server.jett --target wasm
# Compiler maps: Network.listen() → WASI socket API
# Output: WebAssembly module
```

The same Jett source code compiles to 4 different platforms. The LLM wrote the code once. Zero conditional compilation. Zero OS-specific knowledge required.

#### How Capability Lowering Works

Every capability type has a **universal interface** that the LLM programs against, and a **platform-specific implementation** that the compiler selects at build time.

**The universal interface (what the LLM sees):**

```
# Filesystem capability — platform-agnostic operations:
# Filesystem.read_file(fs, path)      → reads a file, returns string
# Filesystem.write_file(fs, path, data) → writes data to a file
# Filesystem.list_dir(fs, path)       → lists directory contents
# Filesystem.file_exists(fs, path)    → checks if file exists
# Filesystem.delete_file(fs, path)    → deletes a file
# Filesystem.create_dir(fs, path)     → creates a directory

# The LLM never sees:
# - Windows: CreateFileW, ReadFile, FindFirstFileW
# - Linux: open(), read(), opendir()
# - macOS: open(), read() (BSD variants)
```

**Path normalization:**

```
# The LLM writes forward slashes everywhere:
let config = Filesystem.read_file(fs, "data/config/app.json") handle error:
    return fail("config not found")

# When compiled for Windows, the compiler automatically translates
# "data/config/app.json" to "data\\config\\app.json" internally.
# The LLM never writes backslashes. The LLM never handles path separators.
```

**The full capability lowering table:**

| Capability | What the LLM writes | Windows lowering | Linux lowering | macOS lowering |
|-----------|---------------------|-----------------|---------------|---------------|
| `Filesystem.read_file` | `Filesystem.read_file(fs, path)` | `CreateFileW` + `ReadFile` | `open` + `read` | `open` + `read` |
| `Filesystem.write_file` | `Filesystem.write_file(fs, path, data)` | `CreateFileW` + `WriteFile` | `open` + `write` | `open` + `write` |
| `Network.listen` | `Network.listen(net, addr, port)` | Winsock `WSASocket` + `bind` | `socket` + `bind` + `listen` | BSD `socket` + `bind` + `listen` |
| `Network.connect` | `Network.connect(net, addr, port)` | Winsock `connect` | `connect` | `connect` |
| `Stdout.write` | `Stdout.write(stdout, text)` | `WriteConsoleW` | `write(1, ...)` | `write(1, ...)` |
| `Process.spawn` | `Process.spawn(proc, cmd, args)` | `CreateProcessW` | `fork` + `execvp` | `posix_spawn` |
| `Clock.now` | `Clock.now(clock)` | `GetSystemTimeAsFileTime` | `clock_gettime` | `gettimeofday` |
| `Environment.get` | `Environment.get(env, key)` | `GetEnvironmentVariableW` | `getenv` | `getenv` |

The entire left column is what the LLM writes. The right columns are what the compiler generates. The LLM never sees the right columns.

#### Zero Conditional Compilation

Jett has **no conditional compilation syntax**. There is no `#ifdef`, no `cfg!()`, no `#if TARGET_OS`. The language does not have a mechanism for the LLM to write platform-specific branches, because it never needs to.

**What the compiler rejects:**

There is simply no syntax for it. The LLM cannot write "if windows then X else Y" because the language does not provide that construct. The only way to interact with the OS is through capabilities, and capabilities are platform-agnostic by design.

If a genuinely platform-specific behavior is needed (rare, and only for advanced use cases), it is handled in the **standard library's capability implementations**, not in user code. The user code stays agnostic.

#### Build Targets

```
# Build for the current platform:
jett build server.jett

# Cross-compile for a specific target:
jett build server.jett --target linux-x86_64
jett build server.jett --target linux-arm64
jett build server.jett --target windows-x86_64
jett build server.jett --target macos-arm64
jett build server.jett --target macos-x86_64
jett build server.jett --target wasm

# Build for multiple targets at once:
jett build server.jett --target linux-x86_64,windows-x86_64,macos-arm64
```

Cross-compilation is a first-class feature of the compiler, not an afterthought. Like Zig and Go, the Jett compiler can produce binaries for any supported target from any host platform.

#### Why This Is Perfect for LLMs

**1. One codebase, zero platform knowledge required.**

The LLM writes one version of the code. It never needs to know that Windows uses `CreateFileW` while Linux uses `open()`. It never needs to memorize the differences between POSIX and Win32 APIs. The hallucination surface for OS-specific calls is reduced to zero.

**2. No conditional compilation to get wrong.**

LLMs frequently hallucinate the syntax or semantics of conditional compilation. Is it `#ifdef _WIN32`? `#if defined(__APPLE__)`? `cfg!(target_os = "windows")`? In Jett, the question doesn't arise — there is no conditional compilation syntax to get wrong.

**3. The capability interface is the only API surface.**

The LLM learns `Filesystem.read_file`, `Network.listen`, `Stdout.write`. These work on every platform. The total API surface the LLM needs to know is the capability interface — a small, stable, well-documented set of functions. Not N × M platform-specific functions.

**4. Cross-compilation is a build flag, not a code change.**

Deploying to a new platform requires `--target linux-arm64`, not a code rewrite. The LLM's generated code is immediately portable to any target the compiler supports.

### Rule Set 18: Zero-Boilerplate, Native Serialization

#### The Problem: LLMs Cannot Write Parsers Reliably

Parsing — JSON deserialization, binary format decoding, network packet construction — is one of the most error-prone tasks for LLMs. Correct parsing requires:

- Tracking byte offsets precisely (LLMs cannot count bytes — Rule Set 12)
- Handling endianness (big-endian vs little-endian field ordering)
- Writing robust error handling for malformed input at every field boundary
- Managing variable-length fields, optional fields, and nested structures
- Keeping deserialization code perfectly in sync with the struct definition

LLMs routinely hallucinate byte offsets, forget to handle endianness, skip error checks on individual fields, and generate deserialization code that drifts from the struct definition after a refactor. Every hand-written parser is a bug waiting to happen.

Even JSON — the "easy" format — causes problems. An LLM might forget to handle a missing optional field, parse a number as a string, or generate code that doesn't match the struct's field names after renaming.

#### The Solution: Serialization Is a Compiler Primitive

In Jett, serialization and deserialization are **not libraries**. They are **native compiler features** that are automatically generated for every data type. The LLM never writes a parser. It defines the data structure, and the compiler produces correct, optimized serialization code at compile time.

**Every struct is automatically compatible with the `json` module and binary serialization:**

```
struct User:
    id: string
    name: string
    email: string
    age: int

# The compiler makes User compatible with:
# json.serialize(user)       → string (JSON representation)
# json.parse(raw, User)      → result[User, string]
# User.to_bytes(user)        → bytes (compact binary representation)
# User.from_bytes(raw)      → result[User, string]
```

The compiler makes every struct compatible with `json.serialize()` and `json.parse(raw, Type)` automatically. There are no auto-generated `.to_json()` or `.from_json()` methods on the struct itself — the `json` module functions are the canonical API. `json.serialize` declares a `view` parameter — it reads the value without consuming it. Because the parameter is declared as `view`, callers simply write `json.serialize(user)` — the `view` keyword only appears in parameter declarations, not at call sites. The compiler handles the view semantics automatically. `json.parse(raw, Type)` is the **only** form — the Type parameter is mandatory, not optional. It parses a JSON string into the specified type and returns `result[Type, string]`. There is no single-argument `json.parse(raw)` that returns an untyped value. For structs with `secret[T]` fields, `json.serialize_public(value)` omits those fields.

The LLM does not write parsing functions. The LLM does not import a serialization library. The LLM does not annotate fields with `#[serde(rename = "...")]` or `@JsonProperty`. The compiler sees the struct definition and generates everything.

**Using auto-generated serialization:**

```
function save_user(fs: Filesystem, user: view User) returns result[nothing, string]:
    let json_data = json.serialize(user)
    Filesystem.write_file(fs, "users/{user.id}.json", json_data) handle error:
        return fail("could not save user")
    return ok(nothing)

function load_user(fs: Filesystem, id: string) returns result[User, string]:
    let raw = Filesystem.read_file(fs, "users/{id}.json") handle error:
        return fail("user file not found")
    let user = json.parse(raw, User) handle error:
        return fail("invalid user data")
    return ok(user)
```

The LLM writes business logic — save this user, load that user. The serialization is a single function call. No parsing loops, no field-by-field extraction, no byte offset arithmetic.

#### How the Compiler Generates Serialization

The compiler uses the **comptime engine** (Rule Set 10.5) to generate serialization functions at compile time. It inspects the struct definition and produces optimal code for each format.

**JSON generation — field names match struct fields exactly:**

```
struct Product:
    name: string
    price: float
    in_stock: bool
    tags: list[string]

let p = Product(name: "widget", price: 9.99, in_stock: true, tags: list("sale", "new"))

let json_string = json.serialize(p)
# Result: {"name":"widget","price":9.99,"in_stock":true,"tags":["sale","new"]}
```

There is no configuration. Field names in JSON match field names in the struct. The types determine the JSON types (string → JSON string, float → JSON number, bool → JSON boolean, list → JSON array). This is the only way — zero syntactic sugar, zero alternatives (Rule Set 1).

**Binary generation — compact, deterministic layout:**

```
let binary_data = Product.to_bytes(p)
# Compact binary representation:
# - Fixed-size fields are stored inline
# - Variable-size fields (strings, lists) use length-prefixed encoding
# - Byte order is always little-endian (no configuration)
# - The format is deterministic: same input → same bytes, always

let restored = Product.from_bytes(binary_data) handle error:
    return fail("corrupt data")
# restored is identical to p
```

The binary format is deterministic and self-describing enough to detect corruption. The LLM never chooses an endianness, never writes a length prefix, never pads a field. The compiler handles all of it.

#### Serialization with Secret Types

The auto-generated serialization respects `secret` types (Rule Set 15):

```
struct UserRecord:
    id: string
    name: string
    password_hash: secret[string]
    api_key: secret[string]

# Serialization behavior:
# json.serialize(user) → COMPILE ERROR: struct contains secret fields
#   hint: use json.serialize_public(user) to serialize non-secret fields only

# The json module provides two serialization paths:
# json.serialize_public(user) → {"id":"123","name":"alice"}
#   (secret fields are omitted)
# json.serialize_full(user, declassify_token) → requires explicit declassification
#   (only callable with a declassification token — see Rule Set 15)
```

Calling `json.serialize` on a struct with secret fields is a compile error. The LLM cannot accidentally serialize secrets. It must explicitly choose `json.serialize_public` (safe) or `json.serialize_full` with a declassification token (auditable).

#### Serialization with State Machines

State machines (Rule Set 9) also get auto-generated serialization, with the state tag included:

```
machine OrderProcess:
    states:
        draft(items: list[Item])
        submitted(items: list[Item], submitted_at: time.Timestamp)
        shipped(tracking: string, shipped_at: time.Timestamp)

let order = OrderProcess(draft, items: list(Item(name: "widget", qty: 2)))

let json_string = json.serialize(order)
# Result: {"state":"draft","items":[{"name":"widget","qty":2}]}

let restored = json.parse(json_string, OrderProcess) handle error:
    return fail("invalid order data")
# restored is in the "draft" state with the same items
```

The serialized form includes the state name. Deserialization restores the correct state with the correct state-specific data. The LLM never writes state-aware parsing logic.

#### Serialization with Refinement Types

Deserialization automatically validates refinement type constraints (Rule Set 3):

```
type Age = int where value >= 0 and value < 150
type Email = string where string.contains(value, "@")

struct ValidatedUser:
    name: string
    age: Age
    email: Email

let raw = "{{\"name\":\"alice\",\"age\":-5,\"email\":\"alice@example.com\"}}"
let user = json.parse(raw, ValidatedUser) handle error:
    return fail("invalid user: {error}")
# RUNTIME ERROR during json.parse:
#   field "age": value -5 does not satisfy: value >= 0
```

The compiler ensures `json.parse` checks every refinement constraint during deserialization. The LLM does not write validation logic. It defines the types with constraints, and the deserializer enforces them automatically.

#### Custom Field Naming (When External APIs Require It)

Sometimes external APIs use different naming conventions (camelCase, PascalCase, different field names entirely). Jett handles this with a `serialize` annotation, keeping the mapping co-located with the struct:

```
struct ApiResponse:
    user_name: string serialize "userName"
    total_count: int serialize "totalCount"
    is_active: bool serialize "isActive"

# json.serialize produces: {"userName":"...","totalCount":42,"isActive":true}
# json.parse accepts: {"userName":"...","totalCount":42,"isActive":true}
# The struct code always uses snake_case field names internally.
```

The `serialize` annotation is the only way to customize field naming. It is co-located with the field definition — the LLM never has to look elsewhere to know the JSON field name.

#### Network Protocol Structs

For binary network protocols, structs can specify a precise binary layout:

```
struct PacketHeader layout binary:
    magic: int size 4
    version: int size 2
    payload_length: int size 4
    checksum: int size 4

# The compiler generates:
# PacketHeader.to_bytes(header) → exactly 14 bytes, fields packed in declaration order
# PacketHeader.from_bytes(raw)  → parses exactly 14 bytes, validates magic/checksum
```

The `layout binary` annotation with `size` on each field gives the compiler enough information to generate a perfect binary parser. The LLM specifies *what* the format is (field names, sizes, order). The compiler generates *how* to parse it (byte offsets, endianness, boundary checks).

#### Why This Is Perfect for LLMs

**1. The LLM never writes a parser.**

Zero parsing code means zero parsing bugs. No byte offsets to hallucinate, no endianness to forget, no field-by-field extraction loops to get wrong.

**2. Struct definition is the single source of truth.**

When the LLM adds a field to a struct, the serialization is automatically updated. There is no separate parser file to keep in sync. No "I added `email` to the struct but forgot to update the JSON parser" bugs.

**3. Validation is automatic.**

Refinement types are enforced during deserialization. The LLM defines `type Age = int where value >= 0` once, and every JSON payload, binary blob, and network packet is validated against that constraint automatically.

**4. Security is automatic.**

Secret fields are excluded from default serialization. The LLM cannot accidentally serialize a password hash into a JSON API response because `json.serialize` on a struct with secret fields is a compile error.

**5. The LLM writes business logic, not I/O plumbing.**

The LLM's job is reduced to: define the struct, call `json.serialize()` or `json.parse()`. The entire serialization layer — format handling, error checking, validation, security — is generated by the compiler.

### Rule Set 19: The Native Pipeline Operator

#### The Problem: Inside-Out Function Nesting

In most languages, composing multiple operations requires nesting function calls from the inside out:

```
# The LLM must plan this inside-out:
let response = format_to_json(fetch_database_records(authenticate_user(request)))
```

To generate this line, the LLM must:

1. Know the final operation it wants (`format_to_json`) — but it can't write it first, because the argument isn't ready yet.
2. Know the middle operation (`fetch_database_records`) — same problem.
3. Start with the innermost call (`authenticate_user(request)`) — the first thing to execute is the last thing to write.

This is **anti-auto-regressive**. The LLM generates tokens left-to-right, but the execution order is right-to-left (innermost to outermost). The LLM must mentally plan the entire chain before emitting the first token, then write it backwards. This is exactly the kind of "look-ahead" planning that LLMs are bad at.

Deeply nested calls also create bracket-matching problems (Rule Set 11) and split the LLM's attention across multiple nesting levels (Rule Set 7).

#### The Solution: The `|>` Pipeline Operator

Jett provides a native pipeline operator `|>` that passes the result of the left expression as the first argument to the function on the right. Data flows **left-to-right, top-to-bottom** — exactly matching the LLM's auto-regressive generation order.

**The same logic, written as a pipeline:**

```
let response = request
    |> authenticate_user
    |> fetch_database_records
    |> format_to_json
```

The LLM generates this top-to-bottom, in execution order:

1. Start with `request` — the input.
2. Pipe to `authenticate_user` — the first operation.
3. Pipe to `fetch_database_records` — the second operation.
4. Pipe to `format_to_json` — the final operation.

Each line is one step. The generation order matches the execution order. The LLM never has to "plan ahead" or write things backwards.

#### How `|>` Works

The pipeline operator takes the expression on its left and passes it as the **first argument** to the function on its right.

**Desugaring:**

```
# Pipeline form:
x |> f

# Desugars to:
f(x)

# Multi-step pipeline:
x |> f |> g |> h

# Desugars to:
h(g(f(x)))

# Pipeline with additional arguments:
x |> f(extra_arg)

# Desugars to:
f(x, extra_arg)
```

**Practical examples:**

```
# Data processing pipeline:
let report = raw_data
    |> string.split("\n")
    |> list.filter(function(line: string) returns bool: return string.is_not_empty(line))
    |> list.map(function(line: string) returns list[string]: return string.split(line, ","))
    |> list.skip(1)
    |> build_report

# HTTP request handling:
let response = request
    |> validate_auth
    |> extract_user_id
    |> load_user_profile
    |> json.serialize
```

```
# String transformation pipeline:
let slug = title
    |> string.trim
    |> string.lower
    |> string.replace(" ", "-")
    |> string.replace("--", "-")
```

#### Type Safety Across Pipelines

The compiler checks that types match at every `|>` boundary. If a function returns `string` but the next function in the pipeline expects `int`, the compiler catches it immediately.

```
function get_name(user: view User) returns string:
    return user.name

function double(x: int) returns int:
    return x * 2

let result = user
    |> get_name
    |> double
    # COMPILE ERROR at |> double:
    #   "get_name" returns string
    #   "double" expects int as first argument
    #   hint: the types in the pipeline do not connect
```

The compiler error points at the exact `|>` step where the types break. The LLM knows exactly which connection in the chain is wrong.

#### Pipelines with Error Handling

Pipelines integrate with the `result` type and `handle` keyword (Rule Set 5). When a pipeline step can fail, the LLM handles the error inline:

```
let user_data = request
    |> validate_auth handle error:
        return fail("auth failed")
    |> extract_user_id handle error:
        return fail("no user id")
    |> load_user_profile handle error:
        return fail("user not found")
    |> json.serialize_public
```

Each `handle` block applies to the pipeline step immediately before it. The error handling is co-located with the operation that can fail — no distant `catch` blocks, no forgotten error paths.

**Pipeline + handle semantics:**

- `|> function_call handle: ...` is a **single pipeline step**. The `handle` is attached to the function call, not to the pipeline itself.
- On success: `handle` unwraps the `result` (or `optional`), and the unwrapped success value flows to the next `|>` step.
- On failure: the `handle` block executes. There are **two valid forms**:
  1. **Default form:** `handle error: default Config(port: 8080)` — the `default` keyword provides a fallback value and execution continues normally.
  2. **Return form:** `handle error: return fail(...)` — early exit from the enclosing function. The pipeline (and function) terminates immediately.
- The pipeline only continues to the next `|>` if every preceding step either succeeded or provided a fallback via `default`.

In the example above, if `validate_auth` returns `fail(...)`, the `handle` block runs `return fail("auth failed")` and the entire pipeline (and enclosing function) returns immediately. If `validate_auth` returns `ok(auth_token)`, the unwrapped `auth_token` flows into `extract_user_id` as the first argument.

#### Pipelines with Capabilities

Pipelines work naturally with capability-based I/O (Rule Set 16). The capability is passed as an additional argument:

```
function process_request(fs: Filesystem, net: Network, stdout: Stdout, request: Request) returns result[string, string]:
    let output = request
        |> authenticate
        |> authorize
        |> fetch_data(fs) handle error:
            return fail("data fetch failed")
        |> transform_response
        |> json.serialize

    Stdout.write(stdout, "processed request")
    return ok(output)
```

**Capability auto-threading in pipelines:**

When a pipeline step takes a capability parameter, the compiler auto-rebinds capability parameters and automatically threads the capability through the pipeline. The LLM writes `fetch_data(fs)` as a pipeline step, and the compiler handles the multi-value return invisibly:

1. `fetch_data(fs)` consumes `fs` and the compiler auto-rebinds it.
2. The compiler extracts `data` as the pipeline value for the next step.
3. The compiler holds `fs` aside and returns it from the enclosing function automatically.

The LLM never sees the multi-value return. It writes the pipeline as if capabilities don't exist — a single value flows left to right. The compiler handles the plumbing.

**What the compiler rejects:**

If a pipeline step consumes a capability but does not allow the compiler to auto-rebind it, the compiler reports an error:

```
let result = request
    |> authenticate
    |> consume_filesystem(fs)
    # COMPILE ERROR: pipeline step "consume_filesystem" consumes capability "fs"
    # but does not allow the compiler to auto-rebind it
    # hint: ensure consume_filesystem returns the capability for auto-rebinding
    |> transform
```

#### Why `|>` Is an Exception to Symbol Minimalism

Rule Set 1 establishes that Jett avoids exotic symbols. The `|>` operator is the **single exception**, justified by how fundamentally it aligns with the LLM generation model:

1. **`|>` is well-tokenized.** Major LLM tokenizers encode `|>` as 1-2 tokens (often a single token) because it appears frequently in Elixir, F#, and OCaml training data.
2. **No keyword alternative is as clear.** Alternatives like `then` or `pipe` would work but add a token to every pipeline step. The symbol `|>` is visually distinctive — it looks like data flowing to the right, which is exactly what it does.
3. **It is the only symbol operator added.** Jett does not have `->`, `=>`, `<-`, `|`, `>>`, or any other arrow/pipe variant. There is one pipeline symbol, and it does one thing.

#### The `|>` Operator vs Direct Calls — Compiler-Enforced One Form Per Case

To satisfy Rule Set 1 (strictly one canonical form), `|>` and nested function calls are **not** interchangeable. The compiler enforces distinct use cases:

- **`|>` for chains of 2+ operations.** When data flows through a sequence of transformations, use the pipeline. **Chained/sequential nesting where data flows through a chain is a compile error** — `f(g(h(x)))` is banned because data flows sequentially through `h`, then `g`, then `f`. Use `x |> h |> g |> f` instead.
- **Argument expressions are allowed.** `"prefix{value}"` is fine because string interpolation is an expression, not a sequential data-flow chain. The rule targets left-to-right data flow chains, not every function call inside another function call.
- **Direct calls for single operations.** `let x = f(y)` is the form for a single function call.

```
# Single call — correct:
let trimmed = string.trim(name)

# ALLOWED — string interpolation:
let message = "count: {total}"

# BANNED — sequential chain (depth 3):
let result = format(process(parse(input)))
# COMPILE ERROR: use pipeline instead
# hint: rewrite as: input |> parse |> process |> format

# Pipeline — correct:
let result = input
    |> parse
    |> process
    |> format
```

This means there is exactly one form for single calls (direct) and exactly one form for chained calls (pipeline). Argument expressions inside a function call are not considered chaining — the ban applies to sequential data-flow nesting, not to every function call that appears inside another. No ambiguity, no choice.

#### Why This Is Perfect for LLMs

**1. Generation order matches execution order.**

The LLM writes step 1, then step 2, then step 3 — in the order they execute. No inside-out planning, no backward construction, no look-ahead required.

**2. Each pipeline step is one line, one operation.**

The LLM generates one line per operation. Each line is self-contained. The attention mechanism focuses on one step at a time. The previous step's output type is immediately above — maximum context proximity.

**3. The compiler validates the chain.**

Type checking at every `|>` boundary means the LLM gets immediate feedback if any step produces the wrong type. The error points at the exact broken connection. The LLM fixes one step, not the whole chain.

**4. Flat, linear, readable.**

Pipelines eliminate deep nesting entirely. A 5-step pipeline is 5 lines of code at the same indentation level. No bracket matching, no indentation tracking, no "which closing paren belongs to which opening paren?" ambiguity.

**5. Naturally encourages small, composable functions.**

To use a pipeline, each step must be a function that takes an input and returns an output. This naturally produces the small, focused, pure functions that Rule Sets 2, 7, and 13 encourage. The pipeline operator makes good architecture the path of least resistance.

### Rule Set 20: Zero-Boilerplate C Interop (Auto-FFI)

#### The Problem: Manual Bindings Are a Hallucination Minefield

Every major operating system's native APIs — GUI frameworks, system calls, hardware access — are built on C or C++ interfaces. To call them from a new language, developers traditionally write thousands of lines of manual "bindings": calculating struct sizes, mapping pointer types, handling memory ownership across the language boundary, and translating calling conventions.

For LLMs, writing C bindings is catastrophic:

- **Pointer sizes vary by platform.** A pointer is 4 bytes on 32-bit, 8 bytes on 64-bit. The LLM will guess wrong.
- **Struct padding and alignment.** C compilers insert invisible padding between fields. An LLM cannot calculate these offsets accurately.
- **Ownership across boundaries.** Who frees the memory — the C side or the Jett side? The LLM will hallucinate the wrong answer.
- **Calling conventions.** `__stdcall` vs `__cdecl` vs `__fastcall` on Windows alone. Different argument passing on ARM vs x86.
- **String encoding.** C strings are null-terminated byte arrays. Windows uses UTF-16 (`wchar_t*`). The LLM will confuse them.

Every one of these is a silent failure — the code compiles, runs, and either crashes with a segfault or silently corrupts memory. An LLM generating C bindings will produce code that looks plausible but is subtly, fatally wrong.

#### The Solution: The Compiler Natively Understands C

Jett's compiler includes a built-in C header parser. The LLM does not write bindings. It **imports a C header file directly**, and the compiler automatically translates every C function, struct, enum, and constant into Jett's safe, linear-typed syntax.

**Importing a C header:**

```
use c "SDL2/SDL.h" as sdl
use c "stdio.h" as stdio
use c "windows.h" as win32
```

That's it. The `use c` directive tells the compiler to:

1. Parse the C header file.
2. Analyze every function signature, struct layout, enum, and `#define` constant.
3. Generate safe Jett wrappers that handle pointer translation, memory ownership, string encoding, and calling conventions.
4. Expose the translated API under the given module name.

The LLM never sees a pointer. The LLM never calculates a struct offset. The LLM never handles null termination.

#### How Auto-FFI Translation Works

**C functions become safe Jett functions:**

```
// C header (SDL2/SDL.h):
// int SDL_Init(Uint32 flags);
// SDL_Window* SDL_CreateWindow(const char* title, int x, int y, int w, int h, Uint32 flags);
// void SDL_DestroyWindow(SDL_Window* window);
// void SDL_Quit(void);

# What the LLM sees in Jett after `use c "SDL2/SDL.h" as sdl`:
# sdl.init(flags: int) returns result[int, string]
# sdl.create_window(title: string, x: int, y: int, w: int, h: int, flags: int) returns result[sdl.Window, string]
# sdl.destroy_window(window: sdl.Window)
# sdl.quit()
```

The compiler automatically:

- **Translates `char*` to `string`** with null-termination handled internally.
- **Wraps raw pointers in opaque handle types** (`SDL_Window*` becomes `sdl.Window` — an opaque, linear type).
- **Makes fallible functions return `result`** (any C function that can return NULL or an error code gets wrapped in `result[T, string]`).
- **Converts naming conventions** (`SDL_CreateWindow` → `sdl.create_window` in snake_case).
- **Handles memory ownership** based on C conventions (functions named `Create*` allocate, functions named `Destroy*`/`Free*` deallocate).

**Using the translated API — the LLM writes safe Jett code:**

```
function create_game_window(stdout: Stdout) returns result[sdl.Window, string]:
    use c "SDL2/SDL.h" as sdl

    sdl.init(sdl.INIT_VIDEO) handle error:
        return fail("SDL init failed")

    let window = sdl.create_window(
        "My Game",
        sdl.WINDOWPOS_CENTERED,
        sdl.WINDOWPOS_CENTERED,
        800, 600,
        sdl.WINDOW_SHOWN
    ) handle error:
        return fail("could not create window")

    Stdout.write(stdout, "window created")
    return ok(window)
```

This code calls the real SDL2 C library with full native performance. The LLM wrote zero unsafe code. No pointers, no manual memory management, no calling convention annotations. The compiler generated all the glue.

#### Opaque Handle Types — Linear Safety Across the FFI Boundary

C pointers are translated into **opaque, linear handle types**. Because they are linear (Rule Set 10.1), they must be explicitly consumed — preventing use-after-free across the language boundary.

```
function window_lifecycle(stdout: Stdout) returns nothing:
    use c "SDL2/SDL.h" as sdl

    sdl.init(sdl.INIT_VIDEO) handle error:
        return

    let window = sdl.create_window("Test", 100, 100, 640, 480, 0) handle error:
        Stdout.write(stdout, "failed to create window")
        sdl.quit()
        return

    # Use the window...
    do_rendering(window)

    # window was consumed by do_rendering (linear type).
    # If do_rendering doesn't return it, we can't use it here.
    # If we need it back, do_rendering must return it as part of its return type.

    sdl.quit()
```

**What the compiler prevents:**

```
function bad_example() returns nothing:
    use c "SDL2/SDL.h" as sdl

    let window = sdl.create_window("Test", 100, 100, 640, 480, 0) handle error:
        return

    sdl.destroy_window(window)
    sdl.destroy_window(window)
    # COMPILE ERROR: "window" was consumed by the first sdl.destroy_window call
    # hint: the window has already been destroyed and cannot be used again
```

Double-free — one of the most common C interop bugs — is a compile error. Use-after-free is a compile error. The linear type system makes memory corruption across the FFI boundary structurally impossible.

#### C Struct Translation

C structs are automatically translated to Jett structs with correct field types and layout:

```
// C header:
// typedef struct {
//     float x, y, z;
//     uint32_t color;
//     float u, v;
// } Vertex;

# Jett sees:
# struct sdl.Vertex:
#     x: float
#     y: float
#     z: float
#     color: int
#     u: float
#     v: float
#
# With auto-generated to_bytes/from_bytes that match the exact C memory layout
# (including padding and alignment for the target platform).
```

The LLM creates `Vertex` structs using normal Jett syntax. The compiler handles the binary layout translation when passing to C functions.

#### Platform-Specific C Headers and Capability Integration

C header imports integrate with the capability system (Rule Set 16) and cross-platform compilation (Rule Set 17):

```
# The LLM writes platform-agnostic GUI code using capabilities:
function create_text_input(gui: GuiCapability, label: string) returns result[TextInput, string]:
    # The compiler resolves GuiCapability to platform-specific C calls:
    # Windows: CreateWindowExW("EDIT", ...) via windows.h
    # macOS: NSTextField via Cocoa.h
    # Linux: gtk_entry_new() via gtk/gtk.h

    let input = GuiCapability.create_text_field(gui, label, width: 200, height: 30) handle error:
        return fail("could not create text input")
    return ok(input)
```

For GUI and OS-specific APIs, the **capability system acts as the abstraction layer** and the **Auto-FFI acts as the implementation layer**. The LLM writes against capabilities. The compiler uses Auto-FFI to call the correct C library for the target platform.

#### Compile-Time Header Analysis

The C header parsing happens entirely at **compile time** using the comptime engine (Rule Set 10.5):

1. The compiler reads the `.h` file.
2. It parses all `typedef`, `struct`, `enum`, `#define`, and function declarations.
3. It generates Jett-safe wrappers with correct types, linear ownership, and error handling.
4. It resolves platform-specific types (`size_t`, `DWORD`, `HANDLE`) to the correct Jett types for the target platform.
5. The generated wrappers are type-checked against Jett's type system.

The LLM never sees this process. It sees a clean Jett module with safe functions.

#### Limitations and Safety Boundaries

Not everything in a C header can be safely auto-translated. The compiler handles these cases explicitly:

| C construct | Auto-FFI behavior |
|------------|-------------------|
| Functions with simple types | Fully auto-wrapped |
| Functions returning pointers | Wrapped in opaque linear handle types |
| Functions taking `void*` | Require explicit type annotation from the LLM |
| Variadic functions (`printf`, ...) | Not auto-wrapped; use Jett standard library equivalents |
| Function pointers / callbacks | Wrapped in Jett `function` types with safety checks |
| Preprocessor macros with logic | Translated to `comptime` functions where possible; complex macros flagged for manual review |
| Inline assembly | Not translated; flagged as unavailable |

When the compiler encounters a construct it cannot safely translate, it emits a clear warning and excludes that function from the generated module. The LLM cannot call an unsafe function that wasn't translated — it simply doesn't exist in the Jett namespace.

#### Why This Is Perfect for LLMs

**1. The LLM never writes unsafe code.**

No pointers, no `malloc`, no `sizeof`, no struct padding calculations. The compiler generates all of it. The LLM writes safe Jett code that happens to call C libraries at native speed.

**2. The entire C ecosystem becomes available.**

SDL2, OpenGL, Vulkan, Win32, Cocoa, GTK, POSIX — any C library with a header file can be imported. The LLM doesn't need specialized training on each library's binding format.

**3. Linear types prevent FFI memory bugs.**

The two most common FFI bugs — use-after-free and double-free — are compile errors. The linear type system tracks ownership across the language boundary.

**4. Naming conventions are auto-translated.**

`SDL_CreateWindow` becomes `sdl.create_window`. The LLM works with Jett's consistent `snake_case` naming. No context-switching between C naming conventions and Jett conventions.

**5. One import line replaces thousands of binding lines.**

`use c "SDL2/SDL.h" as sdl` replaces what would be thousands of lines of manual binding code in other languages. The LLM generates one line. The compiler does the rest.

### Rule Set 21: The Agent Server Protocol (ASP)

#### The Problem: Compiler Errors Are Designed for Human Eyes

Modern compilers produce beautifully formatted error messages — Rust's errors have colored arrows pointing to the exact character, GCC draws ASCII underlines, Clang shows column-aligned source snippets with carets. These are excellent for humans reading a terminal.

LLMs are terrible at parsing them:

- **Spatial formatting is noise.** Arrows (`^^^`), underlines (`~~~~`), box-drawing characters (`│`, `─`) consume tokens but carry no semantic information for the LLM. The LLM must "parse" visual art — a task its architecture is not designed for.
- **Terminal colors are invisible.** ANSI color codes (`\e[31m`) are either stripped (losing the emphasis they carried) or passed through as raw escape sequences (confusing the tokenizer).
- **Line/column references require mental mapping.** "Error on line 47, column 12" forces the LLM to count lines in its context to find the offending code. This is exactly the kind of positional counting that LLMs cannot do reliably (Rule Set 12).
- **Multi-error output is unstructured.** Five errors printed sequentially in a terminal are just a wall of text. The LLM must figure out where one error ends and the next begins.

The result: when an LLM receives compiler output, it spends tokens parsing formatting, miscounts line numbers, and often misidentifies the actual error — producing a "fix" that addresses the wrong problem.

#### The Solution: The Compiler Speaks JSON to LLMs

Jett ships with two output modes:

- **Human mode** (default): beautiful, formatted terminal output for developers.
- **Agent mode** (`--agent`): strict, deterministic JSON payloads designed for LLM consumption.

```
# Human-readable output (default):
jett build server.jett

# Agent-readable output (for LLMs):
jett build server.jett --agent
```

When `--agent` is passed, the compiler outputs **zero formatting, zero spatial art, zero color codes**. It emits a JSON object containing everything an LLM needs to understand and fix the error — structured, labeled, and unambiguous.

#### The Agent JSON Payload

**A single error:**

```json
{
    "status": "error",
    "errors": [
        {
            "code": "E0012",
            "severity": "error",
            "message": "cannot use secret[string] in string interpolation",
            "file": "src/handlers.jett",
            "line": 23,
            "column": 41,
            "ast_node": {
                "type": "string_interpolation",
                "parts": [
                    {"type": "string_literal", "value": "user: "},
                    {"type": "field_access", "object": "user", "field": "password_hash", "field_type": "secret[string]"}
                ]
            },
            "scope": {
                "variables": [
                    {"name": "user", "type": "User", "defined_line": 20},
                    {"name": "request", "type": "Request", "defined_line": 18}
                ]
            },
            "constraint_violated": {
                "rule": "secret_type_exposure",
                "expected": "string",
                "got": "secret[string]",
                "explanation": "secret[string] cannot be passed to functions that expose data (string interpolation, Stdout.write, log, http.respond)"
            },
            "suggested_fix": {
                "action": "replace",
                "line": 23,
                "old_text": "\"user: {user.password_hash}\"",
                "new_text": "\"user: {secret.redact(user.password_hash)}\"",
                "explanation": "use secret.redact() to get a masked representation of the secret value"
            }
        }
    ]
}
```

**What this payload contains:**

| Field | Purpose |
|-------|---------|
| `code` | Unique error identifier — deterministic, greppable |
| `message` | Plain-English description of the error |
| `file`, `line`, `column` | Exact location — no counting needed |
| `ast_node` | The exact AST node that failed — the LLM can see the tree structure of the broken expression |
| `scope` | All variables in scope at the error location — the LLM knows what it has to work with |
| `constraint_violated` | Which type rule or language rule was broken, what was expected, what was found |
| `suggested_fix` | A concrete, apply-ready fix with the exact old text and new text |

The LLM does not parse formatting. It reads structured JSON — something it is excellent at (Rule Set 1.3, AST-native syntax). Every piece of information is labeled, typed, and unambiguous.

#### The Closed-Loop Development Cycle

The Agent Server Protocol enables a **self-healing development loop** where the compiler feeds directly back into the LLM:

```
┌─────────────┐     Jett source      ┌──────────────┐
│             │ ──────────────────→  │              │
│     LLM     │                      │   Compiler   │
│             │ ←──────────────────  │  (--agent)   │
└─────────────┘     JSON errors      └──────────────┘
       │                                    │
       │  fix code                          │  if no errors
       │  based on                          │
       │  JSON payload                      ▼
       │                             ┌──────────────┐
       └─────────────────────────→   │    Binary    │
                                     └──────────────┘
```

**The cycle:**

1. LLM generates Jett source code.
2. Code is compiled with `jett build --agent`.
3. If errors → the JSON payload goes directly back to the LLM's API.
4. LLM reads the structured error, applies the suggested fix (or reasons about the constraint violation to produce its own fix).
5. Updated code is compiled again.
6. Repeat until the build succeeds.

This is not a theoretical workflow — it is the **intended primary development model** for Jett. The language is designed so that this loop converges quickly:

- Errors are precise (exact AST node, exact constraint).
- Fixes are concrete (old text → new text replacement).
- The LLM understands JSON natively.
- Each fix addresses exactly one error.

#### ASP Beyond Build Errors

The Agent Server Protocol extends to every compiler interaction, not just build errors:

**Type information queries:**

```
jett query --agent --type-at src/server.jett:45:12
```

```json
{
    "query": "type_at",
    "file": "src/server.jett",
    "line": 45,
    "column": 12,
    "result": {
        "expression": "user.email",
        "type": "string",
        "refinements": ["string.contains(value, \"@\")"],
        "defined_in": "src/models.jett:12"
    }
}
```

**Function signature lookup:**

```
jett query --agent --signature "string.split"
```

```json
{
    "query": "signature",
    "function": "string.split",
    "params": [
        {"name": "input", "type": "string"},
        {"name": "delimiter", "type": "string"}
    ],
    "returns": "list[string]",
    "capabilities": [],
    "module": "string",
    "doc": "Splits the input string at each occurrence of the delimiter"
}
```

**Available completions at a position:**

```
jett query --agent --complete-at src/server.jett:30:15
```

```json
{
    "query": "complete_at",
    "file": "src/server.jett",
    "line": 30,
    "column": 15,
    "context": {
        "in_function": "handle_request",
        "pipe_input_type": "User",
        "expecting": "function taking User as first argument"
    },
    "completions": [
        {"name": "json.serialize_public", "signature": "(value: view User) returns string"},
        {"name": "json.serialize", "signature": "BLOCKED — User contains secret fields"},
        {"name": "validate_user", "signature": "(user: User) returns result[User, string]"}
    ]
}
```

**Verify/test results** (`jett test` runs all `verify` and `property` blocks):

```
jett test --agent
```

```json
{
    "status": "fail",
    "total": 12,
    "passed": 11,
    "failed": 1,
    "results": [
        {
            "name": "verify calculate_discount",
            "status": "fail",
            "assertion": "calculate_discount(100.0, \"gold\") is 80.0",
            "expected": 80.0,
            "actual": 75.0,
            "file": "src/pricing.jett",
            "line": 15
        }
    ]
}
```

Every tool in the Jett toolchain — build, test, format, query — speaks JSON when asked. The LLM never has to parse human-formatted output.

#### ASP vs LSP — Complementary, Not Competing

Jett ships both:

- **LSP (Language Server Protocol)** — for human developers using editors (VS Code, Neovim, etc.). Real-time diagnostics, hover information, code completion, refactoring. Standard LSP that any editor can use.
- **ASP (Agent Server Protocol)** — for LLM agents using the compiler programmatically. Batch-oriented, JSON-based, deterministic, no streaming. Designed for the compile-fix-compile loop.

LSP is optimized for interactive, keystroke-by-keystroke human development. ASP is optimized for batch, generate-compile-fix LLM development. Both share the same underlying compiler engine.

#### Why This Is Perfect for LLMs

**1. Zero parsing overhead.**

The LLM receives structured JSON, not formatted text. It doesn't spend tokens decoding arrows, colors, or spatial layout. Every token in the payload carries semantic meaning.

**2. Exact AST node identification.**

The error payload includes the exact broken AST node. The LLM doesn't count lines or search for the error — the payload points directly to it, including the tree structure of the expression.

**3. Suggested fixes are apply-ready.**

The `suggested_fix` field contains the exact old text and new text. The LLM can apply the fix as a direct string replacement. No reasoning about what "might" fix the error — the compiler tells it exactly what to change.

**4. Scope information eliminates guessing.**

The `scope` field lists every variable available at the error location. The LLM doesn't have to "remember" what's in scope — the compiler tells it. This is especially valuable for deep nesting or long functions where the LLM's attention may have drifted from earlier variable declarations.

**5. The compile-fix loop converges fast.**

Because errors are precise and fixes are concrete, the LLM typically fixes each error in one iteration. A 5-error build usually converges in 5-6 compile cycles, not 15-20. This saves tokens, time, and money.

### Rule Set 22: Folder-Agnostic Flat Module System

#### The Problem: Directory Trees Are an LLM Hallucination Factory

Deep directory structures are one of the most common sources of broken LLM-generated code:

```
# LLMs constantly hallucinate import paths:
import "../../../models/user"           # wrong number of ../
import "src/main/controllers/auth"      # absolute path that doesn't exist
from "../../utils" import helpers       # correct yesterday, broken after a refactor
use crate::services::auth::login        # Rust path that may or may not be right
```

The problem has multiple dimensions:

- **Relative paths require counting.** The LLM must know exactly how many directories up (`../`) to go, which requires knowing the current file's position in the tree. LLMs cannot reliably count (Rule Set 12).
- **Deep trees consume context tokens.** Explaining `src/main/java/com/example/auth/controllers/LoginController.java` to an LLM uses dozens of tokens just for the path — tokens that carry zero semantic information.
- **Refactoring breaks paths.** Moving a file from `src/auth/` to `src/auth/v2/` breaks every import that referenced it by path. The LLM cannot know whether a path is still valid without seeing the current file tree.
- **Conventions vary.** Is it `src/`, `lib/`, `app/`? Is the module name the file name or the directory name? Does `index.jett` in a directory represent the directory? Every convention is another thing to hallucinate.

#### The Solution: The Compiler Ignores the File System

In Jett, the physical location of files on disk is **irrelevant to the module system**. All scoping is purely **namespace-driven**. The compiler scans all `.jett` files in the project and resolves modules by their declared namespace, not by their file path.

**Declaring a namespace:**

```
# File: auth.jett (or login_stuff.jett, or anything.jett — the filename doesn't matter)
namespace auth

function login(credentials: Credentials) returns result[Session, string]:
    # ...

function logout(session: Session) returns nothing:
    # ...
```

**Using it from any other file:**

```
# File: server.jett (or handlers.jett, or app.jett — doesn't matter)
namespace server

function handle_login(stdout: Stdout, request: Request) returns result[Response, string]:
    use auth
    let session = auth.login(request.credentials) handle error:
        return fail("login failed")
    Stdout.write(stdout, "user logged in")
    return ok(Response(status: 200, body: json.serialize_public(session)))
```

`use auth` works regardless of whether `auth.jett` is in the same directory, a subdirectory, or a completely different part of the project tree. The compiler resolves `auth` to whichever file declared `namespace auth`. The LLM never writes a file path in an import.

#### How Namespace Resolution Works

**1. The compiler scans the project.**

At build time, the compiler finds all `.jett` files in the project directory (recursively). For each file, it reads the `namespace` declaration at the top.

**2. Namespaces map to files — not paths.**

```
# These files can live anywhere in the project:
# /project/auth.jett           → namespace auth
# /project/models/user.jett    → namespace models.user
# /project/stuff/helpers.jett  → namespace helpers
# /project/db.jett             → namespace database

# The physical paths don't matter. Only the namespace declarations matter.
```

**3. Imports use namespace names, never file paths.**

```
use auth              # resolves to whichever file declared "namespace auth"
use models.user       # resolves to whichever file declared "namespace models.user"
use helpers           # resolves to whichever file declared "namespace helpers"
use database          # resolves to whichever file declared "namespace database"
```

**4. Duplicate namespaces are compile errors.**

```
# file_a.jett:
namespace auth

# file_b.jett:
namespace auth

# COMPILE ERROR: namespace "auth" is declared in both file_a.jett and file_b.jett
# hint: each namespace must be declared in exactly one file
```

One namespace, one file. No ambiguity about which `auth` is being imported.

#### Flat File Organization — The LLM Decides

Because the compiler doesn't care about directory structure, the LLM (or developer) can organize files however makes sense:

**Option A — everything flat in one directory:**

```
project/
    jett.proj
    auth.jett         # namespace auth
    database.jett     # namespace database
    models.jett       # namespace models
    server.jett       # namespace server
    handlers.jett     # namespace handlers
```

**Option B — grouped by feature:**

```
project/
    jett.proj
    auth/
        auth.jett         # namespace auth
        credentials.jett  # namespace auth.credentials
    data/
        database.jett     # namespace database
        models.jett       # namespace models
    web/
        server.jett       # namespace server
        handlers.jett     # namespace handlers
```

**Option C — everything in one file (for small projects):**

```
project/
    jett.proj
    app.jett          # namespace auth, namespace models, etc. — multiple namespaces in one file
```

All three options produce identical builds. The imports (`use auth`, `use database`) work the same way in all three. The LLM never has to know or specify which option the project uses.

#### Multiple Namespaces in One File

For small projects or tightly coupled modules, multiple namespaces can live in a single file:

```
# File: app.jett

namespace models

struct User:
    id: string
    name: string
    email: string

namespace auth

function authenticate(name: string, password: string) returns result[models.User, string]:
    use models
    # ...

namespace server

function main(stdout: Stdout, net: Network) returns nothing:
    use auth
    use models
    # ...
```

The compiler treats each `namespace` block as a separate module. Other files can `use models`, `use auth`, or `use server` without knowing they all live in the same file. All `use` statements are inside functions, consistent with the inline-only import rule.

#### Single-File Libraries

Multiple namespaces in one file is the foundation for distributable libraries. Since Jett's dependency system imports external code via single-file URLs with cryptographic hashes (Rule Set 14), a library that spans multiple namespaces must be distributed as a single file:

```
# File: https://packages.jett-lang.org/v1.0/http_toolkit.jett
# This single file IS the entire library.

namespace http_toolkit.client

struct HttpRequest:
    method: string
    url: string
    headers: map[string, string]

function get(net: Network, url: string) returns result[Response, HttpToolkitError]:
    # ...

namespace http_toolkit.server

function listen(net: Network, port: int) returns result[Listener, HttpToolkitError]:
    # ...

namespace http_toolkit.errors

enum HttpToolkitError:
    connection_failed(message: string)
    timeout(message: string)
    status_error(code: int, message: string)
```

A consumer imports this single file and gets access to all its namespaces:

```
# In jett.lock:
# http_toolkit:
#     url = "https://packages.jett-lang.org/v1.0/http_toolkit.jett"
#     hash = "sha256:a1b2c3..."

function main(net: Network, stdout: Stdout) returns nothing:
    use http_toolkit.client
    use http_toolkit.errors
    let response = client.get(net, "https://example.com") handle error:
        Stdout.write(stdout, "failed: {error}")
        return
    Stdout.write(stdout, response.body)
```

**The bundle tool** — for library authors who develop across multiple files, the compiler provides a bundling command:

```
jett bundle --output my_library.jett
```

This concatenates all project files into a single distributable file, preserving all namespace declarations. The result is a self-contained `.jett` file that can be hosted at any URL and imported by other projects. Library authors develop with whatever file organization they prefer, then bundle for distribution.

#### Sub-Namespaces for Large Projects

Hierarchical namespaces use dot notation for logical grouping:

```
# File: http_server.jett
namespace net.http.server

# File: http_client.jett
namespace net.http.client

# File: tcp.jett
namespace net.tcp
```

```
# Importing:
use net.http.server     # imports the server module
use net.http.client     # imports the client module
use net.tcp             # imports the tcp module

# Or import the parent to get everything:
use net.http            # imports both server and client
```

The dot notation is purely logical — it does not imply a directory structure. `net.http.server` can live in a file called `http_server.jett` in the root directory, or `server.jett` in a `net/http/` subdirectory, or `everything.jett` alongside 10 other namespaces.

#### Inline Imports Remain — Now with Namespaces

The inline `use` from Rule Set 4 works with namespaces:

```
namespace handlers

function process_payment(net: Network, order: Order) returns result[Receipt, string]:
    use auth                # inline import — binds to "auth"
    use payment.gateway     # inline import — binds to "gateway"

    let session = auth.validate_token(order.token) handle error:
        return fail("auth failed")
    let receipt = gateway.charge(net, order.total) handle error:
        return fail("payment failed")
    return ok(receipt)
```

Inline `use` keeps dependencies local to the function (Rule Set 4). Namespace resolution makes the import path-free (Rule Set 22). Together, they create self-contained functions with zero directory knowledge.

#### Import Binding and Conflict Resolution

When you write `use net.http`, the import binds to the **last segment** of the namespace — `http`. You call functions with `http.get(...)`, not `net.http.get(...)`.

```
function fetch(net: Network) returns result[string, HttpError]:
    use net.http
    let response = http.get(net, "https://example.com") handle error:
        return fail(error)
    return ok(response.body)
```

If two imports share the same last segment, the compiler produces an error and requires the `as` keyword to disambiguate:

```
function fetch_both(net: Network) returns nothing:
    use net.http
    use tor.http
    # COMPILE ERROR: import name conflict — "http" is bound by both
    #   "net.http" and "tor.http"
    # hint: use the "as" keyword to rename one or both imports:
    #   use net.http as net_http
    #   use tor.http as tor_http
```

The fix — explicit aliasing with `as`:

```
function fetch_both(net: Network, stdout: Stdout) returns nothing:
    use net.http as net_http
    use tor.http as tor_http

    let clearnet = net_http.get(net, "https://example.com") handle error:
        Stdout.write(stdout, "clearnet failed: {error}")
        return
    let onion = tor_http.get(net, "http://example.onion") handle error:
        Stdout.write(stdout, "tor failed: {error}")
        return
    Stdout.write(stdout, "both fetched")
```

The `as` keyword works uniformly across all import types — namespace imports, external URL imports (`use "url" as name`), and C interop imports (`use c "header.h" as name`). One pattern for all cases.

#### What the Compiler Rejects

**No file paths in imports — ever:**

```
use "../models/user"
# COMPILE ERROR: imports must use namespace names, not file paths
# hint: use "models.user" instead

use "src/auth.jett"
# COMPILE ERROR: imports must use namespace names, not file paths
# hint: use "auth" instead
```

**No relative paths, no absolute paths, no file extensions.** The `use` keyword takes a namespace name and nothing else.

**No circular imports:**

```
# auth.jett:
namespace auth
use models       # OK — auth depends on models

# models.jett:
namespace models
use auth          # COMPILE ERROR: circular import detected
                  # "models" and "auth" depend on each other
                  # hint: extract shared definitions into a third namespace
```

Circular imports are a compile error. If two namespaces need each other, extract the shared part into a third namespace that both can import. This forces a clean dependency hierarchy and fits the strict topological ordering principle — the compiler can always process namespaces in dependency order.

#### Why This Is Perfect for LLMs

**1. Zero path hallucination.**

The LLM writes `use auth`, not `use "../../../src/auth/login"`. There are no relative paths to miscalculate, no directory depths to count, no file extensions to remember. The namespace name is the only identifier.

**2. Refactoring never breaks imports.**

Moving `auth.jett` from `src/` to `src/v2/` changes nothing. The file still declares `namespace auth`. Every `use auth` in the project still works. The LLM never has to update import paths after a file move.

**3. The LLM doesn't need to know the project structure.**

To import a module, the LLM only needs to know its namespace name. It doesn't need the file tree in its context window. This saves tokens and eliminates an entire category of context the LLM would otherwise need.

**4. One flat list of available modules.**

The ASP (Rule Set 21) can provide the LLM with a flat list of available namespaces:

```
jett query --agent --namespaces
```

```json
{
    "namespaces": [
        {"name": "auth", "file": "auth.jett", "public_functions": ["login", "logout", "validate_token"]},
        {"name": "models", "file": "models.jett", "public_types": ["User", "Order", "Product"]},
        {"name": "database", "file": "database.jett", "public_functions": ["query", "insert", "update"]}
    ]
}
```

The LLM receives a flat, structured list of every module in the project. No tree parsing, no directory traversal, no path construction. Just names and what they contain.

**5. File organization is a human decision, not an LLM burden.**

Humans can organize files into directories however they prefer for their own readability. The LLM is completely unaffected by this choice. It writes `use auth` whether the project has 1 directory or 50.

### Rule Set 23: Token-Safe Bitwise and Hardware Operations

#### The Problem: LLMs Are Mathematically Blind to Bits

LLMs do not understand binary representation. Because of tokenization, an LLM has no way to know that `0x1F` and `31` are the same value, that `>> 4` divides by 16, or that `& 0x0F` extracts the low nibble. These are not semantic facts embedded in the tokens — they are mathematical relationships that require computation the neural architecture cannot perform.

When LLMs write bitwise code, they hallucinate constantly:

- **Wrong shift amounts.** `>> 4` when it should be `>> 3`. The LLM is pattern-matching from training data, not computing.
- **Wrong masks.** `& 0xFF` when the field is only 4 bits wide (should be `& 0x0F`). Hex constants are opaque to the LLM.
- **Wrong endianness.** Network byte order (big-endian) vs host byte order (little-endian). The LLM cannot reason about byte swapping.
- **Conflated representations.** `0b11110000`, `0xF0`, and `240` are the same value. The LLM may generate one when it means another.
- **Incorrect bit extraction.** Extracting bits 4-7 from a byte requires `(value >> 4) & 0x0F`. The LLM must know the shift, the mask, and the field width — three values that must be mathematically consistent. It will get at least one wrong.

This matters for native-performance applications: network protocol parsing, hardware drivers, graphics programming, compression algorithms, and cryptographic operations all require precise bit manipulation.

#### The Solution: Declarative Bitfields Replace Bitwise Operators

Jett **completely abolishes traditional bitwise operators** (`&`, `|`, `<<`, `>>`, `^`, `~`). They do not exist in the language. There is no way to write `(packet >> 4) & 0x0F`.

Instead, Jett provides **declarative bitfield structs** where the LLM describes memory layouts using plain English field names and base-10 integer sizes. The compiler generates all the bit-shifting, masking, and extraction code.

**Declaring a bitfield:**

```
bitfield TcpFlags:
    fin: 1 bit
    syn: 1 bit
    rst: 1 bit
    psh: 1 bit
    ack: 1 bit
    urg: 1 bit
    ece: 1 bit
    cwr: 1 bit
```

```
bitfield IpHeader:
    version: 4 bits
    header_length: 4 bits
    dscp: 6 bits
    ecn: 2 bits
    total_length: 16 bits
    identification: 16 bits
    flags: 3 bits
    fragment_offset: 13 bits
    ttl: 8 bits
    protocol: 8 bits
    checksum: 16 bits
    source_address: 32 bits
    destination_address: 32 bits
```

The LLM writes field names and bit widths using **base-10 integers** — which it understands perfectly. No hex constants, no shift amounts, no masks. The compiler computes all of it.

#### Using Bitfields — Pure Field Access

Reading and writing bitfield values uses the same dot-access syntax as regular structs:

```
function parse_ip_packet(raw: bytes) returns result[IpHeader, string]:
    let header = IpHeader.from_bytes(raw) handle error:
        return fail("invalid IP header")

    if header.version != 4:
        return fail("not IPv4")

    if header.ttl is 0:
        return fail("TTL expired")

    return ok(header)
```

The LLM writes `header.version`, `header.ttl`, `header.protocol`. It never writes `(raw[0] >> 4) & 0x0F` to extract the version field. The compiler generates the bit extraction automatically from the bitfield declaration.

**Writing bitfield values:**

```
function create_tcp_flags(syn: bool, ack: bool) returns TcpFlags:
    return TcpFlags(
        fin: 0, syn: int(syn), rst: 0, psh: 0,
        ack: int(ack), urg: 0, ece: 0, cwr: 0
    )
```

The LLM sets fields by name. The compiler packs them into the correct bit positions.

#### Bitfields with Enums — Named Values for Bit Patterns

Common bit patterns get named values instead of magic numbers:

```
enum IpProtocol:
    tcp = 6
    udp = 17
    icmp = 1

bitfield IpHeader:
    version: 4 bits
    header_length: 4 bits
    # ...
    protocol: 8 bits as IpProtocol
    # ...
```

```
function is_tcp(header: view IpHeader) returns bool:
    return header.protocol is IpProtocol.tcp
```

The LLM writes `IpProtocol.tcp` — not `6`, not `0x06`, not `0b00000110`. The name carries the meaning. The compiler handles the numeric encoding.

#### Bitfields with Refinement Types

Bit widths naturally constrain value ranges. The compiler auto-generates refinement constraints:

```
bitfield ColorChannel:
    red: 8 bits      # auto-constrained: 0 to 255
    green: 8 bits     # auto-constrained: 0 to 255
    blue: 8 bits      # auto-constrained: 0 to 255
    alpha: 8 bits     # auto-constrained: 0 to 255

let color = ColorChannel(red: 300, green: 128, blue: 0, alpha: 255)
# COMPILE ERROR: field "red" is 8 bits wide (range 0 to 255), but value is 300
```

The compiler knows that an 8-bit field cannot hold 300. The LLM doesn't need to know the range — the bitfield declaration implies it, and the compiler enforces it.

#### Variable-Length and Conditional Fields

For protocols with variable-length sections:

```
bitfield DnsHeader:
    id: 16 bits
    qr: 1 bit
    opcode: 4 bits
    aa: 1 bit
    tc: 1 bit
    rd: 1 bit
    ra: 1 bit
    z: 3 bits
    rcode: 4 bits
    qdcount: 16 bits
    ancount: 16 bits
    nscount: 16 bits
    arcount: 16 bits
    payload: remaining
```

The `remaining` keyword captures everything after the fixed-size fields as a raw byte slice. The LLM can then parse the payload section using further bitfield declarations or standard library functions.

#### Replacing Every Bitwise Operation

| Traditional (LLM-hostile) | Jett bitfield (LLM-friendly) |
|--------------------------|----------------------------|
| `(value >> 4) & 0x0F` | `header.version` (4-bit field) |
| `value \| (1 << 5)` | `TcpFlags(..., ack: 1, ...)` — construct a new TcpFlags with the modified field |
| `value & ~(0xFF << 8)` | Direct field assignment — compiler handles masking |
| `htons(port)` / `ntohs(port)` | Compiler handles byte order based on `layout network_order` annotation |
| `memcpy(&header, buffer, sizeof(header))` | `Header.from_bytes(buffer)` |
| `0x1F`, `0b00011111`, `31` | A field width: `field: 5 bits` |

No hex literals, no binary literals, no shift operators, no mask operators. The LLM works entirely in base-10 integers and field names.

#### Byte Order Annotation

Network protocols use big-endian (network byte order). Hardware registers may use little-endian. The LLM declares the byte order once on the bitfield — not per-field:

```
bitfield TcpHeader layout network_order:
    source_port: 16 bits
    dest_port: 16 bits
    sequence_number: 32 bits
    ack_number: 32 bits
    data_offset: 4 bits
    reserved: 3 bits
    flags: 9 bits
    window_size: 16 bits
    checksum: 16 bits
    urgent_pointer: 16 bits
```

`layout network_order` tells the compiler that all multi-byte fields are big-endian. The compiler automatically inserts byte-swap operations when reading/writing on a little-endian host. The LLM never calls `htons()` or `ntohl()`.

#### Serialization Integration

Bitfields get the same auto-generated serialization as regular structs (Rule Set 18):

```
let header = IpHeader.from_bytes(raw_packet) handle error:
    return fail("invalid header")

let serialized = json.serialize(header)
# {"version":4,"header_length":5,"dscp":0,"ecn":0,"total_length":60,...}

let bytes = IpHeader.to_bytes(header)
# Exact binary representation, bit-packed, correct byte order
```

The LLM can convert between wire format (bytes), structured data (bitfield), and human-readable format (JSON) with single function calls.

#### Why This Is Perfect for LLMs

**1. Base-10 integers only.**

The LLM writes `4 bits`, `16 bits`, `32 bits` — numbers it can reason about. No hex (`0x0F`), no binary (`0b1111`), no octal (`017`). Base-10 is the only number format in bitfield declarations.

**2. Field names carry all semantics.**

`header.source_port` is self-documenting. `(raw[0] << 8) | raw[1]` is not. The LLM writes English. The compiler writes machine code.

**3. The compiler computes everything the LLM can't.**

Shift amounts, mask values, byte order conversion, field packing — all computed at compile time from the declarative bitfield definition. The LLM specifies *what* the layout is. The compiler handles *how* to access it.

**4. Validation is automatic.**

An 8-bit field rejects values above 255. A 4-bit field rejects values above 15. The compiler knows the range from the bit width. The LLM cannot overflow a field.

**5. One format for all hardware interaction.**

Network protocols, file format headers, hardware registers, graphics pixel formats — all use the same `bitfield` syntax. One pattern for the LLM to learn, covering every low-level binary task.

### Rule Set 24: Read-Only Views (Solving the Memory-Borrowing Problem)

#### The Problem: Linear Typing Demands Cloning for Read Access

Rule Set 10.1 established linear typing: when a variable is passed to a function, it is consumed (moved) and becomes invalid in the caller's scope. This is excellent for memory safety — it gives the compiler perfect knowledge of ownership with zero hidden pointers.

But there is a performance problem. If the LLM has a 10GB data structure and wants to pass it to a function that only reads its `.length` field, linear typing forces a choice:

1. **Move it.** The data moves to the callee. The caller loses access. The callee must return it as part of its return type to give it back. This works but creates verbose plumbing.
2. **Clone it.** `Linear.clone(data)` copies the entire 10GB structure just to read one field. This is absurdly wasteful.

Other languages solve this with borrowing and lifetimes. Rust uses `&T` (immutable reference) and `&'a T` (lifetime-annotated reference). But Rust's lifetime syntax is notoriously complex:

```
// Rust lifetime syntax — LLM-hostile:
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str
fn process<'a, 'b>(data: &'a mut Vec<&'b str>) -> &'a str where 'b: 'a
```

Lifetime annotations split the LLM's attention across multiple scopes, require tracking relationships between lifetimes (`'b: 'a` means `'b` outlives `'a`), and introduce a secondary type annotation syntax that LLMs consistently get wrong. This violates Rule Set 1 (tokenizer-friendly keywords) and Rule Set 6 (attention-splitting ambiguity).

#### The Solution: Lexical Views — Zero-Copy Reads Without Lifetimes

Jett introduces one concept: the **view**. A view is a read-only, non-owning reference to data that is **lexically scoped** — it cannot outlive the block it was created in. No lifetime annotations. No borrowing syntax. No `'a`. One keyword, one rule.

**Passing a view:**

```
function count_items(data: view list[Item]) returns int:
    return list.length(data)

function total_price(items: view list[Item]) returns float:
    let mutable sum = 0.0
    for item in view items:
        sum = sum + item.price
    return sum
```

The `view` keyword before the type means: "this function can read this data but cannot consume it, mutate it, or store it." The caller retains ownership. No data is copied. No data is moved.

**Calling with a view:**

```
function process_order(order: Order) returns nothing:
    # order.items is NOT consumed — it is viewed:
    let count = count_items(order.items)
    let total = total_price(order.items)

    # order.items is still valid here — it was never moved:
    let first = list.first(order.items) handle:
        return

    # Now consume it when we're done reading:
    submit_order(order)
    # order is consumed here — moved into submit_order
    return
```

The `view` keyword only appears in declarations (parameter types and for-loop bindings), not at call sites. The compiler knows from the function signature whether a parameter is a view and handles it automatically.

**General rule: `view` only appears in declarations.** When a function declares a `view` parameter, the compiler automatically treats the argument as a view at the call site. The caller writes `json.serialize(user)`, not `json.serialize(view user)`. The `view` keyword appears only in the parameter declaration (`data: view list[Item]`) and in for-loop bindings (`for item in view items`). This keeps call sites clean while the function signature communicates the read-only intent.

#### The Three Strict Rules of Views

Views are governed by three rules that the compiler enforces absolutely. These rules are deliberately restrictive to keep the concept simple and make violations impossible.

**Rule 1: A view cannot be mutated.**

```
function bad_mutate(data: view list[int]) returns nothing:
    list.append(data, 42)
    # COMPILE ERROR: cannot mutate a view
    # "data" is a read-only view and cannot be modified
    # hint: if mutation is needed, take ownership instead of a view
```

A view is read-only. Period. Any operation that would modify the data — append, remove, set, sort in place — is a compile error on a view parameter.

**Rule 2: A view cannot be sent to another thread.**

```
function bad_send(data: view list[int]) returns nothing:
    let worker = spawn Processor()
    send worker process(data)
    # COMPILE ERROR: cannot send a view to an actor
    # views are confined to the current thread
    # hint: clone the data or move ownership to the actor instead
```

Views exist only on the stack of the current thread. They cannot be sent to actors (Rule Set 10.3), put into channels, or stored in any structure that crosses thread boundaries. This eliminates data races without any synchronization overhead.

**Rule 3: A view cannot outlive its lexical scope.**

```
function bad_escape(data: view list[int]) returns view list[int]:
    return data
    # COMPILE ERROR: cannot return a view from a function
    # views cannot outlive the function that received them
    # hint: take ownership if the caller needs the data returned

let mutable stored_view: view list[int]
# COMPILE ERROR: views cannot be stored in variables with broader scope
# a view exists only within the function call or block where it was created
```

A view cannot be returned, cannot be stored in a struct field, cannot be assigned to a variable in an outer scope, and cannot be captured in a closure. It exists for exactly one purpose: reading data within the current function, then disappearing. This is why no lifetime annotations are needed — the lifetime is always "this function call, no more."

#### Why No Lifetime Annotations

In Rust, lifetime annotations exist to tell the compiler how long a reference must be valid when the answer is not obvious from the code structure. Jett's views eliminate the ambiguity entirely:

| Rust (needs lifetimes) | Jett (no lifetimes needed) |
|------------------------|--------------------------|
| A reference returned from a function — which input does it borrow from? | Views cannot be returned. Question eliminated. |
| A reference stored in a struct — how long must the source live? | Views cannot be stored. Question eliminated. |
| Two references with different lifetimes — which outlives which? | Views exist for one function call. No overlapping lifetimes possible. |
| A reference passed to a closure — does the closure outlive the source? | Views cannot be captured. Question eliminated. |

Every situation that requires lifetime annotations in Rust is structurally impossible in Jett. The three rules prevent every case. The compiler doesn't need annotations because the answer is always the same: the view lives for this function call and dies when the function returns.

#### Views with Structs — Reading Fields Without Cloning

```
struct GameState:
    players: list[Player]
    world: World
    tick: int

function render_frame(state: view GameState, stdout: Stdout) returns nothing:
    # Read any field through the view — zero copy:
    let player_count = list.length(state.players)
    Stdout.write(stdout, "players: {player_count}")
    Stdout.write(stdout, "tick: {state.tick}")

    for player in view state.players:
        # `player` is also a view — views propagate through field access:
        render_player(player, stdout)

function game_loop(stdout: Stdout) returns nothing:
    let mutable state = GameState(players: list(), world: World(), tick: 0)

    while true:
        # Pass a view for rendering (read-only, zero-copy):
        render_frame(state, stdout)

        # Then mutate the owned state:
        state = update_game(state)
```

`render_frame` receives a view of the entire game state. It can read every field, iterate through players, access nested structures — all without copying a single byte. When it returns, the game loop still owns `state` and can mutate it.

> **Note:** Views propagate through access. If you have a `view list[T]`, accessing any element gives you a `view T`, not an owned copy. The same applies to struct fields, nested lists, and any sub-structure. To get an owned value from a view, you must explicitly clone with `Linear.clone()`.
>
> ```
> function example(data: view list[Item], stdout: Stdout) returns nothing:
>     for item in view data:
>         # item is view Item — read-only, not copied
>         Stdout.write(stdout, item.name)    # OK — reading a field
>
>     let first = list.first(data)
>     # first is view Item — still a view, not an owned copy
>
>     let owned = Linear.clone(first)
>     # NOW it's an owned copy — explicit
> ```

#### Views with the Pipeline Operator

Views work naturally with pipelines (Rule Set 19). When a pipeline step pipes into a function that declares a `view` parameter, the compiler automatically handles the view semantics. No special syntax is needed at the pipeline call site:

```
let report = large_dataset
    |> filter_active_records
    |> calculate_summary
    |> json.serialize
```

The compiler knows that `json.serialize` declares a `view` parameter and automatically treats the piped value as a view. No `view` keyword is needed in the pipeline step. Transform functions like `filter_active_records` consume their input and produce a new value. Read-only functions like `json.serialize` take a `view` parameter. The compiler handles the distinction automatically. The types are checked at each `|>` boundary as usual.

#### Views and Capabilities

View parameters work alongside capability parameters:

```
function log_stats(stdout: Stdout, state: view GameState) returns nothing:
    Stdout.write(stdout, "players: {list.length(state.players)}")
    Stdout.write(stdout, "world size: {state.world.size}")
    # stdout is owned (capability), state is viewed (read-only)
    # The function can write to stdout but cannot modify state
```

#### Performance: Zero-Copy Reads at C Speed

Under the hood, a view is a pointer to the original data. No copying, no reference counting, no garbage collection overhead. Reading through a view is exactly as fast as dereferencing a raw pointer in C — because that is exactly what the compiled code does.

The safety comes not from runtime checks but from the three compile-time rules. The compiler statically proves that:

- The data cannot be freed while a view to it exists (because views can't outlive their scope).
- The data cannot be mutated while a view to it exists (because views are read-only and the owner can't mutate while a view is active).
- No data race can occur (because views can't cross thread boundaries).

This is the same level of safety as Rust's borrow checker, achieved with zero annotation overhead.

#### The Complete Ownership Model

With views, Jett's ownership model has exactly three modes:

| Mode | Keyword | What it means | When to use |
|------|---------|--------------|-------------|
| **Own** | (default) | Value is moved. Caller loses it. | When the function needs to consume, store, or modify the data. |
| **View** | `view` | Read-only reference. Caller keeps ownership. | When the function only needs to read. Zero-copy, zero-cost. |
| **Clone** | `Linear.clone()` | Deep copy. Both sides have independent copies. | When both caller and callee need independent ownership. |

Three modes, three keywords, zero lifetime annotations. The LLM chooses between them based on one simple question: does this function need to modify or keep the data?

- **Yes, modify or keep** → pass normally (move).
- **No, just read** → pass as `view`.
- **Both need their own copy** → `Linear.clone()`.

#### Why This Is Perfect for LLMs

**1. One keyword replaces all of Rust's borrowing syntax.**

No `&`, `&mut`, `&'a`, `&'a mut`, `&'static`. Just `view`. One concept, one word, one rule: read-only, dies when the function returns.

**2. The decision is trivially simple.**

"Does this function need to modify the data?" If no → `view`. If yes → move. The LLM doesn't need to reason about lifetimes, borrow scopes, or ownership transfer chains. The question has a binary answer.

**3. Zero-copy performance with zero annotation cost.**

The LLM gets C-level performance (pointer dereference, no copying) without writing any unsafe code or lifetime annotations. The compiler guarantees safety from the three structural rules.

**4. Views are explicit in declarations.**

`count_items(data: view list[Item])` — the `view` keyword in the parameter declaration tells the LLM (and the reader) exactly what is happening. There is no implicit borrowing, no hidden reference creation. The declaration says what it does.

**5. No lifetime errors — the most common Rust stumbling block eliminated.**

Lifetime errors are the single most common compilation failure in Rust. They are also the hardest for LLMs to fix because the errors reference abstract lifetime relationships (`'a does not live long enough`). Jett eliminates the concept entirely. There are no lifetime errors because there are no lifetimes.

> **Note: Memory optimization through linear types.** Because Jett enforces single ownership, the compiler always knows that a value has no other references (except read-only views). This enables aggressive in-place memory reuse without runtime checks:
>
> - **Consuming transforms**: `x = transform(x)` — `x` is consumed, so `transform` can mutate the underlying memory in-place and return it. It looks like a new value is created, but the compiled code reuses the same allocation.
> - **List operations**: `list.append(old_list, item)` — `old_list` is consumed, so the compiler can append in-place to the existing buffer. No need to copy the entire list.
> - **Struct updates**: Returning a modified struct after consuming the original — the compiler can update fields in-place since it knows the original is dead.
> - **Views are zero-cost**: Reading through a view is just pointer dereferencing. No allocation, no reference counting, no copies.
> - **`Linear.clone()` is the only real copy**: Actual memory duplication only happens when the programmer explicitly requests it — and that cost is visible in the source code.
>
> The immutable-looking style is actually more memory-efficient than languages with mutable aliasing, because the compiler has perfect ownership knowledge and never needs defensive copies.

### Rule Set 25: Native Property-Testing (Fuzzing) Over Unit Testing

#### The Problem: LLMs Only Test the Happy Path

When an LLM writes unit tests (or `verify` blocks from Rule Set 13), it writes the patterns it saw most often in training data:

```
verify add_positive:
    assert add_positive(2, 3) is 5
    assert add_positive(0, 0) is 0
    assert add_positive(-1, 1) is 0
```

These are correct — but they are exclusively "normal" inputs. The LLM will not think to test:

- **Integer overflow:** `2147483647 + 1` — what happens at the maximum integer boundary?
- **Extremely large inputs:** a list with 10 million elements passed to a sort function.
- **Empty inputs:** an empty string passed to a parser, an empty list passed to `list.first`.
- **Adversarial inputs:** strings with null bytes, unicode edge cases, negative zero, NaN.
- **Boundary conditions:** off-by-one at array bounds, exactly-at-limit values for refinement types.

LLMs are bad at edge cases because edge cases are rare in training data. The neural architecture pattern-matches on common examples, not on adversarial ones. This leaves massive blind spots — the LLM's `verify` block passes, but the function fails in production on an input the LLM never imagined.

#### The Solution: Property-Based Testing as a Language Primitive

Jett adds `property` blocks alongside `verify` blocks. A `property` block does not specify individual inputs and expected outputs. Instead, the LLM declares **the rules that must always hold** — the invariants, the relationships, the properties of the function's behavior. The compiler's built-in fuzzer then bombards the function with thousands of random, edge-case, and adversarial inputs to find violations.

**Basic property test:**

```
function sort_list(items: view list[int]) returns list[int]:
    # ... sorting implementation ...

property sort_list:
    given items: list[int]
    let sorted = sort_list(items)
    assert list.length(sorted) is list.length(items)
    assert list.is_sorted(sorted)
    assert list.all_elements_in(sorted, items)
```

The `property` block declares:

1. **`given`** — the randomly generated inputs. The fuzzer knows the type (`list[int]`) and generates thousands of variations: empty lists, single-element lists, already-sorted lists, reverse-sorted lists, lists with duplicates, lists with maximum/minimum integers, extremely long lists.
2. **`assert`** — the properties that must hold for every generated input. The sorted list must have the same length, must be ordered, and must contain exactly the same elements.

The LLM does not choose specific inputs. The LLM declares what "correct" means. The CPU does the testing.

#### How the Fuzzer Works

When the developer or LLM runs `jett test`, the compiler:

1. Finds all `property` blocks.
2. For each `given` parameter, generates inputs using type-aware random generation:

| Type | What the fuzzer generates |
|------|--------------------------|
| `int` | 0, 1, -1, max_int, min_int, random positive, random negative, powers of 2, boundary values |
| `float` | 0.0, -0.0, 1.0, -1.0, very small (epsilon), very large, max_float, min_float, infinity, negative infinity, NaN |
| `string` | empty, single char, ASCII, unicode, multi-byte characters, very long strings, strings with null bytes, whitespace-only |
| `list[T]` | empty, single element, two elements, many elements, duplicates, sorted, reverse-sorted, all-same |
| `bool` | true, false |
| `optional[T]` | none, some(generated T) |
| Refinement types | Values at boundaries (just above minimum, just below maximum, exactly at limits) |
| Enums | Every variant, including variants with generated data |
| Custom structs | All fields generated recursively using the above rules |

3. Runs the property block with each generated input set.
4. If any assertion fails, the fuzzer **shrinks** the input to find the minimal failing case.
5. Reports the minimal failing input via the ASP (Rule Set 21).

**Default: 10,000 random inputs per property block.** Because Jett compiles to native speed (Rule Set 10), 10,000 test iterations execute in milliseconds.

#### Shrinking — Finding the Minimal Failing Input

When the fuzzer finds a failure, the raw random input is often large and confusing (e.g., a list with 847 elements). The fuzzer automatically **shrinks** the input to the smallest example that still triggers the failure:

```
# Fuzzer found failure with: [483, -2, 0, 17, -99, 42, 0, 8, ...]  (847 elements)
# Shrinking...
# Minimal failing input: [1, 0]
```

The ASP output for a property failure:

```json
{
    "status": "property_failure",
    "property": "sort_list",
    "file": "src/sorting.jett",
    "line": 25,
    "failed_assertion": "list.is_sorted(sorted)",
    "minimal_input": {
        "items": [1, 0]
    },
    "actual_output": {
        "sorted": [1, 0]
    },
    "iterations_before_failure": 42,
    "shrink_steps": 15,
    "explanation": "sort_list([1, 0]) produced [1, 0] which is not sorted"
}
```

The LLM receives the **minimal failing input** — the simplest case that breaks the function. This is vastly more useful than "failed on a list with 847 elements." The LLM can immediately see that `sort_list([1, 0])` returns `[1, 0]` (unsorted) and fix the bug.

#### Property Tests + Verify Blocks — Two Layers of Correctness

`verify` and `property` are complementary:

- **`verify`** — specific input/output pairs, executed at compile time (comptime). Proves the function is correct for known examples. Fast, deterministic, zero overhead.
- **`property`** — invariant declarations, executed by the fuzzer at test time. Proves the function is correct for thousands of unknown examples. Finds the edge cases the LLM didn't imagine.

```
function clamp(value: int, low: int, high: int) returns int:
    if value < low:
        return low
    if value > high:
        return high
    return value

verify clamp:
    assert clamp(5, 0, 10) is 5
    assert clamp(-1, 0, 10) is 0
    assert clamp(15, 0, 10) is 10
    assert clamp(0, 0, 10) is 0
    assert clamp(10, 0, 10) is 10

property clamp:
    given value: int, low: int, high: int
    where low <= high
    let result = clamp(value, low, high)
    assert result >= low
    assert result <= high
    if value >= low and value <= high:
        assert result is value
```

The `verify` block proves 5 specific cases at compile time. The `property` block proves the invariants hold for 10,000 random `(value, low, high)` triples — including integer boundaries, negative numbers, and extreme ranges the LLM would never think to test.

#### The `where` Clause — Preconditions for Generated Inputs

The `where` clause in a `property` block filters generated inputs to only valid combinations:

```
property divide:
    given a: int, b: int
    where b != 0
    let result = a / b
    assert result * b is a
```

**Note: This is an INTENTIONAL example of property testing catching a bug.** The assertion `result * b is a` does NOT hold for integer division when `a` is not evenly divisible by `b`. For example, `7 / 2 = 3` (integer division truncates), then `3 * 2 = 6`, and `6 != 7`. The fuzzer will quickly find a counterexample like `(a=7, b=2)` and report a failure. This demonstrates a key strength of property testing: it catches mathematical assumptions that humans (and LLMs) miss. The programmer assumed division is the inverse of multiplication, but that only holds for exact division. The correct property would be `result * b + (a modulo b) is a`.

The fuzzer only generates cases where `b` is not zero. The `where` clause expresses a precondition — the LLM states what inputs are valid, and the fuzzer respects it.

```
property percentage:
    given score: int, total: int
    where total > 0
    where score >= 0
    where score <= total
    let pct = calculate_percentage(score, total)
    assert pct >= 0.0
    assert pct <= 100.0
```

Multiple `where` clauses compose. The fuzzer generates only inputs that satisfy all of them.

#### Property Tests with State Machines

Properties can test state machine transitions (Rule Set 9):

```
function apply_auth_action(session: UserAuth, action: AuthAction, user_id: string) returns UserAuth:
    match action:
        login_attempt:
            if session at guest:
                let s = UserAuth.transition(session, authenticating, user_id: user_id)
                return UserAuth.transition(s, logged_in, user_id: user_id)
            return session
        logout:
            if session at logged_in:
                return UserAuth.transition(session, guest)
            return session
        ban:
            if session at logged_in:
                return UserAuth.transition(session, banned, user_id: session.user_id)
            return session

property user_auth_lifecycle:
    given actions: list[AuthAction], user_id: string
    let mutable session = UserAuth(guest)
    for action in actions:
        session = apply_auth_action(session, action, user_id)
    # After any sequence of actions, the session is in a valid state:
    assert session at guest or session at authenticating or session at logged_in or session at banned
```

The fuzzer generates random sequences of actions and verifies that the state machine never reaches an invalid state.

#### Property Tests with Serialization

Properties naturally verify serialization round-trips (Rule Set 18):

```
property json_round_trip:
    given user: User
    let json_string = json.serialize(user)
    let restored = json.parse(json_string, User) handle error:
        assert false "round-trip failed: json.parse returned error"
    assert restored is user
```

The fuzzer generates thousands of random `User` structs with random field values, serializes each to JSON, deserializes it back, and verifies perfect equality. This catches encoding bugs, missing fields, and type conversion errors that no LLM-written unit test would find.

#### ASP Integration — Failures Feed Back to the LLM

Property failures integrate with the Agent Server Protocol (Rule Set 21):

```
jett test --agent
```

```json
{
    "status": "property_failure",
    "property": "json_round_trip",
    "file": "src/models.jett",
    "line": 45,
    "failed_assertion": "restored is user",
    "minimal_input": {
        "user": {"id": "a", "name": "", "email": "x@y", "age": 0}
    },
    "expected": {"id": "a", "name": "", "email": "x@y", "age": 0},
    "actual": {"id": "a", "name": null, "email": "x@y", "age": 0},
    "explanation": "empty string \"\" was deserialized as null instead of empty string"
}
```

The LLM receives: the minimal input that breaks the code, the expected vs actual output, and a plain-English explanation. It fixes the specific bug and re-runs `jett test`. The CPU found the edge case. The LLM fixed it. Neither had to do the other's job.

#### Why This Is Perfect for LLMs

**1. Offloads edge-case thinking from LLM to CPU.**

The LLM is bad at imagining adversarial inputs. The CPU is perfect at generating them. Property tests let each do what it's best at: the LLM declares correctness properties (pattern matching on specs), the CPU generates test cases (brute-force enumeration).

**2. The LLM writes properties, not test cases.**

`assert list.is_sorted(sorted)` is a single statement that replaces 50 hand-picked `verify` assertions. The LLM expresses *what* correct means, not *which specific inputs to check*.

**3. Minimal failing inputs are LLM-readable.**

The fuzzer shrinks failing cases to the simplest reproduction. `sort_list([1, 0]) returned [1, 0]` is trivially debuggable. The LLM doesn't waste tokens analyzing a 847-element list.

**4. Native speed makes fuzzing practical.**

10,000 iterations in milliseconds because Jett compiles to native code. The fuzzer runs as part of `jett test`, not as a separate expensive process. The LLM's compile-test-fix loop stays fast.

**5. Catches hallucinated logic that verify blocks miss.**

A `verify` block with 5 hand-picked examples might pass even if the function is completely wrong for edge cases. A `property` block with 10,000 random inputs will almost certainly catch it. The combination of both — `verify` for compile-time proof of known cases, `property` for fuzz-time proof of unknown cases — provides the strongest correctness guarantee an LLM-generated function can have.

#### Implicit Views in Test and Debug Contexts

In `property` blocks, `verify` blocks, and `agent_breakpoint()` evaluations, all values are **implicitly viewable** — they can be used multiple times without being consumed. This is a pragmatic relaxation of linear typing for testing and debugging contexts:

```
property sort_preserves_elements:
    given items: list[int]
    let sorted = sort_list(items)
    # In property blocks, sorted can be used multiple times:
    assert list.length(sorted) is list.length(items)
    assert list.is_sorted(sorted)
    assert list.all_elements_in(sorted, items)
    # Without implicit views, each use of `sorted` would consume it.
```

**Why this is safe:**

- Property and verify blocks never run in production — they execute at compile time or during `jett test`.
- The relaxation is confined to a lexical scope (the block itself). Outside the block, normal linear rules apply.
- The compiler still tracks types, capabilities, and refinements. Only linear consumption is relaxed.
- `agent_breakpoint()` evaluations are debug-only (compiled out in `--release`). Expression evaluation implicitly views all variables in scope, ensuring debugging is non-destructive.

### Rule Set 26: Type-Level Data Lineage (Tracked)

#### The Problem: Debugging Requires Tracing, and Tracing Floods Context

When an LLM-generated function produces a wrong result, debugging traditionally requires one of two approaches:

1. **Print-statement debugging.** Scatter `print(variable)` calls throughout the code. This produces massive output — hundreds of lines of log output for every variable at every step. The LLM must parse all of it to find the one place where the value went wrong. The output floods the context window with irrelevant data.

2. **Step-through debugging.** Use a debugger to step through execution line by line. This is interactive and requires a human. An LLM cannot use a step-through debugger — it is a batch-oriented agent, not an interactive one.

Both approaches share the same flaw: they generate **far more information than needed**. If `tax_rate` is wrong, the LLM doesn't need to see the history of every variable in the program — it only needs the history of `tax_rate`.

#### The Solution: `tracked[T]` — Per-Variable Lineage at the Type Level

Jett introduces a `tracked` type wrapper. When the LLM suspects a specific variable is wrong, it changes the type from `T` to `tracked[T]`. The compiler then secretly attaches a lightweight history log to that variable's memory arena. Every time the value passes through a function or is transformed in a pipeline, the compiler records the file name, line number, function name, and the before/after state.

**Standard code:**

```
let tax_rate: int = calculate_tax(user)
```

**Debugging code — change one type:**

```
let tax_rate: tracked[int] = calculate_tax(user)
```

That's it. One word added to the type annotation. The rest of the code does not change — `tracked[int]` is assignment-compatible with `int` in all contexts. The function signatures don't change. The pipeline doesn't change. The compiler silently instruments the tracking.

#### How Tracking Works Under the Hood

Because Jett uses linear typing (Rule Set 10.1), the compiler knows every function a value passes through and the exact order. There are no hidden references, no aliasing, no shared mutable state. The value moves sequentially from function to function. The compiler uses this to build a perfect lineage chain.

When a variable is typed as `tracked[T]`, the compiler:

1. Allocates a small array in the current arena (Rule Set 10.1).
2. Records the initial value and source location.
3. At every function call or pipeline step that takes and returns the value, records: function name, file, line, value before, value after.
4. When the LLM calls `trace()`, the accumulated lineage is emitted as structured JSON.

**The trace output:**

```
function process_invoice(stdout: Stdout, income: float) returns nothing:
    let mutable tax: tracked[float] = calculate_base_tax(income)
    tax = apply_state_tax(tax, "CA")
    tax = apply_discount(tax, "veteran")
    let final_amount = finalize(tax)

    trace(final_amount, stdout)
```

**Output — a tiny, hyper-specific JSON log for one variable:**

```json
{
    "variable": "tax",
    "final_value": 847.30,
    "lineage": [
        {
            "step": 1,
            "function": "calculate_base_tax",
            "file": "src/tax.jett",
            "line": 12,
            "input": {"income": 50000.0},
            "output": 5000.0
        },
        {
            "step": 2,
            "function": "apply_state_tax",
            "file": "src/tax.jett",
            "line": 13,
            "input": 5000.0,
            "output": 5325.0,
            "note": "applied CA rate: 6.5%"
        },
        {
            "step": 3,
            "function": "apply_discount",
            "file": "src/tax.jett",
            "line": 14,
            "input": 5325.0,
            "output": 4792.50,
            "note": "applied veteran discount: 10%"
        },
        {
            "step": 4,
            "function": "finalize",
            "file": "src/tax.jett",
            "line": 15,
            "input": 4792.50,
            "output": 847.30,
            "note": "SUSPICIOUS: large change from 4792.50 to 847.30"
        }
    ]
}
```

The LLM receives just this — a few lines of JSON showing exactly how the value evolved. It instantly sees that `finalize` is where the math went wrong (input 4792.50, output 847.30 — an unreasonable transformation). No guessing. No massive logs. No scrolling through hundreds of print statements.

#### `trace()` and the Pipeline Operator

Tracking integrates naturally with pipelines (Rule Set 19):

```
let tax_amount: tracked[float] = income
    |> calculate_base_tax
    |> apply_state_tax("CA")
    |> apply_discount("veteran")
    |> finalize

trace(tax_amount, stdout)
```

Each `|>` step is a lineage entry. The trace output shows the value flowing left-to-right through the pipeline, with before/after at every step. The pipeline structure maps 1:1 to the lineage array.

#### Zero Performance Impact on Untracked Variables

`tracked[T]` only instruments the specific variable it is applied to. Every other variable in the program runs at full native speed with zero overhead. This is critical for debugging in production-like conditions — the LLM can track one suspicious variable without slowing down the rest of the application.

| Variable type | Runtime cost |
|--------------|-------------|
| `int` | Zero overhead — native speed |
| `tracked[int]` | Small overhead — arena allocation + lineage recording per step |
| Every other variable in the program | Zero overhead — unaffected by the tracked variable |

#### Tracked Types with Error Handling

When a tracked value passes through a `handle` block, the lineage records the error path:

```
let data: tracked[string] = read_config(fs, "app.conf") handle error:
    return fail("config not found")
```

If the `handle` path is taken, the lineage records:

```json
{
    "step": 1,
    "function": "read_config",
    "file": "src/config.jett",
    "line": 5,
    "input": "app.conf",
    "output": "ERROR: file not found",
    "error_handled": true,
    "handler_action": "return fail(\"config not found\")"
}
```

The LLM can see not just the value changes but also where error paths were taken and what the error was.

#### ASP Integration — Trace Output as Structured JSON

`trace()` outputs to the ASP (Rule Set 21) when `--agent` is active:

```
jett run app.jett --agent --trace-var tax
```

The trace data is part of the agent JSON payload. The LLM receives it directly in the compile-test-fix loop. No terminal parsing, no log file searching.

#### Tracked Types Are Opt-In and Temporary

`tracked[T]` is a **debugging tool**, not a permanent annotation. The workflow:

1. LLM generates code. A test or property fails.
2. LLM changes the suspicious variable from `T` to `tracked[T]`.
3. Runs the program. Reads the trace output.
4. Identifies the broken function from the lineage.
5. Fixes the function.
6. Removes `tracked` — changes back to `T`.

The `tracked` annotation is meant to be temporary. `jett format` can optionally warn about tracked types left in production code.

#### Combining Tracked with Property Testing

When a `property` block finds a failing input, the LLM can re-run with tracking to see exactly where the logic broke:

```
# Property test found: sort_list([3, 1, 2]) returned [3, 1, 2] (not sorted)
# LLM adds tracking to debug:

function sort_list_debug(items: view list[int], stdout: Stdout) returns tracked[list[int]]:
    let mutable result: tracked[list[int]] = Linear.clone(items)
    result = partition(result)
    result = merge(result)
    trace(result, stdout)
    return result
```

The trace shows which step (partition or merge) produced the wrong intermediate result.

#### Why This Is Perfect for LLMs

**1. Minimal context window usage.**

The trace output is a tiny JSON array — typically 5-10 entries, one per transformation step. Compare this to full application logs (thousands of lines) or print-statement debugging (output for every variable at every step). The LLM's context window stays focused on the one variable that matters.

**2. The LLM sees the exact step where things went wrong.**

The lineage array shows input and output at every function. If step 3 takes 5325.0 as input and produces 847.30 as output, the bug is in step 3. The LLM doesn't need to reason about the whole program — it reads the lineage and pinpoints the broken function.

**3. One-word change to enable.**

`int` → `tracked[int]`. No restructuring the code, no adding logging frameworks, no inserting print statements at 20 locations. One type annotation change activates full lineage tracking for that variable.

**4. Works with the existing type system.**

`tracked[int]` is compatible with `int` everywhere. Functions that take `int` accept `tracked[int]`. Pipelines work. Error handling works. The compiler handles the instrumentation transparently.

**5. Structured output feeds directly into the LLM.**

The trace JSON goes through the ASP. The LLM receives it as structured data it can parse natively. No regex on log files, no pattern matching on terminal output. Just JSON with labeled fields: `function`, `input`, `output`, `line`.

### Rule Set 27: The Interactive Agent Breakpoint

#### The Problem: LLMs Can't Step-Debug, and Print-Debugging Requires Predicting What to Inspect

Human developers use interactive debuggers — set a breakpoint, step through code, inspect variables on demand. LLMs cannot do this because debuggers are interactive, cursor-driven tools designed for human terminal sessions.

The fallback — print-statement debugging — requires the LLM to **predict in advance** which variables it will need to inspect. If the LLM adds `print(user)` but the bug is actually in `session.permissions`, it has to edit the code, add a new print, recompile, and re-run. Each round-trip costs tokens, time, and context.

The fundamental mismatch: debugging is inherently **interactive and exploratory**, but LLMs operate in **batch mode** (generate code → compile → read output). The LLM needs a way to explore the runtime state of a program dynamically, without predicting what to inspect before the program runs.

#### The Solution: `agent_breakpoint()` — A Chatbot Inside the Running Program

Jett provides a built-in `agent_breakpoint()` function. When the native application hits this line during execution, it:

1. **Pauses execution** at that exact point.
2. **Opens an ASP communication channel** (lightweight HTTP server on localhost or stdin/stdout loop).
3. **Sends a structured prompt** to the LLM describing the current execution state.
4. **Waits for queries** from the LLM.
5. **Responds to each query** with structured JSON.
6. **Resumes execution** when the LLM sends a `continue` command.

The running application becomes a **chatbot** that the LLM can interrogate.

**Inserting a breakpoint:**

```
function process_order(fs: Filesystem, order: Order) returns result[Receipt, string]:
    let validated = validate_order(order) handle error:
        return fail("validation failed")

    agent_breakpoint()   # execution pauses here

    let charged = charge_payment(validated) handle error:
        return fail("payment failed")
    return ok(create_receipt(charged))
```

**What the LLM receives when the breakpoint is hit:**

```json
{
    "type": "agent_breakpoint",
    "file": "src/orders.jett",
    "line": 6,
    "function": "process_order",
    "scope": {
        "variables": [
            {"name": "order", "type": "Order", "status": "consumed"},
            {"name": "validated", "type": "ValidatedOrder", "status": "owned"},
            {"name": "fs", "type": "Filesystem", "status": "owned"}
        ]
    },
    "awaiting": "query"
}
```

The LLM now knows: execution is paused at line 6 of `process_order`, `validated` is available to inspect, `order` has been consumed (moved into `validate_order`), and `fs` is available.

#### The Query Protocol

The LLM sends JSON queries. The running application responds with JSON answers. Every exchange is structured — no terminal formatting, no spatial art.

**Inspect a variable:**

```json
{"query": "inspect", "variable": "validated"}
```

```json
{
    "variable": "validated",
    "type": "ValidatedOrder",
    "value": {
        "id": "ord-123",
        "items": [
            {"name": "widget", "qty": 2, "price": 9.99},
            {"name": "gadget", "qty": 1, "price": 24.99}
        ],
        "total": 44.97,
        "customer_id": "cust-456"
    }
}
```

**Inspect a specific field:**

```json
{"query": "inspect", "expression": "validated.total"}
```

```json
{
    "expression": "validated.total",
    "type": "float",
    "value": 44.97
}
```

**Evaluate an expression:**

```json
{"query": "evaluate", "expression": "list.length(validated.items)"}
```

```json
{
    "expression": "list.length(validated.items)",
    "type": "int",
    "value": 2
}
```

**Inspect the call stack:**

```json
{"query": "call_stack"}
```

```json
{
    "call_stack": [
        {"function": "main", "file": "src/main.jett", "line": 15},
        {"function": "handle_request", "file": "src/server.jett", "line": 42},
        {"function": "process_order", "file": "src/orders.jett", "line": 6, "current": true}
    ]
}
```

**Inspect variables in a parent scope:**

```json
{"query": "inspect_scope", "frame": "handle_request"}
```

```json
{
    "function": "handle_request",
    "variables": [
        {"name": "request", "type": "Request", "value": {"method": "POST", "path": "/orders"}},
        {"name": "stdout", "type": "Stdout", "status": "capability"}
    ]
}
```

**Continue execution:**

```json
{"query": "continue"}
```

```json
{
    "type": "breakpoint_resumed",
    "file": "src/orders.jett",
    "line": 6
}
```

#### Conditional Breakpoints

The LLM can make breakpoints conditional — they only pause when a condition is true:

```
function process_batch(fs: Filesystem, orders: view list[Order]) returns nothing:
    for order in view orders:
        if order.total > 1000.0:
            agent_breakpoint()   # only pause for high-value orders
        let result = process_single_order(fs, order)
```

Or using a more targeted form:

```
agent_breakpoint(when: validated.total < 0.0)
# Only pause when the total is negative — catches the specific bug
```

The conditional form saves time: the LLM doesn't have to step through 500 normal orders to reach the one that's broken. It pauses only when the suspicious condition is met.

#### The Debugging Workflow

The LLM's debugging loop with `agent_breakpoint()`:

```
1. LLM generates code. A test or property fails.
2. LLM reads the failure (via ASP) and suspects a function.
3. LLM inserts agent_breakpoint() before the suspicious line.
4. Recompiles and runs: jett run app.jett --agent
5. Program pauses. LLM receives the breakpoint notification.
6. LLM sends inspect queries:
   - "What is validated.total?" → 44.97
   - "What is validated.items?" → [{widget, 2}, {gadget, 1}]
   - "What does list.length(validated.items) give?" → 2
   - Aha — the total should be 44.97 but the expected was 45.97.
     The bug is in validate_order's total calculation.
7. LLM sends "continue" to resume.
8. LLM removes agent_breakpoint(), fixes validate_order.
9. Re-runs tests. Passes.
```

The LLM explored the runtime state interactively — inspecting only what it needed, one query at a time. No pre-planned print statements. No flooded context window. Just targeted questions and precise answers.

#### Multiple Breakpoints and Stepping

The LLM can insert multiple breakpoints:

```
function calculate_tax(income: float, state: string) returns float:
    let base = income * 0.15
    agent_breakpoint()   # check base calculation

    let state_rate = get_state_rate(state)
    let state_tax = base * state_rate
    agent_breakpoint()   # check state tax calculation

    return base + state_tax
```

Each breakpoint pauses independently. The LLM can inspect the state at two points in the same function without needing to understand the entire execution flow between them.

**Step-to-next-line (single-step):**

```json
{"query": "step"}
```

After hitting a breakpoint, the LLM can step forward one statement at a time, inspecting the state after each step. This simulates step-through debugging without a traditional debugger UI.

#### Security: Agent Breakpoints Are Debug-Only

`agent_breakpoint()` only compiles in debug mode:

```
# Debug mode — breakpoints are active:
jett run app.jett --agent --debug

# Release mode — breakpoints are compiled out:
jett build app.jett --release
# agent_breakpoint() calls are silently removed. Zero overhead.
```

In release builds, `agent_breakpoint()` does not exist — it compiles to nothing. There is no performance cost and no security risk of leaving a breakpoint in production. The compiler can optionally warn about `agent_breakpoint()` calls in release builds.

`agent_breakpoint()` is a compiler primitive exempt from the capability system. It only exists in debug mode and is compiled out in release builds. No capability parameter is required.

#### ASP Communication Modes

The agent breakpoint communicates through two modes:

**Stdin/stdout mode (default for CLI):**

```
jett run app.jett --agent --debug
# Breakpoint notifications and queries go through stdin/stdout as JSON lines
```

**HTTP mode (for remote debugging):**

```
jett run app.jett --agent --debug --breakpoint-port 9229
# Breakpoint opens HTTP server on localhost:9229
# LLM agent sends JSON queries via HTTP POST
```

The HTTP mode allows the LLM to debug a running server or long-lived process without stopping it. The LLM connects to the breakpoint port, sends queries, and the application resumes when told to continue.

#### Why This Is Perfect for LLMs

**1. Debugging becomes conversational.**

The running program is a chatbot. The LLM asks questions ("what is `validated.total`?"), gets answers (44.97), and asks follow-up questions. This matches the LLM's natural interaction model perfectly — it's what LLMs are built to do.

**2. No prediction required.**

With print-debugging, the LLM must predict in advance which variables to inspect. With `agent_breakpoint()`, it decides at runtime based on what it sees. This eliminates wasted round-trips where the LLM printed the wrong variable.

**3. Minimal context window usage.**

Each query returns one small JSON response. The LLM only sees the data it asked for — not a flood of every variable in the program. The context window stays focused on the investigation.

**4. Works with the compile-fix loop.**

Insert breakpoint → run → inspect → identify bug → remove breakpoint → fix → re-run. This fits naturally into the existing ASP compile-fix loop (Rule Set 21). The breakpoint is just another tool in the same JSON-based workflow.

**5. Conditional breakpoints save tokens.**

`agent_breakpoint(when: total < 0.0)` skips thousands of normal executions and pauses only on the suspicious case. The LLM doesn't waste context on irrelevant iterations.

---

### Rule Set 28: Profiling — Bottleneck Summaries over Visual Flamegraphs

#### The Problem with Traditional Profiling

When an LLM writes an application and it runs slowly, the developer (or the LLM itself) needs to understand *why*. Traditional profilers produce visual flamegraphs — interactive SVG or HTML graphics that a human navigates by hovering, zooming, and scanning colored bands. This output is completely useless to an LLM:

1. **Flamegraphs are images.** LLMs cannot see pixels. An SVG flamegraph is an opaque blob.
2. **pprof/perf output is noisy.** Raw profiler dumps contain thousands of lines of stack frames with cryptic symbol names. Even if fed as text, the LLM wastes its entire context window parsing noise.
3. **Humans use spatial intuition.** Flamegraphs work because a wide band *looks* big. LLMs have no spatial reasoning over rendered graphics. They need the same information as structured data.

Jett solves this by making the compiler itself the profiler — and outputting **Bottleneck Summaries** in structured JSON instead of visual artifacts.

#### The Design: `jett run --agent-profile`

Jett includes a built-in CPU sampling profiler at the compiler level. It is not a separate tool, not a third-party library, and not a visual application. It is a compiler flag:

```
jett run --agent-profile app.jett
```

This runs the program normally while collecting CPU samples at a configurable frequency. When the program exits (or is interrupted), instead of generating a flamegraph, the compiler analyzes the samples and produces a **Bottleneck Summary** — a structured JSON document identifying the critical performance bottlenecks.

#### Bottleneck Summary Format

The output is a JSON array of bottleneck entries, sorted by impact (highest CPU percentage first):

```json
{
    "profile_summary": {
        "total_samples": 48000,
        "sample_rate_hz": 1000,
        "wall_time_seconds": 48.0,
        "cpu_time_seconds": 47.2
    },
    "bottlenecks": [
        {
            "rank": 1,
            "function": "process_image",
            "namespace": "pipeline.transform",
            "file": "transform.jett",
            "line": 142,
            "cpu_percent": 34.2,
            "self_percent": 28.1,
            "total_samples": 16416,
            "self_samples": 13488,
            "hot_lines": [
                {"line": 155, "percent": 12.4, "code": "let pixel = image.get_pixel(x, y)"},
                {"line": 162, "percent": 9.7, "code": "let blurred = convolve(kernel, neighbors)"},
                {"line": 170, "percent": 6.0, "code": "output.set_pixel(x, y, blurred)"}
            ],
            "call_chain": [
                "main → run_pipeline → process_batch → process_image"
            ],
            "suggestion": "process_image accounts for 34.2% of CPU. The hot path is pixel-by-pixel iteration with per-pixel allocation. Consider using the standard library batch image operations (images.convolve_batch) which operate on the entire buffer."
        },
        {
            "rank": 2,
            "function": "parse_config",
            "namespace": "config.loader",
            "file": "loader.jett",
            "line": 28,
            "cpu_percent": 18.7,
            "self_percent": 3.2,
            "total_samples": 8976,
            "self_samples": 1536,
            "hot_lines": [
                {"line": 45, "percent": 8.1, "code": "let parsed = json.parse(raw_text, Document) handle error: return fail(error)"},
                {"line": 52, "percent": 7.4, "code": "let validated = schema.validate(parsed)"}
            ],
            "call_chain": [
                "main → initialize → parse_config"
            ],
            "suggestion": "parse_config is called once at startup but accounts for 18.7% of CPU. The json.parse and schema.validate calls dominate. If the config file is static, consider parsing at comptime."
        }
    ]
}
```

#### Key Design Decisions

**1. The compiler generates suggestions, not just data.**

Each bottleneck entry includes a `suggestion` field — a plain-English sentence the LLM (or human) can act on immediately. The compiler generates these using internal heuristics:

- If a function is called inside a loop and allocates on every iteration → suggest hoisting or batching.
- If a function's self-time is high relative to total time → the bottleneck is in its own body, not callees.
- If a function's self-time is low relative to total time → the bottleneck is in what it calls; suggest inlining or replacing callees.
- If a function appears in a single call chain → suggest restructuring the caller.
- If a hot line involves a known-expensive standard library function → suggest the efficient alternative.
- If work is done at runtime that could be done at comptime → suggest `comptime`.

**2. Hot lines pinpoint the exact code.**

Instead of just naming the function, the summary includes the specific lines within that function that consumed the most CPU. This gives the LLM (or human) surgical precision — fix *these three lines*, not "somewhere in this 40-line function."

**3. Call chains provide context.**

Each bottleneck includes the call chain that leads to it. This is critical for LLMs because it answers "how did we get here?" — the LLM knows whether the bottleneck is in a startup path, a request handler, a background task, etc.

**4. Sorted by impact.**

Bottlenecks are ranked by CPU percentage, highest first. An LLM can read just the first entry and make the highest-impact fix. It doesn't need to process the entire profile — the most important information is always first.

**5. Threshold filtering.**

Only bottlenecks above a configurable threshold (default: 5% of CPU time) are included. This eliminates noise. A traditional profiler shows every function; the bottleneck summary shows only what matters.

```
# Only show bottlenecks above 10% CPU
jett run --agent-profile --profile-threshold 10 app.jett

# Show more detail (lower threshold)
jett run --agent-profile --profile-threshold 2 app.jett
```

#### Integration with the Agent Server Protocol

When combined with the `--agent` flag (Rule Set 21), the profiler output is emitted as part of the standard ASP JSON stream:

```
jett run --agent --agent-profile app.jett
```

This means the profiler fits into the existing LLM-driven development loop:

1. LLM writes the application.
2. LLM runs it with `--agent --agent-profile`.
3. Program executes and profile is collected.
4. ASP returns the bottleneck summary as structured JSON.
5. LLM reads the top bottleneck, applies the suggestion.
6. LLM re-runs with profiling to verify the improvement.
7. Repeat until performance is acceptable.

No human intervention required. No visual tools. No copy-pasting flamegraph screenshots into chat windows.

#### Memory Profiling

The same approach extends to memory profiling with `--agent-profile-memory`:

```
jett run --agent-profile-memory app.jett
```

Output follows the same structure but reports allocation-heavy functions instead of CPU-heavy ones:

```json
{
    "memory_summary": {
        "peak_memory_bytes": 134217728,
        "total_allocations": 2400000,
        "total_bytes_allocated": 891289600
    },
    "bottlenecks": [
        {
            "rank": 1,
            "function": "build_index",
            "namespace": "search.indexer",
            "file": "indexer.jett",
            "line": 88,
            "allocation_percent": 42.1,
            "total_allocations": 1010400,
            "total_bytes": 375272960,
            "hot_lines": [
                {"line": 102, "percent": 31.0, "code": "let entry = IndexEntry(term, doc_id, position)"}
            ],
            "suggestion": "build_index is responsible for 42.1% of all allocations. Each IndexEntry is allocated individually inside a loop. Consider using an arena (Rule Set 10.2) to batch-allocate all entries."
        }
    ]
}
```

#### Comparison Profiling

To measure the impact of an optimization, `--agent-profile-compare` accepts a baseline profile:

```
jett run --agent-profile --profile-output baseline.profile app.jett
# ... make changes ...
jett run --agent-profile --profile-compare baseline.profile app.jett
```

The output includes a `delta` field on each bottleneck:

```json
{
    "rank": 1,
    "function": "process_image",
    "cpu_percent": 12.1,
    "delta": {
        "previous_cpu_percent": 34.2,
        "change_percent": -22.1,
        "status": "improved"
    }
}
```

This closes the optimization loop: the LLM can verify that its fix actually worked, with exact numbers, in a single structured response.

#### Why This Is Perfect for LLMs

**1. Zero visual dependency.**

The entire profiling workflow is text/JSON. No flamegraphs, no browser-based viewers, no SVG files. An LLM can consume the output directly in its context window.

**2. Actionable by default.**

Traditional profilers present raw data and expect the developer to interpret it. Bottleneck summaries include the `suggestion` field — the compiler has already done the first-pass interpretation. The LLM can act immediately.

**3. Token-efficient.**

A flamegraph for a complex application might have thousands of stack frames. The bottleneck summary distills this to 3-10 entries, each a few lines of JSON. This fits easily within any context window.

**4. Fits the ASP loop.**

Because the profiler output is standard ASP JSON, it slots directly into the existing compile → run → diagnose → fix cycle (Rule Set 21). The LLM doesn't need a separate tool or workflow for performance optimization — it uses the same `--agent` flag it already uses for compilation errors.

**5. Comparison profiling closes the loop.**

The `--profile-compare` flag gives the LLM a before/after diff. This is essential for auto-regressive optimization: the LLM makes a change, measures the impact, and decides whether to keep it or try something else. Without structured comparison, the LLM would have to remember the previous profile and manually compute deltas.

---

## Core Design Principles

### 1. Token Economy

Every syntactic choice should minimize the number of tokens required to express a concept. LLM APIs charge per token and context windows are finite. Wasting tokens on boilerplate, verbose keywords, or redundant delimiters is a real cost.

**Guidelines:**

- Prefer common English words as keywords (they typically map to single tokens).
- Avoid multi-character symbolic operators that tokenizers split unpredictably.
- Eliminate mandatory boilerplate (the entry point is just `function main(...)`, no required imports for standard functionality).
- Use indentation-based structure to remove the need for braces or `end` keywords.

### 2. Symbol Minimalism

Jett keeps its symbol set **as small as possible**. Symbols are only used where a keyword alternative would be genuinely worse.

**Allowed symbol set:**

| Symbol | Usage |
|--------|-------|
| `=` | Assignment |
| `+` `-` `*` `/` | Arithmetic (convenience for simple expressions). `modulo` is a keyword operator: `a modulo b` |
| `>` `<` `>=` `<=` | Comparison operators |
| `!=` | Inequality |
| `.` | Member access |
| `,` | Separator |
| `(` `)` | Grouping, function calls |
| `[` `]` | Indexing, collections |
| `:` | Type annotations, block starts |
| `"` | Strings |
| `\|>` | Pipeline operator (left-to-right data flow) |

The compiler infers `view` at call sites automatically. When a pipeline step pipes into a function that declares a `view` parameter, the compiler handles the view semantics — no explicit `view` annotation is needed at the call site. For example: `data |> json.serialize`.

**Replaced by keywords:**

| Instead of | Jett uses |
|------------|-----------|
| `==`, `===` | `is` |
| `&&` | `and` |
| `\|\|` | `or` |
| `!` | `not` |
| `->`, `=>` | `returns` |
| `{ }` | indentation |
| `;` | newline |
| `//` | `#` |

### 3. Predictable Patterns

LLMs excel at pattern completion. Jett's syntax is **highly regular** so that once an LLM has seen one example of a construct, it can reliably produce all variations.

**Guidelines:**

- One way to do things, not many. No synonym keywords, no alternate syntax.
- Every block-level construct follows the same shape: `keyword ... :` followed by an indented body.
- Function definitions, conditionals, loops, and type definitions all share this uniform structure.
- No implicit returns, no optional parentheses that change meaning, no context-sensitive parsing surprises.

### 4. Explicit Over Clever

Clever shortcuts and implicit behavior are where LLMs make mistakes. Jett favors code that reads linearly and states its intent clearly.

**Guidelines:**

- No operator overloading.
- No implicit type coercion.
- No variable shadowing.
- No hoisting or order-independent definitions.
- Error handling is explicit (no hidden exceptions propagating silently).

---

## Syntax Overview

### Program Entry Point

Every Jett program starts with a `main` function. There are no top-level statements — not even in the main file. Every file consists only of struct definitions, function definitions, and namespace declarations.

```
namespace app

function main(stdout: Stdout, fs: Filesystem) returns nothing:
    use config

    let app_config = config.load(fs) handle error:
        Stdout.write(stdout, "config failed: {error}")
        return

    Stdout.write(stdout, "running with config: {app_config.name}")
```

The runtime provides capabilities to `main` based on its parameter list. If `main` does not declare a `Network` parameter, the program physically cannot access the network — the capability is never created. This is where the capability system begins: `main` is the root of the capability tree.

> **Note:** `main` follows the same limits as every other function (50 statements, 4 nesting levels, 6 parameters, 10 cyclomatic complexity). If `main` is hitting those limits, it is doing too much — extract the logic into named functions. A well-structured `main` is a short orchestrator that wires together capabilities and delegates to other functions.

### Variables

```
let name = "jett"
let age = 1
let mutable counter = 0
```

Variables are immutable by default. The `mutable` keyword opts into mutability. (Full word, not `mut` — see tokenizer-friendly keywords rule.)

### Functions

```
function add(a: int, b: int) returns int:
    return a + b

function greet(stdout: Stdout, name: string) returns nothing:
    Stdout.write(stdout, "hello {name}")
```

`function` is always spelled out. `returns` declares the return type. No `->` arrow.

Every function always has a `returns` clause — functions that produce no value use `returns nothing`. This is consistent with the one-canonical-form principle: there is always exactly one pattern for function signatures, never "sometimes there's a `returns` clause, sometimes there isn't."

Named arguments work in both struct construction AND function calls. Any parameter can be passed by name for clarity. This allows `agent_breakpoint(when: condition)` and `GuiCapability.create_text_field(gui, label, width: 200, height: 30)` — mixing positional and named arguments in a single call.

### Conditionals

```
function classify(stdout: Stdout, x: int) returns nothing:
    if x > 0:
        Stdout.write(stdout, "positive")
    else if x is 0:
        Stdout.write(stdout, "zero")
    else:
        Stdout.write(stdout, "negative")
```

Note: `else if condition:` is the construct for chaining conditionals. It is not a separate keyword -- it is `else` followed by `if`, which naturally composes under the unified block syntax.

### Loops

```
function process_items(stdout: Stdout, items: list[string]) returns nothing:
    for item in items:
        Stdout.write(stdout, item)

function run_loop(mutable running: bool) returns nothing:
    while running:
        running = false
```

### Collections

```
let names = list("alice", "bob", "charlie")
let scores = map("alice": 10, "bob": 20)
```

Collections are constructed with explicit keywords. No `[]` literal for lists, no `{}` for maps. The constructor keyword *is* the type — AST-native.

### Structs

```
struct Point:
    x: float
    y: float

    function distance(self: view Point, other: view Point) returns float:
        let dx = self.x - other.x
        let dy = self.y - other.y
        return math.sqrt(dx * dx + dy * dy)

# Methods are called with module syntax — there is no p1.distance(p2) form:
let p1 = Point(x: 0.0, y: 0.0)
let p2 = Point(x: 3.0, y: 4.0)
let d = Point.distance(p1, p2)
```

### Error Handling

```
function read_file(fs: Filesystem, path: string) returns result[string, string]:
    let content = Filesystem.read_file(fs, path) handle error:
        return fail("could not open file")
    return ok(content)

# handle is the ONLY way to unwrap a result:
let content = read_file(fs, "data.txt") handle error:
    Stdout.write(stdout, error)
    return
Stdout.write(stdout, content)
```

Errors are values, never exceptions. Functions that can fail return `result[T, E]`. The `handle` keyword is the **only** way to unwrap a result — `match` is reserved for user-defined enums. See Rule Set 5 for the full rationale.

Every `handle` block must end with either `return` (exit function) or `default` (provide fallback value):

- **Default form:** provides a fallback value using the `default` keyword.
  ```
  let content = read_file(fs, "data.txt") handle error:
      default "default content"
  ```
- **Return form:** exits the enclosing function via `return` or `return fail(...)`.
  ```
  let content = read_file(fs, "data.txt") handle error:
      return fail(error)
  ```

### Enums (User-Defined Union Types)

Enums are Jett's user-defined union types. Each variant can carry different associated data, and `match` forces exhaustive handling of all variants. There are no anonymous union types (`string | int`) — if you need a value that can be one of several types, define an enum.

```
enum Color:
    red
    green
    blue

enum Shape:
    circle(radius: float)
    rect(width: float, height: float)
```

Jett has three union-like constructs, each with its own unwrap mechanism:

| Type | Variants | Unwrap mechanism |
|------|----------|-----------------|
| `result[T, E]` | `ok(T)`, `fail(E)` | `handle error:` |
| `optional[T]` | `some(T)`, `none` | `handle:` |
| User-defined enums | Any number of variants | `match` |

### Match (User-Defined Enums Only)

`match` is used exclusively for user-defined enums. It cannot be used on `result` types — use `handle` for those (see Rule Set 5).

```
function describe_shape(stdout: Stdout, shape: Shape) returns nothing:
    match shape:
        circle(r):
            Stdout.write(stdout, "circle with radius {r}")
        rect(w, h):
            Stdout.write(stdout, "rect {w} by {h}")
```

### Assert

`assert` checks a condition and halts the program if it fails. Two forms are supported:

```
assert list.length(items) > 0
assert balance >= 0.0 "balance must not be negative"
```

The first form checks truthiness. The second form provides a custom failure message.

### Modules

```
namespace myapp

function main(stdout: Stdout, net: Network) returns nothing:
    use math
    use net.http
    let pi = math.pi
    let response = http.get(net, "https://example.com") handle error:
        # error is HttpError — the module's specific error type
        Stdout.write(stdout, "request failed: {error}")
        return
```

All `use` statements must be inside a function or block — file-level imports are banned.

### String Interpolation

String interpolation is the ONE canonical mechanism for building strings in Jett. There is no `string.concat()` function and no `+` operator for strings. **All strings are interpolated by default** — there is no separate "plain string" vs "template string" distinction, no `f""` prefix, no backtick delimiter. Every `"..."` string supports `{expr}` interpolation. This eliminates a decision point: the LLM never has to choose between string types.

```
let name = "world"
let greeting = "hello {name}"           # "hello world"
let result = "total: {order.total}"     # expressions inside {} are evaluated
let multi = "{a} + {b} = {a + b}"       # arbitrary expressions allowed
```

**Displayable requirement:** Expressions inside `{}` must be of a type that implements the `Displayable` interface. The compiler calls `Displayable.display()` under the hood to produce the string representation. Types that do not implement `Displayable` are rejected:

```
let count = 42
let message = "count is {count}"        # OK — int implements Displayable

let user = User(name: "alice")
let msg = "user: {user}"               # COMPILE ERROR: User does not implement Displayable
```

**Compiler-stdlib coupling:** This is one of a small number of places where the compiler has special knowledge of a standard library interface. String interpolation depends on `Displayable`, just as `handle error:` depends on the built-in `result` type and `handle:` depends on `optional`. These are intentional, well-defined couplings — not a general implicit conversion system. Outside of string interpolation, converting to string requires an explicit `string.from_int()` or `string.from_float()` call.

**Literal braces:** Use `{{` and `}}` for literal `{` and `}` characters:

```
let json_example = "the format is: {{key: value}}"
# produces: "the format is: {key: value}"
```

### Comparison Operators

Jett uses symbolic comparison operators alongside keyword operators for equality and logic:

| Operator | Meaning |
|----------|---------|
| `>` | Greater than |
| `<` | Less than |
| `>=` | Greater than or equal |
| `<=` | Less than or equal |
| `is` | Equality |
| `!=` | Inequality |
| `and` | Logical and |
| `or` | Logical or |
| `not` | Logical not |

Arithmetic: `+`, `-`, `*`, `/`, `modulo`.

```
if x > 0:
    Stdout.write(stdout, "positive")
if balance >= 0.0:
    Stdout.write(stdout, "solvent")
```

### Capability Auto-Rebinding

Capability parameters (such as `Filesystem`, `Stdout`, `Network`, etc.) are automatically rebound by the compiler. Functions declare capability parameters as regular parameters in their signature, and the compiler recognizes capability types and handles rebinding without needing a `with` clause.

```
# The compiler sees that stdout is a Stdout capability and automatically
# threads it through — no 'with Stdout' annotation needed.
function greet(stdout: Stdout, name: string) returns nothing:
    Stdout.write(stdout, "hello {name}")
```

The `view` keyword is used only in parameter declarations to indicate read-only, non-owning references. The compiler infers `view` at call sites automatically — callers do not write `view` when passing arguments.

---

## Type System

### Built-in Types

| Type | Description |
|------|-------------|
| `int` | 64-bit integer |
| `float` | 64-bit floating point |
| `string` | UTF-8 string (full word, not `str`) |
| `bool` | `true` or `false` |
| `list[T]` | Ordered collection |
| `map[K, V]` | Key-value collection |
| `set[T]` | Unique collection |
| `optional[T]` | Either a `T` or `none` |
| `result[T, E]` | Either `ok(T)` or `fail(E)` |
| `nothing` | Unit type with exactly one value, also called `nothing`. Used in `result[nothing, string]` for functions that can fail but return no value on success. `ok(nothing)` is the canonical form for wrapping success in `result[nothing, E]`. |
| `bytes` | Sequence of raw bytes (0-255). Distinct from `string` (which is UTF-8 text). Used for binary data, network packets, file I/O with binary formats. |

### Type Inference

Types are inferred where unambiguous. Annotations are required on function signatures but optional on local variables.

```
let x = 42              # inferred as int
let y: float = 42       # explicit annotation
```

### Generics

Generics use `[T]` (square brackets) rather than `<T>` — avoids ambiguity with comparison operators and is more reliably tokenized.

**Basic generic function:**

```
function first[T](items: view list[T]) returns optional[T]:
    return list.get(items, 0)
```

**Constrained generics — limiting which types T can be:**

Generic type parameters can be constrained to types that implement specific interfaces using the `implements` keyword:

```
function sort[T implements Orderable](items: list[T]) returns list[T]:
    # T is guaranteed to support comparison operations
    ...

function display_sorted[T implements Orderable and Displayable](items: list[T], stdout: Stdout) returns nothing:
    # T must implement both Orderable and Displayable
    let sorted = sort(items)
    for item in sorted:
        Stdout.write(stdout, Displayable.display(item))

# Multiple type parameters — comma separates parameters, and separates interfaces:
function merge[T implements Orderable and Hashable, U implements Displayable](a: T, b: U) returns string:
    ...
```

Multiple interface constraints on the same type parameter are joined with `and`. Commas separate distinct type parameters. This avoids ambiguity: `and` always means "also this interface," and commas always mean "next type parameter."

**Unconstrained generics:**

If a generic type parameter has no constraint, the only operations available are storing it and passing it around. You cannot compare, display, or otherwise operate on an unconstrained `T` — the compiler enforces this:

```
function wrap[T](value: T) returns list[T]:
    return [value]    # OK — storing T in a list

function bad_sort[T](items: list[T]) returns list[T]:
    if items[0] > items[1]:    # COMPILE ERROR: T does not implement Orderable
        ...
```

**Monomorphization — generics are resolved at compile time:**

The compiler generates a separate version of each generic function for every concrete type used at call sites. If the codebase calls `sort[int](numbers)` and `sort[string](names)`, the compiler produces two functions: one for `int` and one for `string`. There is no runtime type erasure and no runtime overhead — generic code runs at the same speed as hand-written type-specific code.

The interface constraint does not cause the compiler to pre-generate code for all implementing types. It only generates code for types **actually used**. The constraint is for type-checking the function body, not for driving code generation.

**Standard library interfaces for primitives:**

Primitive types (`int`, `float`, `string`, `bool`) implement standard interfaces from the standard library:

| Interface | Implemented by | Operations |
|-----------|---------------|------------|
| `Equatable` | `int`, `float`, `string`, `bool` | `is`, `!=` |
| `Orderable` | `int`, `float`, `string` | `<`, `>`, `<=`, `>=` |
| `Displayable` | `int`, `float`, `string`, `bool` | string representation (used by string interpolation) |
| `Hashable` | `int`, `string`, `bool` | can be used as `map` keys and `set` elements |

These are ordinary interface implementations, not compiler magic. They follow the same `implement` block pattern as any user-defined struct.

---

## Standard Library

The standard library is intentionally massive and opinionated. The goal is to make Jett a **batteries-included** language where the LLM writes orchestration code, not algorithms. See Rule Set 8 for the full rationale.

### Core Modules

- **string** — trim, split, join, replace, pad, slugify, truncate, contains, between, starts/ends_with, is_empty, is_not_empty
- **math** — arithmetic, clamp, round, abs, min/max, average, median, pow, floor, ceil, constants
- **list** — filter, map, reduce, find, sort, sort_by, sort_by_index, unique, chunk, zip, group_by, flatten, first, last, skip, take, length, get, append, is_sorted, all_elements_in
- **map** — get, set, keys, values, merge, filter, contains_key, get_or
- **set** — add, remove, union, intersection, difference, contains
- **net.http** — HTTP client (get, post, put, delete), response handling, HttpError enum (connection_failed, timeout, status_error)
- **net.socket** — low-level TCP/UDP networking
- **json** — parse, parse_raw, serialize, serialize_public, serialize_full, pretty print, nested path access, get, get_or
- **time** — now, format, parse, difference, add/subtract, comparisons, day_of_week, years_between
- **os** — environment variables, process management, file system, argv
- **test** — mock infrastructure for property-based testing (`test.mock` for mock filesystems, networks, etc.)
- **log** — structured logging with levels
- **format** — number formatting, padding, and text alignment
- **crypto** — hashing (sha256, sha512, md5), HMAC
- **encoding** — base64, hex, URL encoding/decoding
- **validate** — standard refinement types for common formats: Email, URL, UUID, IPv4, IPv6. The type IS the validation — once assigned, the value is guaranteed valid.
- **regex** — pattern matching and extraction (when string functions aren't enough)
- **csv** — parsing and writing CSV data
- **random** — random numbers, random selection, shuffling
- **uuid** — UUID generation

---

## LLM-Specific Optimizations

### Consistent Naming Convention

All standard library functions and types use `snake_case`. No exceptions, no mixed conventions.

### No Semicolons, No Braces

Two of the most common LLM generation errors — missing semicolons and mismatched braces — are eliminated by design. Newlines end statements. Indentation defines blocks.

### Unified Block Syntax

Every block construct follows exactly one pattern — `keyword ... :` then indented body:

```
function ...:
if ...:
else ...:
for ...:
while ...:
struct ...:
enum ...:
match ...:
machine ...:
actor ...:
concurrent ...:
verify ...:
property ...:
mutual ...:
implement ...:
receive ...:
bitfield ...:
```

All 17 block constructs share the same shape. An LLM only needs to learn one pattern.

### Full English Keywords

Jett's keyword set uses complete, common English words that each map to a single token:

`let`, `mutable`, `function`, `return`, `returns`, `if`, `else`, `for`, `in`, `while`, `struct`, `enum`, `match`, `use`, `true`, `false`, `none`, `and`, `or`, `not`, `is`, `is_near`, `within`, `self`, `handle`, `error`, `default`, `result`, `ok`, `fail`, `as`, `break`, `continue`, `interface`, `implement`, `assert`, `type`, `where`, `value`, `mutual`, `machine`, `states`, `transitions`, `to`, `at`, `transition`, `arena`, `clone`, `actor`, `receive`, `send`, `ask`, `respond`, `spawn`, `concurrent`, `join`, `cancel`, `comptime`, `layout`, `verify`, `secret`, `declassify`, `serialize`, `namespace`, `bitfield`, `bit`, `bits`, `remaining`, `view`, `property`, `given`, `tracked`, `trace`, `agent_breakpoint`, `some`, `optional`, `nothing`, `int`, `float`, `string`, `bool`, `bytes`, `list`, `map`, `set`, `modulo`

### JSON AST Round-Tripping

Every Jett program can be mechanically converted to a JSON AST and back. LLMs can generate either form. Tooling can accept either form as input. This enables workflows like:

1. LLM generates JSON AST (if structured output is easier for the task).
2. Tool converts JSON AST to Jett source.
3. Human reviews Jett source.
4. Tool converts back to JSON AST for further LLM processing.

### Meaningful Error Messages

The compiler produces LLM-readable error messages. Instead of cryptic codes, errors describe what went wrong and suggest a fix in plain English.

```
error at line 12: variable "count" is not mutable
  hint: add "mutable" to the declaration: let mutable count = 0
```

---

## Compilation and Runtime

### Implementation Language

The Jett compiler and toolchain will be **written initially in Rust**. Rust provides a strong type system, memory safety, excellent performance, and a mature ecosystem of libraries for building compilers (parsing, code generation, etc.). Once Jett is mature enough, the compiler will be **self-hosted** — rewritten in Jett itself. This is a standard milestone for programming languages and will serve as a real-world stress test of the language.

### Target

Jett compiles to native code via an **LLVM backend** (primary target) for performance-critical applications, and can also be interpreted for scripting and rapid prototyping. As a future secondary target, Jett will support **transpilation to C** — this provides portability to platforms LLVM does not cover well (e.g., niche embedded targets), enables building Jett programs without an LLVM installation, and produces inspectable intermediate output for debugging.

### Modes

- **`jett run file.jett`** — interpret and run immediately
- **`jett build file.jett`** — compile to native binary
- **`jett test`** — run all `verify` and `property` blocks in the project (Jett has no `test` keyword; `verify` blocks run at compile time for pure functions, and `property` blocks run fuzz-based tests at test time)
- **`jett format`** — format source code (single canonical style, no configuration)

### Project Structure

```
project/
    jett.proj
    source/
        main.jett
        utils.jett
```

The `.proj` file is minimal:

```
name = "myproject"
version = "0.1.0"

deps:
    http = "https://packages.jett-lang.org/std/http/1.0/http.jett"
        hash "sha256:a1b2c3d4..."
    json = "https://packages.jett-lang.org/std/json/2.1/json.jett"
        hash "sha256:e5f6a7b8..."
```

---

## Implementation Roadmap

### Phase 1 — Language Foundation

- [ ] Formal grammar specification
- [ ] Lexer and parser
- [ ] AST definition (with JSON serialization from day one)
- [ ] Basic type checker
- [ ] Tree-walk interpreter for initial testing
- [ ] JSON AST to Jett source converter

### Phase 2 — Core Language

- [ ] Variables, functions, control flow
- [ ] Structs and enums
- [ ] Interfaces and `implement` blocks
- [ ] State machines (`machine`, `states`, `transitions`, `at`)
- [ ] Pattern matching
- [ ] Error handling (`result[T, E]`, `handle`, `ok`/`fail`)
- [ ] Capability types (Filesystem, Network, Stdout, Stderr, Stdin, Clock, Random, Process, Environment)
- [ ] Capability threading and auto-rebinding (compiler infers capability parameters)
- [ ] Capability narrowing (read_only, scoped, allow)
- [ ] Namespace-based module system (`namespace`, inline `use`, path-free resolution)
- [ ] Namespace scanner (recursive `.jett` file discovery, duplicate detection)
- [ ] Sub-namespace resolution (dot notation, parent imports)
- [ ] Generics
- [ ] Refinement types (`type X = T where ...`)
- [ ] Compile-time constraint validation
- [ ] Linear type system (move semantics, `Linear.clone()`)
- [ ] `view` keyword (read-only, non-owning references)
- [ ] View enforcement: no mutation, no thread escape, no scope escape
- [ ] Active view tracking (prevent mutation of owned data while views exist)
- [ ] `verify` blocks (co-located, comptime-executed contract tests)
- [ ] `property` blocks (property-based testing declarations)
- [ ] Type-aware random input generation (int, float, string, list, struct, enum)
- [ ] Input shrinking (minimal failing case discovery)
- [ ] ASP integration for property failure reporting
- [ ] Pipeline operator (`|>`) with compiler-enforced chaining rules
- [ ] `secret[T]` type wrapper with taint propagation
- [ ] `declassify` keyword and `secret.redact()`/`secret.compare()` functions

### Phase 3 — Memory and Performance

- [ ] Arena allocator (`arena()`, scope-bound bulk deallocation)
- [ ] `layout soa` annotation and compiler SoA transformation
- [ ] `comptime` keyword and compile-time function execution
- [ ] Comptime generic specialization
- [ ] Compiler makes all structs compatible with `json.serialize()` / `json.parse(raw, Type)`
- [ ] Auto-generated `to_bytes` / `from_bytes` for all structs
- [ ] `serialize` field annotation for custom naming
- [ ] `layout binary` with `size` for network protocol structs
- [ ] `bitfield` type with bit-level field declarations
- [ ] Bitfield `from_bytes` / `to_bytes` with automatic bit extraction/packing
- [ ] `layout network_order` annotation with auto byte-swap
- [ ] Bitfield enum integration (`as EnumType` on fields)
- [ ] Bitfield range validation (bit width → value range enforcement)
- [ ] Secret-aware serialization (`json.serialize_public`, compile error on `json.serialize` with secrets)
- [ ] Refinement type validation during deserialization

### Phase 4 — Concurrency

- [ ] Actor model (`actor`, `receive`, `send`, `ask`, `respond`)
- [ ] Linear typing enforcement across actor message passing
- [ ] Structured concurrency (`concurrent`, `spawn`, `join`, `cancel`)
- [ ] Compiler enforcement: no orphaned tasks, no unjoined spawns

### Phase 5 — Standard Library

- [ ] Core types (string, list, map, set)
- [ ] I/O and file system
- [ ] JSON support
- [ ] HTTP client
- [ ] Testing framework
- [ ] String formatting
- [ ] Crypto, encoding, validation modules
- [ ] CSV, random, UUID modules

### Phase 6 — Tooling

- [ ] Formatter (`jett format`)
- [ ] Language server (LSP) for editor support
- [ ] Agent Server Protocol (`--agent` flag, JSON error payloads)
- [ ] ASP type queries (`jett query --agent --type-at`)
- [ ] ASP signature lookup (`jett query --agent --signature`)
- [ ] ASP completion queries (`jett query --agent --complete-at`)
- [ ] ASP namespace listing (`jett query --agent --namespaces`)
- [ ] ASP test results (`jett test --agent`)
- [ ] ASP trace output (`jett run --agent --trace-var`)
- [ ] `tracked[T]` type wrapper with lineage recording
- [ ] `trace()` function for lineage output
- [ ] `agent_breakpoint()` with ASP query protocol
- [ ] Conditional breakpoints (`agent_breakpoint(when: ...)`)
- [ ] Breakpoint communication: stdin/stdout mode and HTTP mode
- [ ] Single-step execution from breakpoint (`step` query)
- [ ] Debug-only compilation (breakpoints compiled out in `--release`)
- [ ] Built-in CPU sampling profiler (`--agent-profile`)
- [ ] Bottleneck summary generation with ranked hotspots and suggestions
- [ ] Hot-line analysis (per-line CPU/allocation attribution within functions)
- [ ] Memory profiler (`--agent-profile-memory`) with allocation-heavy function detection
- [ ] Comparison profiling (`--agent-profile-compare`) with delta reporting
- [ ] Profiler threshold configuration (`--profile-threshold`)
- [ ] Profiler integration with Agent Server Protocol (ASP JSON output)
- [ ] Dependency manager (`jett dep add`, URL fetch, SHA-256 hashing)
- [ ] Lock file (`jett.lock`) generation and resolution
- [ ] LLVM backend for native compilation
- [ ] Jett-to-JSON and JSON-to-Jett CLI tools

### Phase 7 — Cross-Platform Compilation

- [ ] Capability lowering layer (universal interface → OS-specific syscalls)
- [ ] Platform implementations: Linux x86_64, Linux arm64
- [ ] Platform implementations: Windows x86_64
- [ ] Platform implementations: macOS arm64, macOS x86_64
- [ ] Platform implementation: WebAssembly (WASI)
- [ ] Cross-compilation from any host to any target (`--target` flag)
- [ ] Multi-target builds (`--target linux-x86_64,windows-x86_64`)
- [ ] Path normalization (forward slashes → platform-native at compile time)

### Phase 8 — C Interop (Auto-FFI)

- [ ] Built-in C header parser (typedef, struct, enum, #define, function declarations)
- [ ] `use c "header.h" as module` syntax
- [ ] Auto-translation of C types to Jett types (pointers → opaque linear handles)
- [ ] Calling convention detection and generation (cdecl, stdcall, fastcall)
- [ ] Struct layout translation (padding, alignment, platform-specific sizes)
- [ ] String encoding translation (char* ↔ string, wchar_t* ↔ string)
- [ ] Auto-naming convention conversion (PascalCase/SCREAMING_CASE → snake_case)
- [ ] Safety boundary: flagging untranslatable constructs (variadic, inline asm)

### Phase 9 — LLM Integration Testing

- [ ] Benchmark token usage vs. Python, Go, Rust for equivalent programs
- [ ] Test LLM code generation accuracy across models
- [ ] Iterate on syntax based on where LLMs make errors
- [ ] Publish prompt-friendly language documentation optimized for LLM context injection

---

## Open Questions

- **Comparison operators** — RESOLVED: symbols (`>`, `<`, `>=`, `<=`).
- **Arithmetic expressions** — RESOLVED: standard operators (`+`, `-`, `*`, `/`) plus keyword operator `modulo` (`a modulo b`) with conventional precedence. Function-call forms (`add`, `multiply`) are not provided. Operators are a universal exception to the symbol-minimalism rule — every LLM tokenizer handles them well.
- **Comments syntax** — RESOLVED: `#` for comments.
- **Effect system** — RESOLVED: capability-based I/O only. No `effects` keyword. All side effects declared via capability parameters.
- **Refinement type complexity** — RESOLVED: `where` clauses accept any pure expression (no capabilities, no mutation) that evaluates to `bool`. `value` refers to the value being constrained. Expressions use normal Jett syntax — no special intrinsics. Constraints are checked at runtime type boundaries (when a value enters the refined type). This means `where string.is_valid_json(value)` and `where list.length(value) <= 100` are both valid.
- **Dependent types** — should refinement types be able to reference other values (e.g. `type Matrix = list[list[float]] where rows is cols`)? This approaches dependent type territory and significantly increases type checker complexity.
- **Concurrency model** — RESOLVED: actor model with zero shared memory, structured concurrency with enforced join/cancel. Concurrency uses `concurrent` blocks and `spawn`/`join`/`cancel` keywords with capability parameters for I/O.
- **Memory management** — RESOLVED: linear types (move-by-default, explicit `Linear.clone()`) plus scope-bound arenas for bulk allocation. No GC, no manual `free`, no lifetime annotations.
- **Arena granularity** — should arenas be function-scoped only, or can they be passed across function boundaries? Passing arenas adds flexibility but introduces a form of lifetime tracking.
- **SoA limitations** — which struct features are compatible with `layout soa`? Can SoA structs contain other structs, or only primitive fields? How do optional fields interact with SoA layout?
- **Comptime boundaries** — what standard library functions are available at comptime? All pure functions? Only a subset? File I/O at comptime (for code generation from schemas)?
- **Actor supervision** — should there be a built-in actor supervision tree (like Erlang/OTP) for handling actor crashes? Or is the error-as-values model sufficient?
- **Interop** — RESOLVED: Auto-FFI with built-in C header parser. `use c "header.h"` imports C libraries with automatic safe wrapping. C pointers become opaque linear handles. WASM target is supported via capability lowering (Rule Set 17).
- **C++ interop** — Auto-FFI targets C headers. C++ headers with templates, classes, and name mangling are significantly harder. Should Jett support C++ headers directly, or require a C-compatible wrapper? C-only is simpler and covers most OS APIs.
- **Self-hosting timeline** — at what point is Jett mature enough to rewrite the compiler from Rust into Jett? Likely after Phase 5 (standard library) at the earliest.
- **Error handling syntax** — RESOLVED: `handle` is the only way to unwrap `result` types. `match` is reserved for user-defined enums only.
- **String building** — RESOLVED: string interpolation `"text {expr}"`. No `+` operator for strings, no `string.concat()`.
- **Testing constructs** — RESOLVED: `verify` blocks (comptime, pure functions) and `property` blocks (runtime fuzzing). No `test` blocks.
- **Method syntax** — RESOLVED: module function syntax only (`list.length(items)`, not `items.length()`).
- **Void functions** — RESOLVED: every function always has a `returns` clause. Functions that produce no value use `returns nothing`. This is consistent with the one-canonical-form principle.
- **Mutable semantics** — RESOLVED: rebinding semantics. `mutable` allows consume-and-rebind to the same name.
- **Mutual struct composition** — two structs cannot contain each other (composition is physical containment, so circular inclusion would be infinitely sized). The `mutual` block exists for functions but not for structs. Need to determine how recursive data structures (trees, linked lists, graphs) are expressed in Jett — possibly via arena-allocated indices or some form of indirection.
- **Events** — RESOLVED: Jett does not have a dedicated event system. Event-driven patterns are built from existing constructs: actors with `receive` for async event handling (pub/sub, event loops), function parameters for callbacks, and state machines for state-driven events. No `event` keyword is needed — existing constructs compose to cover these use cases.
- **TOON (Token Oriented Object Notation)** — a serialization format optimized for token efficiency, more compact than JSON. Could be added as standard library functions (`toon.serialize()`, `toon.parse()`) alongside the existing JSON support. Not a syntax change — purely a stdlib addition for LLM-friendly data interchange.
