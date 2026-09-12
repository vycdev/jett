"""Reproducibly emit the numerical benchmark fixtures in five languages.

The compact typed Python baselines use a deliberately small common imperative
subset. The emitter preserves that program while each task's fixture oracle is
written independently. This file is evaluator infrastructure, never onboarding.
"""
from __future__ import annotations

import ast
import copy
import itertools
import json
import math
from collections import Counter
from pathlib import Path
from typing import Callable

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "benchmarks" / "tasks"
LANGS = ("jett", "python", "typescript", "go", "rust")
EXT = dict(zip(LANGS, ("jett", "py", "ts", "go", "rs")))
TASKS: list[dict[str, object]] = []


def add(name: str, title: str, statement: str, bounds: list[str], source: str,
        cases: list[list[object]], oracle: Callable[..., object], *, difficulty: str = "medium",
        record: tuple[str, tuple[str, ...]] | None = None) -> None:
    TASKS.append(dict(name=name, title=title, statement=statement, bounds=bounds,
                     source=source.strip() + "\n", cases=cases, oracle=oracle,
                     difficulty=difficulty, record=record))


def camel(name: str) -> str:
    first, *rest = name.split("_")
    return first + "".join(part.title() for part in rest)


def pascal(name: str) -> str:
    return "".join(part.title() for part in name.split("_"))


class Emitter:
    def __init__(self, task: dict[str, object], lang: str) -> None:
        self.task = task
        self.lang = lang
        self.tree = ast.parse(str(task["source"]))
        self.public = str(task["name"])
        self.current_return = "int"
        self.record = task["record"]
        self.signatures = {node.name: node for node in self.tree.body if isinstance(node, ast.FunctionDef)}

    def name(self, name: str) -> str:
        if self.lang == "typescript":
            return camel(name)
        if self.lang == "go":
            if name == "search_range":
                return "FindSearchRange"
            return pascal(name) if name == self.public else camel(name)
        return name

    def typ(self, annotation: ast.expr | str) -> str:
        value = annotation if isinstance(annotation, str) else ast.unparse(annotation)
        types = {
            "jett": {"int": "int64", "bool": "bool", "list[int]": "list[int64]", "int | None": "optional[int64]"},
            "typescript": {"int": "bigint", "bool": "boolean", "list[int]": "bigint[]", "int | None": "bigint | null"},
            "go": {"int": "int64", "bool": "bool", "list[int]": "[]int64", "int | None": "MaybeInt"},
            "rust": {"int": "i64", "bool": "bool", "list[int]": "Vec<i64>", "int | None": "Option<i64>"},
        }
        return types.get(self.lang, {}).get(value, value)

    def expr(self, node: ast.expr) -> str:
        lang = self.lang
        e = self.expr
        if isinstance(node, ast.Name):
            return self.name(node.id)
        if isinstance(node, ast.Constant):
            if node.value is None:
                return {"jett": "none", "typescript": "null", "go": "MaybeInt{}", "rust": "None"}[lang]
            if isinstance(node.value, bool):
                return str(node.value).lower()
            if isinstance(node.value, int):
                return str(node.value) + ("n" if lang == "typescript" else "")
            return json.dumps(node.value)
        if isinstance(node, ast.List):
            vals = ", ".join(e(item) for item in node.elts)
            return {"jett": f"list({vals})" if vals else "list.new[int64]()", "typescript": f"[{vals}]", "go": f"[]int64{{{vals}}}", "rust": f"vec![{vals}]"}[lang]
        if isinstance(node, ast.BinOp):
            if isinstance(node.op, (ast.FloorDiv, ast.Mod)):
                name = self.name("safe_div" if isinstance(node.op, ast.FloorDiv) else "safe_mod")
                return f"{name}({e(node.left)}, {e(node.right)})"
            op = {ast.Add: "+", ast.Sub: "-", ast.Mult: "*"}[type(node.op)]
            return f"({e(node.left)} {op} {e(node.right)})"
        if isinstance(node, ast.UnaryOp):
            op = "-" if isinstance(node.op, ast.USub) else ("not " if lang == "jett" else "!")
            return f"{op}({e(node.operand)})"
        if isinstance(node, ast.BoolOp):
            op = (" and " if isinstance(node.op, ast.And) else " or ") if lang == "jett" else (" && " if isinstance(node.op, ast.And) else " || ")
            return "(" + op.join(e(value) for value in node.values) + ")"
        if isinstance(node, ast.Compare):
            assert len(node.ops) == 1
            op = {ast.Eq: "==", ast.NotEq: "!=", ast.Lt: "<", ast.LtE: "<=", ast.Gt: ">", ast.GtE: ">="}[type(node.ops[0])]
            return f"({e(node.left)} {op} {e(node.comparators[0])})"
        if isinstance(node, ast.Call):
            assert isinstance(node.func, ast.Name)
            name = node.func.id
            args = [e(arg) for arg in node.args]
            if name == "len":
                arg = args[0]
                return {"jett": f"list.length[int64](view {arg})", "typescript": f"BigInt({arg}.length)", "go": f"int64(len({arg}))", "rust": f"i64::try_from({arg}.len()).unwrap_or(0)"}[lang]
            if name == "nth" and lang == "jett":
                args[0] = "view " + args[0]
            if name == "nth" and lang == "rust":
                args[0] = "&" + args[0]
            if name in self.signatures:
                for index, param in enumerate(self.signatures[name].args.args):
                    if ast.unparse(param.annotation) == "list[int]":
                        if lang == "jett":
                            args[index] = "view " + args[index]
                        elif lang == "rust":
                            args[index] = "&" + args[index]
            if self.record and name == self.record[0]:
                fields = self.record[1]
                if lang == "jett":
                    return name + "(" + ", ".join(f"{field}: {arg}" for field, arg in zip(fields, args)) + ")"
                if lang in ("typescript", "go", "rust"):
                    prefix = "" if lang == "typescript" else name + (" " if lang == "rust" else "")
                    return prefix + "{" + ", ".join(f"{field}: {arg}" for field, arg in zip(fields, args)) + "}"
            return f"{self.name(name)}({', '.join(args)})"
        raise ValueError(ast.dump(node))

    def block(self, body: list[ast.stmt], depth: int) -> list[str]:
        lang = self.lang
        lines: list[str] = []
        indent = "    " * depth
        semi = ";" if lang in ("typescript", "rust") else ""
        for node in body:
            if isinstance(node, ast.AnnAssign):
                assert isinstance(node.target, ast.Name) and node.value is not None
                name, typ, value = self.name(node.target.id), self.typ(node.annotation), self.expr(node.value)
                prefix = {"jett": f"mutable {typ} {name}", "typescript": f"let {name}: {typ}", "go": f"var {name} {typ}", "rust": f"let mut {name}: {typ}"}[lang]
                lines.append(f"{indent}{prefix} = {value}{semi}")
            elif isinstance(node, ast.Assign):
                assert len(node.targets) == 1
                lines.append(f"{indent}{self.expr(node.targets[0])} = {self.expr(node.value)}{semi}")
            elif isinstance(node, ast.Expr) and isinstance(node.value, ast.Call):
                call = node.value
                assert isinstance(call.func, ast.Attribute) and call.func.attr == "append"
                arr, val = self.expr(call.func.value), self.expr(call.args[0])
                forms = {"jett": f"{arr} = list.append[int64]({arr}, {val})", "typescript": f"{arr}.push({val});", "go": f"{arr} = append({arr}, {val})", "rust": f"{arr}.push({val});"}
                lines.append(indent + forms[lang])
            elif isinstance(node, ast.Return):
                assert node.value is not None
                value = self.expr(node.value)
                if self.current_return == "int | None" and not (isinstance(node.value, ast.Constant) and node.value.value is None):
                    value = {"jett": f"some({value})", "typescript": value, "go": f"MaybeInt{{Found: true, Value: {value}}}", "rust": f"Some({value})"}[lang]
                lines.append(f"{indent}return {value}{semi}")
            elif isinstance(node, (ast.If, ast.While)):
                keyword = "if" if isinstance(node, ast.If) else ("for" if lang == "go" else "while")
                condition = self.expr(node.test)
                if lang == "typescript":
                    condition = f"({condition})"
                lines.append(indent + keyword + " " + condition + (":" if lang == "jett" else " {"))
                lines.extend(self.block(node.body, depth + 1))
                if lang != "jett":
                    lines.append(indent + "}")
                if node.orelse:
                    if lang == "jett":
                        lines.append(indent + "else:")
                    else:
                        lines[-1] += " else {"
                    lines.extend(self.block(node.orelse, depth + 1))
                    if lang != "jett":
                        lines.append(indent + "}")
            else:
                raise ValueError(ast.dump(node))
        return lines

    def signature(self, node: ast.FunctionDef, public: bool = False) -> str:
        params: list[str] = []
        for arg in node.args.args:
            assert arg.annotation is not None
            typ = self.typ(arg.annotation)
            if self.lang == "rust" and typ == "Vec<i64>":
                typ = "&[i64]"
            if self.lang == "typescript" and typ == "bigint[]":
                typ = "readonly bigint[]"
            name = self.name(arg.arg)
            if self.lang == "jett" and typ == "list[int64]" and node.name != self.public:
                name = "view " + name
            params.append(f"{name} {typ}" if self.lang == "go" else f"{name}: {typ}")
        assert node.returns is not None
        name, returns = self.name(node.name), self.typ(node.returns)
        prefix = {"jett": "function ", "typescript": "export function " if public else "function ", "go": "func ", "rust": "pub fn " if public else "fn "}[self.lang]
        join = {"jett": " returns ", "typescript": ": ", "go": " ", "rust": " -> "}[self.lang]
        return prefix + name + "(" + ", ".join(params) + ")" + join + returns

    def helpers(self) -> str:
        source = str(self.task["source"])
        out: list[str] = []
        if "nth(" in source:
            out.append({
                "jett": "function nth(view values: list[int64], index: int64) returns int64:\n    int64 item = list.get[int64](view values, index) handle:\n        default 0\n    return item",
                "typescript": "function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }",
                "go": "func nth(values []int64, index int64) int64 { return values[index] }",
                "rust": "fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }",
            }[self.lang])
        for name, token in (("safe_div", "//"), ("safe_mod", "%")):
            if token not in source:
                continue
            op = "/" if token == "//" else ("modulo" if self.lang == "jett" else "%")
            op_name = self.name(name)
            out.append({
                "jett": f"function {op_name}(left: int64, right: int64) returns int64:\n    if right == 0:\n        return 0\n    return left {op} right",
                "typescript": f"function {op_name}(left: bigint, right: bigint): bigint {{ return left {op} right; }}",
                "go": f"func {op_name}(left int64, right int64) int64 {{ return left {op} right }}",
                "rust": f"fn {op_name}(left: i64, right: i64) -> i64 {{ left {op} right }}",
            }[self.lang])
        return "\n".join(out)

    def record_source(self) -> str:
        if not self.record:
            return ""
        name, fields = self.record
        if self.lang == "python":
            return "from dataclasses import dataclass\n\n@dataclass(frozen=True)\nclass " + name + ":\n" + "\n".join("    " + field + ": int" for field in fields)
        if self.lang == "jett":
            return f"struct {name}:\n" + "\n".join(f"    {field}: int64" for field in fields)
        if self.lang == "typescript":
            return f"export type {name} = {{ " + "; ".join(f"readonly {field}: bigint" for field in fields) + " };"
        if self.lang == "go":
            return f"type {name} struct {{ " + "; ".join(f"{field} int64" for field in fields) + " }"
        return f"#[derive(Debug, PartialEq, Eq)]\npub struct {name} {{ " + ", ".join(f"pub {field}: i64" for field in fields) + " }"

    def source(self) -> str:
        if self.lang == "python":
            helpers = "def nth(values: list[int], index: int) -> int:\n    return values[index]\n\n" if "nth(" in str(self.task["source"]) else ""
            return self.record_source() + "\n\n" + helpers + str(self.task["source"])
        parts = [{"jett": "namespace benchmark", "go": "package benchmark"}.get(self.lang, ""), self.record_source()]
        if "int | None" in str(self.task["source"]) and self.lang == "go":
            parts.append("type MaybeInt struct { Found bool; Value int64 }")
        parts.append(self.helpers())
        for node in self.tree.body:
            assert isinstance(node, ast.FunctionDef)
            self.current_return = ast.unparse(node.returns)
            parts.append(self.signature(node, node.name == self.public) + (":" if self.lang == "jett" else " {"))
            parts.extend(self.block(node.body, 1))
            if self.lang != "jett":
                parts.append("}")
        return "\n".join(part for part in parts if part) + "\n"

    def public_signature(self) -> str:
        node = self.signatures[self.public]
        if self.lang == "python":
            args = ", ".join(f"{arg.arg}: {ast.unparse(arg.annotation)}" for arg in node.args.args)
            sig = f"def {node.name}({args}) -> {ast.unparse(node.returns)}"
        else:
            sig = self.signature(node, True)
        prefix = {"jett": "namespace benchmark\n\n", "go": "package benchmark\n\n"}.get(self.lang, "")
        record = self.record_source()
        if self.lang == "go" and ast.unparse(node.returns) == "int | None":
            record = "type MaybeInt struct { Found bool; Value int64 }"
        return prefix + (record + "\n\n" if record else "") + sig


def literal(value: object, lang: str, *, argument: bool = False) -> str:
    if value is None:
        return {"python": "None", "jett": "none", "typescript": "null", "go": "MaybeInt{}", "rust": "None"}[lang]
    if isinstance(value, bool):
        return str(value) if lang == "python" else str(value).lower()
    if isinstance(value, int):
        return str(value) + ("n" if lang == "typescript" else "")
    if isinstance(value, list):
        joined = ", ".join(literal(item, lang) for item in value)
        return {"python": f"[{joined}]", "jett": f"list({joined})", "typescript": f"[{joined}]", "go": f"[]int64{{{joined}}}", "rust": f"{'&' if argument else ''}[{joined}]"}[lang]
    raise ValueError(value)


def hidden_source(task: dict[str, object], lang: str, fixtures: list[dict[str, object]]) -> str:
    emitter = Emitter(task, lang)
    node = emitter.signatures[str(task["name"])]
    kind = ast.unparse(node.returns)
    name = emitter.name(str(task["name"]))
    record = task["record"]
    prefix = {
        "python": f"from solution import {name}\n",
        "typescript": f'import {{ {name} }} from "./solution.js";\n',
        "go": 'package benchmark\n\nimport ("testing"; "reflect")\n\nfunc TestHidden(t *testing.T) {\n    _ = reflect.DeepEqual\n',
        "rust": 'include!("solution.rs");\n\n#[test]\nfn hidden_cases() {\n',
        "jett": "",
    }[lang]
    if lang == "jett":
        if kind == "list[int]":
            prefix += 'function fixture_label(values: list[int64]) returns string:\n    mutable string label = ""\n    for value in values:\n        label = "{label}{value},"\n    return label\n'
        elif kind == "int | None":
            prefix += "function fixture_label(outcome: optional[int64]) returns string:\n    int64 value = outcome handle:\n        return \"none\"\n    return string.from_int64(value)\n"
        elif record:
            prefix += f"function fixture_label(outcome: {record[0]}) returns string:\n    return \"" + ",".join("{outcome." + field + "}" for field in record[1]) + "\"\n"
    lines = [prefix]
    for idx, fixture in enumerate(fixtures):
        inputs, expected = fixture["inputs"], fixture["expected"]
        call_args = [literal(value, lang, argument=True) for value in inputs]
        if lang in ("python", "go"):
            for arg_idx, value in enumerate(inputs):
                if isinstance(value, list):
                    local = f"input_{idx}_{arg_idx}"
                    lines.append(("    " if lang == "go" else "") + local + (" := " if lang == "go" else ": list[int] = ") + literal(value, lang))
                    call_args[arg_idx] = local
        call = name + "(" + ", ".join(call_args) + ")"
        if lang == "jett":
            # Keep each verifier well below compiler policy limits.
            if idx % 8 == 0:
                lines.append(f"verify numerical_cases_{idx // 8}:")
            if kind == "list[int]":
                wanted = json.dumps("".join(f"{item}," for item in expected))
                call = f"fixture_label({call})"
            elif kind == "int | None":
                wanted = json.dumps("none" if expected is None else str(expected))
                call = f"fixture_label({call})"
            elif record:
                wanted = json.dumps(",".join(str(expected[field]) for field in record[1]))
                call = f"fixture_label({call})"
            else:
                wanted = literal(expected, lang)
            lines.append(f"    assert {call} == {wanted}")
        elif lang == "python":
            if record:
                lines.append(f"case_{idx} = {call}")
                lines.extend(f"assert case_{idx}.{field} == {expected[field]}, 'case {idx}: {field}'" for field in record[1])
            else:
                lines.append(f"assert {call} == {literal(expected, lang)}, 'case {idx}'")
        elif lang == "typescript":
            if kind == "list[int]":
                lines.append(f"const case{idx} = {call};")
                lines.append(f"const wanted{idx}: readonly bigint[] = {literal(expected, lang)};")
                condition = f"case{idx}.length !== wanted{idx}.length || wanted{idx}.some((value, index) => case{idx}[index] !== value)"
            elif record:
                lines.append(f"const case{idx} = {call};")
                condition = " || ".join(f"case{idx}.{field} !== {literal(expected[field], lang)}" for field in record[1])
            else:
                condition = f"{call} !== {literal(expected, lang)}"
            lines.append(f'if ({condition}) throw new Error("case {idx}");')
        elif lang == "go":
            if record:
                wanted = record[0] + "{" + ", ".join(f"{field}: {expected[field]}" for field in record[1]) + "}"
            elif kind == "int | None" and expected is not None:
                wanted = f"MaybeInt{{Found: true, Value: {expected}}}"
            else:
                wanted = literal(expected, lang)
            if kind == "list[int]":
                condition = f"!reflect.DeepEqual(append([]int64{{}}, got...), {wanted})"
            elif kind == "int | None":
                condition = "got.Found" if expected is None else f"!got.Found || got.Value != {expected}"
            else:
                condition = f"got != ({wanted})"
            lines.append(f'    if got := {call}; {condition} {{ t.Fatalf("case {idx}: got %#v", got) }}')
        else:
            if record:
                wanted = record[0] + " {" + ", ".join(f"{field}: {expected[field]}" for field in record[1]) + "}"
            elif kind == "int | None" and expected is not None:
                wanted = f"Some({expected})"
            elif kind == "list[int]":
                wanted = "vec!" + literal(expected, lang)
            else:
                wanted = literal(expected, lang)
            lines.append(f'    assert_eq!({call}, {wanted}, "case {idx}");')
        if lang in ("python", "go"):
            for arg_idx, value in enumerate(inputs):
                if isinstance(value, list):
                    local = f"input_{idx}_{arg_idx}"
                    wanted = literal(value, lang)
                    if lang == "python":
                        lines.append(f"assert {local} == {wanted}, 'case {idx}: input mutation'")
                    else:
                        lines.append(f'    if !reflect.DeepEqual({local}, {wanted}) {{ t.Fatalf("case {idx}: input mutation") }}')
    if lang in ("rust", "go"):
        lines.append("}")
    return "\n".join(lines) + "\n"


def emit_all() -> None:
    adapter_template = json.loads((DEST / "first_duplicate" / "task.json").read_text())["adapters"]
    for task in TASKS:
        name = str(task["name"])
        anchor_inputs, anchor_expected = ANCHORS[name]
        assert task["oracle"](*anchor_inputs) == anchor_expected, f"Independent anchor failed: {name}"
        directory = DEST / ("num_" + name)
        directory.mkdir(exist_ok=True)
        cases = list({json.dumps(case): case for case in task["cases"]}.values())
        assert len(cases) >= 10
        oracle = task["oracle"]
        fixtures = [{"inputs": case, "expected": oracle(*case)} for case in cases]
        document = {"id": "num_" + name, "version": "1.0.0", "title": task["title"], "category": "numerical_sequence_algorithms", "difficulty": task["difficulty"], "statement": task["statement"], "constraints": task["bounds"] + ["Inputs obey the stated domain; do not mutate caller-owned inputs.", "The required return value fits signed 64-bit integers; choose arithmetic that keeps intermediate values within that range. TypeScript uses bigint for integer values.", "Do not perform input/output, throw, panic, add tests, or suppress static diagnostics."], "adapters": {}}
        for lang in LANGS:
            emitter = Emitter(task, lang)
            adapter = copy.deepcopy(adapter_template[lang])
            adapter["signature"] = emitter.public_signature()
            # The type-escape policies are language-general; absence wording is not.
            for policy in adapter.get("forbidden_patterns", []):
                policy["message"] = policy["message"].split("; return")[0]
            document["adapters"][lang] = adapter
            (directory / adapter["baseline"]).write_text(emitter.source(), encoding="utf-8", newline="\n")
            (directory / adapter["hidden"]).write_text(hidden_source(task, lang, fixtures), encoding="utf-8", newline="\n")
        (directory / "task.json").write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8", newline="\n")
        (directory / "cases.json").write_text(json.dumps({"schema_version": 1, "cases": fixtures}, indent=2) + "\n", encoding="utf-8", newline="\n")
        (directory / "pyrightconfig.json").write_text('{"include": ["solution.py", "hidden.py"], "pythonVersion": "3.12", "typeCheckingMode": "strict"}\n', encoding="utf-8", newline="\n")
    fixture_count = sum(len({json.dumps(case) for case in task["cases"]}) for task in TASKS)
    print(f"Generated {len(TASKS)} numerical tasks, {len(TASKS) * 5} adapters, {fixture_count} distinct shared fixtures")


# Task definitions follow. Oracles intentionally favor clear independent methods.
add("prime_status", "Primality of a bounded integer",
    "Return true exactly when n is prime: an integer greater than 1 with no positive divisors other than 1 and itself.",
    ["-100 <= n <= 1000000"], '''
def prime_status(n: int) -> bool:
    if n < 2:
        return False
    divisor: int = 2
    while divisor * divisor <= n:
        if n % divisor == 0:
            return False
        divisor = divisor + 1
    return True
''', [[n] for n in [-100, -1, 0, 1, 2, 3, 4, 9, 25, 49, 97, 121, 997, 1024, 65521, 999983, 1000000]],
    lambda n: n > 1 and all(n % d for d in range(2, math.isqrt(max(n, 0)) + 1)), difficulty="easy")

add("divisor_sum", "Sum all positive divisors",
    "Return the sum of every positive divisor of n, including 1 and n. Count a square-root divisor only once.",
    ["1 <= n <= 100000"], '''
def divisor_sum(n: int) -> int:
    total: int = 0
    divisor: int = 1
    while divisor * divisor <= n:
        if n % divisor == 0:
            total = total + divisor
            partner: int = n // divisor
            if partner != divisor:
                total = total + partner
        divisor = divisor + 1
    return total
''', [[n] for n in [1, 2, 3, 4, 6, 12, 16, 25, 36, 49, 64, 97, 120, 360, 99991, 100000]],
    lambda n: sum(d for d in range(1, n + 1) if n % d == 0))

add("totient", "Count coprime positive integers",
    "Return Euler's totient of n: the count of integers k with 1 <= k <= n and gcd(k, n) = 1. In particular, totient(1) = 1.",
    ["1 <= n <= 10000"], '''
def totient(n: int) -> int:
    remaining: int = n
    count: int = n
    factor: int = 2
    while factor * factor <= remaining:
        if remaining % factor == 0:
            count = count - count // factor
            while remaining % factor == 0:
                remaining = remaining // factor
        factor = factor + 1
    if remaining > 1:
        count = count - count // remaining
    return count
''', [[n] for n in [1, 2, 3, 4, 5, 6, 8, 9, 10, 12, 30, 36, 49, 97, 210, 1024, 9999, 10000]],
    lambda n: sum(math.gcd(k, n) == 1 for k in range(1, n + 1)))

add("modular_power", "Bounded modular exponentiation",
    "Return (base raised to exponent) modulo modulus as a nonnegative integer. An exponent of zero denotes 1, including 0 raised to 0; modulus 1 therefore always returns 0.",
    ["0 <= base <= 1000000", "0 <= exponent <= 60", "1 <= modulus <= 1000000"], '''
def modular_power(base: int, exponent: int, modulus: int) -> int:
    power: int = base % modulus
    remaining: int = exponent
    answer: int = 1 % modulus
    while remaining > 0:
        if remaining % 2 == 1:
            answer = answer * power % modulus
        power = power * power % modulus
        remaining = remaining // 2
    return answer
''', [[0,0,7],[0,1,7],[7,0,1],[2,10,1000],[3,4,5],[999999,60,1000000],[1000000,60,999983],[1,60,2],[6,5,8],[12,13,17],[5,8,25],[17,11,97],[2,60,99991],[999,3,1000]],
    lambda base, exponent, modulus: pow(base, exponent, modulus))

add("digit_checksum", "Alternating decimal digit checksum",
    "Starting at the rightmost decimal digit of n, alternately add, subtract, add, subtract each digit and return the signed sum. Zero has checksum 0.",
    ["0 <= n <= 1000000000000"], '''
def digit_checksum(n: int) -> int:
    remaining: int = n
    sign: int = 1
    checksum: int = 0
    while remaining > 0:
        checksum = checksum + sign * (remaining % 10)
        sign = -sign
        remaining = remaining // 10
    return checksum
''', [[n] for n in [0,1,9,10,11,12,123,1234,90909,100001,987654321,1000000000000,999999999999,10101010101]],
    lambda n: sum(int(digit) * (-1 if i % 2 else 1) for i, digit in enumerate(reversed(str(n)))), difficulty="easy")

add("integer_sqrt", "Exact floor square root",
    "Return the greatest nonnegative integer r for which r*r <= n. The result must be exact, including inputs just below and above perfect squares.",
    ["0 <= n <= 1000000000000"], '''
def integer_sqrt(n: int) -> int:
    lower: int = 0
    upper: int = 1000001
    while lower + 1 < upper:
        middle: int = (lower + upper) // 2
        if middle * middle <= n:
            lower = middle
        else:
            upper = middle
    return lower
''', [[n] for n in [0,1,2,3,4,8,9,10,15,16,17,99980000,99980001,99980002,999999999999,1000000000000]],
    math.isqrt)


def collatz_oracle(n: int, limit: int) -> int | None:
    for steps in range(limit + 1):
        if n == 1:
            return steps
        n = n // 2 if n % 2 == 0 else 3 * n + 1
    return None


add("collatz_steps", "Budgeted Collatz trajectory",
    "Starting with n, repeatedly divide an even value by 2 or replace an odd value by 3*n+1. Return the number of transitions needed to first reach 1 if this is at most limit, otherwise return the empty optional. Starting at 1 returns 0 even with a zero budget.",
    ["1 <= n <= 1000000", "0 <= limit <= 200"], '''
def collatz_steps(n: int, limit: int) -> int | None:
    current: int = n
    steps: int = 0
    while current != 1:
        if steps == limit:
            return None
        if current % 2 == 0:
            current = current // 2
        else:
            current = 3 * current + 1
        steps = steps + 1
    return steps
''', [[1,0],[2,0],[2,1],[3,6],[3,7],[6,8],[7,15],[7,16],[27,110],[27,111],[1000000,200],[999999,200],[1024,9],[1024,10]], collatz_oracle)

add("fibonacci", "Exact bounded Fibonacci number",
    "Return F(n), where F(0)=0, F(1)=1 and F(n)=F(n-1)+F(n-2) for n>=2.",
    ["0 <= n <= 70"], '''
def fibonacci(n: int) -> int:
    previous: int = 0
    current: int = 1
    index: int = 0
    while index < n:
        following: int = previous + current
        previous = current
        current = following
        index = index + 1
    return previous
''', [[n] for n in [0,1,2,3,4,5,8,10,20,30,40,50,60,70]],
    lambda n: sum(math.comb(n-k-1,k) for k in range((n-1)//2+1)) if n else 0, difficulty="easy")

add("binomial", "Exact binomial coefficient",
    "Return the number of k-element subsets of an n-element set. Return 0 when k>n. Empty subsets have count 1, including n=0.",
    ["0 <= n <= 30", "0 <= k <= 35"], '''
def binomial(n: int, k: int) -> int:
    if k > n:
        return 0
    answer: int = 1
    index: int = 1
    while index <= k:
        answer = answer * (n - index + 1) // index
        index = index + 1
    return answer
''', [[0,0],[0,1],[1,0],[1,1],[1,2],[5,2],[5,3],[10,1],[10,9],[20,10],[30,15],[30,0],[30,30],[30,35]],
    lambda n, k: math.comb(n,k) if k <= n else 0)


def coin_oracle(coins: list[int], amount: int) -> int:
    reachable = {0}
    for depth in range(amount + 1):
        if amount in reachable:
            return depth
        reachable = {total + coin for total in reachable for coin in coins if total + coin <= amount}
    return -1


add("coin_change", "Minimum number of reusable coins",
    "Return the minimum number of coins needed to sum exactly to amount using unlimited copies of the supplied positive denominations. Return -1 when impossible. The empty sum uses zero coins. Duplicate denominations do not change the answer.",
    ["0 <= amount <= 100", "coins has at most 10 elements; each denomination is between 1 and 50"], '''
def coin_change(coins: list[int], amount: int) -> int:
    costs: list[int] = [0]
    total: int = 1
    while total <= amount:
        best: int = 1000
        index: int = 0
        while index < len(coins):
            coin: int = nth(coins, index)
            if coin <= total:
                prior: int = nth(costs, total - coin)
                if prior + 1 < best:
                    best = prior + 1
            index = index + 1
        costs.append(best)
        total = total + 1
    answer: int = nth(costs, amount)
    if answer == 1000:
        return -1
    return answer
''', [[[],0],[[],1],[[1],0],[[1],7],[[2],3],[[2],8],[[1,3,4],6],[[5,2],11],[[3,7],10],[[3,7],5],[[2,2,4],8],[[50],100],[[7,10,25],99],[[9,6,5,1],11]], coin_oracle, difficulty="hard")


def staircase_oracle(height: int, blocked: list[int]) -> int:
    ways = [0] * (height + 1)
    ways[0] = 1
    for position in range(1, height + 1):
        if position not in blocked:
            ways[position] = ways[position-1] + (ways[position-2] if position > 1 else 0)
    return ways[height]


add("staircase_blocked", "Count paths up a staircase with forbidden steps",
    "Count paths from step 0 to height using jumps of exactly 1 or 2. A path may not land on any step in blocked, but may jump over it. The empty path to height 0 counts as one. The order and duplicates of blocked do not matter.",
    ["0 <= height <= 40", "blocked has at most 40 entries, all in [1, height]"], '''
def has_step(blocked: list[int], step: int) -> bool:
    index: int = 0
    while index < len(blocked):
        if nth(blocked, index) == step:
            return True
        index = index + 1
    return False

def staircase_blocked(height: int, blocked: list[int]) -> int:
    previous: int = 0
    current: int = 1
    step: int = 1
    while step <= height:
        following: int = current + previous
        if has_step(blocked, step):
            following = 0
        previous = current
        current = following
        step = step + 1
    return current
''', [[0,[]],[1,[]],[1,[1]],[2,[1]],[2,[2]],[3,[]],[4,[2]],[5,[2,3]],[6,[5,1]],[10,[4,4,7]],[20,[]],[40,[]],[40,[39]],[8,[1,2]]], staircase_oracle, difficulty="hard")


def knapsack_oracle(weights: list[int], values: list[int], capacity: int) -> int:
    return max((sum(values[i] for i in range(len(values)) if mask >> i & 1)
                for mask in range(1 << len(values))
                if sum(weights[i] for i in range(len(values)) if mask >> i & 1) <= capacity), default=0)


add("knapsack_value", "Zero-one knapsack value",
    "Items are aligned by index in weights and values. Each item may be chosen at most once. Return the greatest total value whose total weight is at most capacity. Choosing no items is allowed; identical items at different indices are distinct.",
    ["weights and values have the same length, at most 12", "1 <= each weight <= 40; 0 <= each value <= 100", "0 <= capacity <= 40"], '''
def knapsack_value(weights: list[int], values: list[int], capacity: int) -> int:
    costs: list[int] = []
    size: int = 0
    while size <= capacity:
        costs.append(0)
        size = size + 1
    item: int = 0
    while item < len(weights):
        next_costs: list[int] = []
        room: int = 0
        weight: int = nth(weights, item)
        value: int = nth(values, item)
        while room <= capacity:
            best: int = nth(costs, room)
            if weight <= room:
                candidate: int = nth(costs, room - weight) + value
                if candidate > best:
                    best = candidate
            next_costs.append(best)
            room = room + 1
        costs = next_costs
        item = item + 1
    return nth(costs, capacity)
''', [[[],[],0],[[],[],8],[[1],[7],0],[[5],[10],4],[[5],[10],5],[[2,3,4],[4,5,7],5],[[2,2,2],[3,3,3],4],[[1,1,1],[0,5,7],2],[[6,3,4,2],[30,14,16,9],10],[[10,20,30],[60,100,100],40],[[1,3,4],[1,4,5],7],[[40],[100],40],[[7,6,5,4,3,2],[5,6,7,8,9,10],12]], knapsack_oracle, difficulty="hard")


def lis_oracle(values: list[int]) -> int:
    # Independent patience-sorting oracle; baseline below uses quadratic DP.
    import bisect
    tails: list[int] = []
    for value in values:
        position = bisect.bisect_left(tails, value)
        if position == len(tails):
            tails.append(value)
        else:
            tails[position] = value
    return len(tails)


SEQ_CASES = [[], [0], [-7], [1,2,3,4], [4,3,2,1], [2,2,2], [-3,-2,-1], [3,1,2,1,4], [5,-1,5,-1,5], [0,-1,2,-3,4,-5], [9,3,7,1,8,2,6,4,5], [100,-100,100,0], [0]*20, list(range(20)), list(range(20,-1,-1)), [1000,-1000]*50, list(range(100,0,-1))]

add("lis_length", "Strictly increasing subsequence length",
    "Return the maximum length of a strictly increasing subsequence. A subsequence preserves relative input order but need not be contiguous. Equal values cannot extend a strictly increasing subsequence. The empty input returns 0.",
    ["values has at most 100 entries, each in [-1000, 1000]"], '''
def lis_length(values: list[int]) -> int:
    lengths: list[int] = []
    best: int = 0
    index: int = 0
    while index < len(values):
        ending: int = 1
        prior: int = 0
        while prior < index:
            if nth(values, prior) < nth(values, index):
                candidate: int = nth(lengths, prior) + 1
                if candidate > ending:
                    ending = candidate
            prior = prior + 1
        lengths.append(ending)
        if ending > best:
            best = ending
        index = index + 1
    return best
''', [[values] for values in SEQ_CASES], lis_oracle, difficulty="hard")

add("max_subarray", "Maximum nonempty contiguous sum",
    "Return the largest sum of any nonempty contiguous slice. Return the empty optional for an empty input. An all-negative list returns its greatest single element, not zero.",
    ["values has at most 100 entries, each in [-1000, 1000]"], '''
def max_subarray(values: list[int]) -> int | None:
    if len(values) == 0:
        return None
    ending: int = nth(values, 0)
    best: int = ending
    index: int = 1
    while index < len(values):
        value: int = nth(values, index)
        if ending > 0:
            ending = ending + value
        else:
            ending = value
        if ending > best:
            best = ending
        index = index + 1
    return best
''', [[values] for values in SEQ_CASES + [[-8,-2,-3],[-2,1,-3,4,-1,2,1,-5,4],[5,-10,6]]],
    lambda values: max((sum(values[i:j]) for i in range(len(values)) for j in range(i+1,len(values)+1)), default=None))

add("max_product_pair", "Greatest product of two distinct positions",
    "Return the largest product obtained from two different input indices. Equal values at distinct indices may be paired. Return the empty optional when fewer than two values are present.",
    ["values has at most 100 entries, each in [-10000, 10000]"], '''
def max_product_pair(values: list[int]) -> int | None:
    if len(values) < 2:
        return None
    best: int = nth(values, 0) * nth(values, 1)
    first: int = 0
    while first < len(values):
        second: int = first + 1
        while second < len(values):
            candidate: int = nth(values, first) * nth(values, second)
            if candidate > best:
                best = candidate
            second = second + 1
        first = first + 1
    return best
''', [[values] for values in SEQ_CASES + [[-10,-9,1,2],[0,-1],[-3,4],[-10000,10000,9999,-9999]]],
    lambda values: max((a*b for a,b in itertools.combinations(values,2)), default=None))

add("prefix_balances", "Running balances including the opening value",
    "Starting from opening, return every running balance after applying values left to right, including opening itself as the first element. Thus the output always has len(values)+1 entries.",
    ["-10000 <= opening <= 10000", "values has at most 100 entries, each in [-1000, 1000]"], '''
def prefix_balances(values: list[int], opening: int) -> list[int]:
    balances: list[int] = [opening]
    balance: int = opening
    index: int = 0
    while index < len(values):
        balance = balance + nth(values, index)
        balances.append(balance)
        index = index + 1
    return balances
''', [[values, [0,7,-3][i%3]] for i,values in enumerate(SEQ_CASES)],
    lambda values, opening: [opening] + [opening + total for total in itertools.accumulate(values)], difficulty="easy")

add("equilibrium_index", "First equilibrium index",
    "Return the smallest zero-based index such that the sum strictly before it equals the sum strictly after it. The element at the index is excluded from both sums. Return the empty optional when no such index exists.",
    ["values has at most 100 entries, each in [-1000, 1000]"], '''
def equilibrium_index(values: list[int]) -> int | None:
    remaining: int = 0
    index: int = 0
    while index < len(values):
        remaining = remaining + nth(values, index)
        index = index + 1
    before: int = 0
    index = 0
    while index < len(values):
        value: int = nth(values, index)
        remaining = remaining - value
        if before == remaining:
            return index
        before = before + value
        index = index + 1
    return None
''', [[values] for values in SEQ_CASES + [[1,3,5,2,2],[-7,1,5,2,-4,3,0],[0,0,0],[2,-2,7],[7,2,-2]]],
    lambda values: next((i for i in range(len(values)) if sum(values[:i]) == sum(values[i+1:])), None))

add("window_peak", "Largest fixed-width window sum",
    "Return the largest sum of a contiguous window containing exactly width elements. Return the empty optional when width is zero or exceeds the input length. Negative window sums are valid.",
    ["0 <= width <= 101", "values has at most 100 entries, each in [-1000, 1000]"], '''
def window_peak(values: list[int], width: int) -> int | None:
    if width == 0:
        return None
    if width > len(values):
        return None
    total: int = 0
    index: int = 0
    while index < width:
        total = total + nth(values, index)
        index = index + 1
    best: int = total
    while index < len(values):
        total = total + nth(values, index) - nth(values, index - width)
        if total > best:
            best = total
        index = index + 1
    return best
''', [[[],0],[[],1],[[7],0],[[7],1],[[7],2],[[1,2,3,4],2],[[-5,-2,-7],2],[[2,-1,2,-1,2],3],[[5,-9,5],1],[[5,-9,5],3],[[0,0,0],2],[[10,-5,-5,10],2],[list(range(20)),5],[list(range(20,-1,-1)),7]],
    lambda values,width: max((sum(values[i:i+width]) for i in range(len(values)-width+1)), default=None) if 0 < width <= len(values) else None)

add("longest_positive_run", "Longest contiguous run of positive values",
    "Return the length of the longest contiguous run of values strictly greater than zero. Zero and negative numbers break a run. An empty input returns 0.",
    ["values has at most 100 entries, each in [-1000, 1000]"], '''
def longest_positive_run(values: list[int]) -> int:
    current: int = 0
    best: int = 0
    index: int = 0
    while index < len(values):
        if nth(values, index) > 0:
            current = current + 1
            if current > best:
                best = current
        else:
            current = 0
        index = index + 1
    return best
''', [[values] for values in SEQ_CASES + [[1,2,0,3,4,5],[1,0,1,0],[0,1,2,3]]],
    lambda values: max((len(list(group)) for key,group in itertools.groupby(values,key=lambda value:value>0) if key),default=0), difficulty="easy")

add("rotate_left", "Rotate a sequence left by a nonnegative distance",
    "Return a new list rotated left by distance positions. Rotation wraps and distances greater than the length are reduced modulo the length. Empty input returns an empty list.",
    ["0 <= distance <= 1000000", "values has at most 100 entries, each in [-1000, 1000]"], '''
def rotate_left(values: list[int], distance: int) -> list[int]:
    rotated: list[int] = []
    count: int = len(values)
    if count == 0:
        return rotated
    shift: int = distance % count
    index: int = 0
    while index < count:
        position: int = (index + shift) % count
        rotated.append(nth(values, position))
        index = index + 1
    return rotated
''', [[[],0],[[],100],[[7],0],[[7],999],[[1,2,3],0],[[1,2,3],1],[[1,2,3],2],[[1,2,3],3],[[1,2,3],4],[[-1,0,1,0],7],[[2,2,3,2],2],[list(range(20)),1000000],[list(range(7)),999999]],
    lambda values,distance: values[distance%len(values):] + values[:distance%len(values)] if values else [])

def water_oracle(heights: list[int]) -> int:
    # Independent horizontal-layer counting, not the baseline's vertical profiles.
    total = 0
    for level in range(1, max(heights, default=0) + 1):
        walls = [i for i, height in enumerate(heights) if height >= level]
        if len(walls) >= 2:
            total += sum(heights[i] < level for i in range(walls[0], walls[-1] + 1))
    return total


add("trapped_water", "Water trapped by unit-width elevation bars",
    "Each nonnegative height describes a unit-width bar, in left-to-right order. Return the total unit volume of water that remains after rain, assuming water escapes beyond both ends. Empty inputs, fewer than three bars, and flat plateaus trap zero water.",
    ["heights has at most 100 entries, each in [0, 1000]"], '''
def trapped_water(heights: list[int]) -> int:
    left_peaks: list[int] = []
    peak: int = 0
    index: int = 0
    while index < len(heights):
        height: int = nth(heights, index)
        if height > peak:
            peak = height
        left_peaks.append(peak)
        index = index + 1
    peak = 0
    volume: int = 0
    while index > 0:
        index = index - 1
        height: int = nth(heights, index)
        if height > peak:
            peak = height
        ceiling: int = nth(left_peaks, index)
        if peak < ceiling:
            ceiling = peak
        volume = volume + ceiling - height
    return volume
''', [[values] for values in [[],[0],[5],[5,0],[0,5],[3,3,3],[3,0,3],[5,0,2],[2,0,5],[3,0,2,0,4],[0,1,0,2,1,0,1,3,2,1,2,1],[5,4,3,2,1],[1,2,3,4,5],[4,2,0,3,2,5],[3,0,0,3],[1000]+[0]*98+[1000],[1000,0]*50,[0]*100]],
    water_oracle, difficulty="hard")

add("stable_partition", "Stable negative versus nonnegative partition",
    "Return all negative values followed by all nonnegative values, preserving relative order within both groups. Zero belongs to the second group. Do not sort either group.",
    ["values has at most 100 entries, each in [-1000, 1000]"], '''
def stable_partition(values: list[int]) -> list[int]:
    output: list[int] = []
    index: int = 0
    while index < len(values):
        value: int = nth(values, index)
        if value < 0:
            output.append(value)
        index = index + 1
    index = 0
    while index < len(values):
        value: int = nth(values, index)
        if value >= 0:
            output.append(value)
        index = index + 1
    return output
''', [[values] for values in SEQ_CASES + [[-1,3,-2,0,2,-3],[0,-1,0,-2,0]]],
    lambda values: [v for v in values if v<0] + [v for v in values if v>=0], difficulty="easy")

add("sorted_insert", "Insert into a sorted list without removing duplicates",
    "Return a new nondecreasing list containing all values plus one extra occurrence of value. Inputs are already nondecreasing. Existing duplicates must be preserved.",
    ["values has at most 100 entries, each in [-1000, 1000], in nondecreasing order", "-1000 <= value <= 1000"], '''
def sorted_insert(values: list[int], value: int) -> list[int]:
    output: list[int] = []
    inserted: bool = False
    index: int = 0
    while index < len(values):
        item: int = nth(values, index)
        if not inserted:
            if value <= item:
                output.append(value)
                inserted = True
        output.append(item)
        index = index + 1
    if not inserted:
        output.append(value)
    return output
''', [[[],3],[[1],0],[[1],1],[[1],2],[[1,2,3],0],[[1,2,3],2],[[1,2,3],4],[[0,0,0],0],[[-5,-2,0,3],-3],[[-5,-2,0,3],0],[[1,1,2,2],1],[list(range(20)),10],[[1000],-1000]],
    lambda values,value: sorted(values+[value]), difficulty="easy")

add("search_range", "Inclusive range of a target in sorted data",
    "Return SearchRange(first, last) containing the zero-based first and last positions of target in nondecreasing values. Both positions are inclusive. If absent, return first=-1 and last=-1.",
    ["values has at most 100 entries, each in [-1000, 1000], in nondecreasing order", "-1000 <= target <= 1000"], '''
def search_range(values: list[int], target: int) -> SearchRange:
    first: int = -1
    last: int = -1
    index: int = 0
    while index < len(values):
        if nth(values, index) == target:
            if first == -1:
                first = index
            last = index
        index = index + 1
    return SearchRange(first, last)
''', [[[],0],[[1],1],[[1],0],[[1],2],[[1,2,2,2,3],2],[[1,1,2],1],[[1,2,2],2],[[0,0,0],0],[[-5,-2,-2,0,3],-2],[[-5,-2,0,3],1],[list(range(20)),10],[list(range(20)),20],[[7]*100,7]],
    lambda values,target: {"first":values.index(target),"last":len(values)-1-values[::-1].index(target)} if target in values else {"first":-1,"last":-1}, record=("SearchRange",("first","last")))

add("inversion_count", "Count out-of-order index pairs",
    "Return the number of index pairs i<j with values[i]>values[j]. Equal values do not form inversions. Count pairs of indices, including repeated values at different indices.",
    ["values has at most 100 entries, each in [-1000, 1000]"], '''
def inversion_count(values: list[int]) -> int:
    count: int = 0
    first: int = 0
    while first < len(values):
        second: int = first + 1
        while second < len(values):
            if nth(values, first) > nth(values, second):
                count = count + 1
            second = second + 1
        first = first + 1
    return count
''', [[values] for values in SEQ_CASES + [[2,1,1],[3,3,2,2,1,1],list(range(100,0,-1))]],
    lambda values: sum(a>b for a,b in itertools.combinations(values,2)))

add("unique_sorted", "Remove adjacent duplicates from sorted values",
    "Return each distinct input value exactly once, in nondecreasing order. The input is already sorted. Empty input returns an empty list.",
    ["values has at most 100 entries, each in [-1000, 1000], in nondecreasing order"], '''
def unique_sorted(values: list[int]) -> list[int]:
    output: list[int] = []
    index: int = 0
    while index < len(values):
        value: int = nth(values, index)
        if index == 0:
            output.append(value)
        else:
            if value != nth(values, index - 1):
                output.append(value)
        index = index + 1
    return output
''', [[sorted(values)] for values in SEQ_CASES + [[-1000,-1000,0,1000,1000],[1]*100]],
    lambda values: sorted(set(values)), difficulty="easy")

add("pair_sum_count", "Count index pairs with a target sum",
    "Return the number of pairs of different indices i<j whose values sum to target. Count index pairs rather than distinct value combinations: four zeroes with target zero yield six pairs.",
    ["values has at most 100 entries, each in [-1000, 1000]", "-2000 <= target <= 2000"], '''
def pair_sum_count(values: list[int], target: int) -> int:
    count: int = 0
    first: int = 0
    while first < len(values):
        second: int = first + 1
        while second < len(values):
            if nth(values, first) + nth(values, second) == target:
                count = count + 1
            second = second + 1
        first = first + 1
    return count
''', [[[],0],[[0],0],[[0,0],0],[[0,0,0,0],0],[[1,2,3,4],5],[[1,1,1,2,2],3],[[-2,-1,0,1,2],0],[[5,-5,5,-5],0],[[1,2,3],10],[[1000,1000],2000],[[1]*100,2],[list(range(20)),19],[[2,2,2],4]],
    lambda values,target: sum(a+b==target for a,b in itertools.combinations(values,2)))

add("polynomial_value", "Evaluate coefficients in ascending power order",
    "Return coefficients[0] + coefficients[1]*x + coefficients[2]*x^2 + ... . Coefficients are ordered from the constant term upward. An empty coefficient list denotes zero.",
    ["coefficients has at most 12 entries, each in [-100, 100]", "-10 <= x <= 10"], '''
def polynomial_value(coefficients: list[int], x: int) -> int:
    answer: int = 0
    index: int = len(coefficients)
    while index > 0:
        index = index - 1
        answer = answer * x + nth(coefficients, index)
    return answer
''', [[[],3],[[5],0],[[5],-10],[[1,2,3],0],[[1,2,3],1],[[1,2,3],2],[[1,2,3],-2],[[0,0,1],3],[[1,-1,1,-1],-1],[[100]*12,10],[[-100]*12,-10],[[0]*12,10],[[3,0,-4,0,5],2]],
    lambda coefficients,x: sum(coefficient*x**power for power,coefficient in enumerate(coefficients)))


def mode_oracle(values: list[int]) -> dict[str,int]:
    counts = Counter(values)
    if not counts:
        return {"value":0,"count":0}
    value = min(counts,key=lambda item:(-counts[item],item))
    return {"value":value,"count":counts[value]}


add("histogram_mode", "Most frequent value with a numeric tie break",
    "Return Mode(value, count) for the most frequent input value. Break frequency ties by choosing the numerically smallest value. For empty input return value=0 and count=0.",
    ["values has at most 100 entries, each in [-1000, 1000]"], '''
def occurrence_count(values: list[int], target: int) -> int:
    count: int = 0
    index: int = 0
    while index < len(values):
        if nth(values, index) == target:
            count = count + 1
        index = index + 1
    return count

def histogram_mode(values: list[int]) -> Mode:
    best_value: int = 0
    best_count: int = 0
    index: int = 0
    while index < len(values):
        value: int = nth(values, index)
        count: int = occurrence_count(values, value)
        replace: bool = count > best_count
        if count == best_count:
            if value < best_value:
                replace = True
        if replace:
            best_value = value
            best_count = count
        index = index + 1
    return Mode(best_value, best_count)
''', [[values] for values in SEQ_CASES + [[3,3,1,1],[-2,-2,-3,-3],[5,1,5,1,5],[1000,-1000],[7]*100]],
    mode_oracle, record=("Mode",("value","count")))

add("matrix_trace", "Trace of a row-major square matrix",
    "values is a row-major flattened square matrix with side rows. Return the sum of the main diagonal, at positions (0,0), (1,1), and so on. A zero-by-zero matrix has trace zero. Off-diagonal entries must not affect the answer.",
    ["0 <= rows <= 8", "len(values) equals rows*rows; every value is in [-1000,1000]"], '''
def matrix_trace(values: list[int], rows: int) -> int:
    total: int = 0
    row: int = 0
    while row < rows:
        total = total + nth(values, row * rows + row)
        row = row + 1
    return total
''', [[[],0],[[0],1],[[7],1],[[-7],1],[[1,2,3,4],2],[[0,9,9,0],2],[[-1,8,9,-2],2],[[1,2,3,4,5,6,7,8,9],3],[[0,0,0,0,1,0,0,0,0],3],[list(range(16)),4],[[1000 if i%9==0 else -1000 for i in range(64)],8],[[(-1)**i*i for i in range(25)],5],[[0]*49,7]],
    lambda values,rows: sum(row[index] for index,row in enumerate([values[i:i+rows] for i in range(0,len(values),rows)])) if rows else 0, difficulty="easy")


# Hand-calculated anchors guard the oracle itself, independently of baselines.
ANCHORS: dict[str, tuple[list[object], object]] = {
    "prime_status": ([49], False),
    "divisor_sum": ([36], 91),
    "totient": ([36], 12),
    "modular_power": ([2, 10, 1000], 24),
    "digit_checksum": ([1234], 2),
    "integer_sqrt": ([999999999999], 999999),
    "collatz_steps": ([3, 7], 7),
    "fibonacci": ([10], 55),
    "binomial": ([10, 3], 120),
    "coin_change": ([[1, 3, 4], 6], 2),
    "staircase_blocked": ([4, [2]], 1),
    "knapsack_value": ([[6, 3, 4, 2], [30, 14, 16, 9], 10], 46),
    "lis_length": ([[3, 1, 2, 1, 4]], 3),
    "max_subarray": ([[-5, -2]], -2),
    "max_product_pair": ([[-10, -9, 1, 2]], 90),
    "prefix_balances": ([[2, -5, 4], 7], [7, 9, 4, 8]),
    "equilibrium_index": ([[1, 3, 5, 2, 2]], 2),
    "window_peak": ([[-5, -2, -7], 2], -7),
    "longest_positive_run": ([[1, 2, 0, 3, 4, 5]], 3),
    "rotate_left": ([[1, 2, 3], 4], [2, 3, 1]),
    "trapped_water": ([[3, 0, 2, 0, 4]], 7),
    "stable_partition": ([[-1, 3, -2, 0, 2, -3]], [-1, -2, -3, 3, 0, 2]),
    "sorted_insert": ([[1, 2, 2, 3], 2], [1, 2, 2, 2, 3]),
    "search_range": ([[1, 2, 2, 2, 3], 2], {"first": 1, "last": 3}),
    "inversion_count": ([[2, 1, 1]], 2),
    "unique_sorted": ([[-1, -1, 0, 2, 2]], [-1, 0, 2]),
    "pair_sum_count": ([[0, 0, 0, 0], 0], 6),
    "polynomial_value": ([[1, 2, 3], -2], 9),
    "histogram_mode": ([[-2, -2, -3, -3]], {"value": -3, "count": 2}),
    "matrix_trace": ([[1, 2, 3, 4, 5, 6, 7, 8, 9], 3], 15),
}


if __name__ == "__main__":
    emit_all()
