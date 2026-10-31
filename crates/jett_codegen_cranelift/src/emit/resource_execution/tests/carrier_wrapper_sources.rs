// Exact additive suffixes from the frozen Source17 wrapper packet.
// The complete original fixture is included independently and never pruned.
const WRAPPERS: [(&str, &str); 14] = [
    (
        "01_empty_tokens",
        r###"
export function main(net: Network) returns nothing:
    list[resource_probe.TestHandle] values = empty_tokens()
    if list.length[resource_probe.TestHandle](view values) != 0:
        int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "02_absent_tokens",
        r###"
function source17_absent_optional(view candidate: optional[resource_probe.TestHandle]) returns bool:
    resource_probe.TestHandle token = view((view candidate) handle:
        return true
    )
    return false

export function main(net: Network) returns nothing:
    list[optional[resource_probe.TestHandle]] values = absent_tokens()
    if list.length[optional[resource_probe.TestHandle]](view values) != 2:
        int64 ignored = resource_probe.terminal_label()
    mutable int64 absent_count = 0
    for candidate in values:
        if source17_absent_optional(view candidate):
            absent_count = absent_count + 1
    if absent_count != 2:
        int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "03_empty_map",
        r###"
export function main(net: Network) returns nothing:
    map[string, resource_probe.TestHandle] values = empty_map()
    if map.length[string, resource_probe.TestHandle](view values) != 0:
        int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "04_absent_map",
        r###"
function source17_absent_optional(view candidate: optional[resource_probe.TestHandle]) returns bool:
    resource_probe.TestHandle token = view((view candidate) handle:
        return true
    )
    return false

export function main(net: Network) returns nothing:
    map[string, optional[resource_probe.TestHandle]] values = absent_map()
    if map.length[string, optional[resource_probe.TestHandle]](view values) != 1:
        int64 ignored = resource_probe.terminal_label()
    mutable int64 absent_count = 0
    for key, candidate in values:
        if key != "empty":
            int64 ignored = resource_probe.terminal_label()
        if source17_absent_optional(view candidate):
            absent_count = absent_count + 1
    if absent_count != 1:
        int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "05_empty_result_tokens",
        r###"
export function main(net: Network) returns nothing:
    list[resource_probe.TestHandle] values = empty_result_tokens() handle error:
        int64 ignored = resource_probe.terminal_label()
        return nothing
    if list.length[resource_probe.TestHandle](view values) != 0:
        int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "06_failed_result_tokens",
        r###"
export function main(net: Network) returns nothing:
    list[resource_probe.TestHandle] values = failed_result_tokens() handle error:
        if error != "empty":
            int64 ignored = resource_probe.terminal_label()
        return nothing
    int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "07_absent_envelope",
        r###"
function source17_absent_optional(view candidate: optional[resource_probe.TestHandle]) returns bool:
    resource_probe.TestHandle token = view((view candidate) handle:
        return true
    )
    return false

export function main(net: Network) returns nothing:
    TokenEnvelope value = absent_envelope()
    if value.marker != 17:
        int64 ignored = resource_probe.terminal_label()
    if list.length[resource_probe.TestHandle](view value.tokens) != 0:
        int64 ignored = resource_probe.terminal_label()
    if !source17_absent_optional(view value.fallback):
        int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "08_absent_box",
        r###"
export function main(net: Network) returns nothing:
    TokenBox[resource_probe.TestHandle] value = absent_box()
    if list.length[resource_probe.TestHandle](view value.items) != 0:
        int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "09_empty_choice",
        r###"
export function main(net: Network) returns nothing:
    TokenChoice value = empty_choice()
    match value:
        empty:
            return nothing
        other:
            int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "10_absent_choice",
        r###"
export function main(net: Network) returns nothing:
    TokenChoice value = absent_choice()
    match value:
        vacant(marker, tokens):
            if marker != 17:
                int64 ignored = resource_probe.terminal_label()
            if list.length[resource_probe.TestHandle](view tokens) != 0:
                int64 ignored = resource_probe.terminal_label()
            return nothing
        other:
            int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "11_absent_state",
        r###"
function source17_absent_optional(view candidate: optional[resource_probe.TestHandle]) returns bool:
    resource_probe.TestHandle token = view((view candidate) handle:
        return true
    )
    return false

export function main(net: Network) returns nothing:
    TokenState value = absent_state()
    if value at vacant:
        if value.marker != "empty":
            int64 ignored = resource_probe.terminal_label()
        if !source17_absent_optional(view value.fallback):
            int64 ignored = resource_probe.terminal_label()
        return nothing
    int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "12_absent_exact_state",
        r###"
function source17_absent_optional(view candidate: optional[resource_probe.TestHandle]) returns bool:
    resource_probe.TestHandle token = view((view candidate) handle:
        return true
    )
    return false

export function main(net: Network) returns nothing:
    TokenState at vacant value = absent_exact_state()
    if value at vacant:
        if value.marker != "empty":
            int64 ignored = resource_probe.terminal_label()
        if !source17_absent_optional(view value.fallback):
            int64 ignored = resource_probe.terminal_label()
        return nothing
    int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "13_wrong_empty_length_control",
        r###"
export function main(net: Network) returns nothing:
    list[resource_probe.TestHandle] values = empty_tokens()
    if list.length[resource_probe.TestHandle](view values) != 1:
        int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
    (
        "14_required_primitive_controls",
        r###"
export function main(net: Network) returns nothing:
    int64 answer = absent_required_controls()
    if answer != 34:
        int64 ignored = resource_probe.terminal_label()
    if absent_namespace_value != 17:
        int64 ignored = resource_probe.terminal_label()
    return nothing
"###,
    ),
];
const LIST_LENGTH: &str = r###"namespace list
export function length[T](view items: list[T]) returns int64:
    return list.__length[T](view items)
"###;
const MAP_LENGTH: &str = r###"namespace map
export function length[K, V](view items: map[K, V]) returns int64:
    return map.__length[K, V](view items)
"###;
