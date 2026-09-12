"""Materialize the structured-data benchmark family from typed, audited source specs.

This is evaluator tooling, never language-skill or candidate prompt content.
The small AST emitter shares only syntax between five adapters; each spec owns
its domain records, contract, algorithm, and language-neutral deterministic cases.
"""
from __future__ import annotations

import ast
import copy
import json
import re
import subprocess
from pathlib import Path

from bench_data_specs import SPECS
from bench_data_oracles import expected as oracle_expected
from jett_bench import jett_executable

ROOT = Path(__file__).resolve().parents[1]
LANGUAGES = {"jett": "jett", "python": "py", "typescript": "ts", "go": "go", "rust": "rs"}


def pascal(text: str) -> str:
    return "".join(piece.capitalize() for piece in text.split("_"))


class Emitter:
    def __init__(self, language: str, spec: dict[str, object]):
        self.language = language
        self.spec = spec
        self.input_fields = str(spec["fields"]).split()
        self.output_fields = str(spec["outputs"]).split()
        self.local_types: dict[str, str] = {}
        self.function_types: dict[str, str] = {}
        self.parameter_types: dict[str, list[str]] = {}
        self.temporary_index = 0

    def name(self, value: str) -> str:
        return pascal(value) if self.language == "go" and value in self.input_fields + self.output_fields else value

    def type(self, value: str, parameter: bool = False) -> str:
        lang = self.language
        if value == "int":
            return {"jett": "int64", "python": "int", "typescript": "bigint", "go": "int64", "rust": "i64"}[lang]
        if value == "bool":
            return "boolean" if lang == "typescript" else "bool"
        if value == "list[Entry]":
            return {"jett": "list[Entry]", "python": "list[Entry]", "typescript": "readonly Entry[]", "go": "[]Entry", "rust": "&[Entry]" if parameter else "Vec<Entry>"}[lang]
        if value == "dict[int, int]":
            return {"jett": "map[int64, int64]", "python": value, "typescript": "Map<bigint, bigint>", "go": "map[int64]int64", "rust": "&std::collections::BTreeMap<i64, i64>" if parameter else "std::collections::BTreeMap<i64, i64>"}[lang]
        return value

    def expr(self, node: ast.expr) -> str:
        lang = self.language
        if isinstance(node, ast.Constant):
            if isinstance(node.value, bool):
                return str(node.value) if lang == "python" else str(node.value).lower()
            return str(node.value) + ("n" if lang == "typescript" else "")
        if isinstance(node, ast.Name):
            return node.id
        if isinstance(node, ast.Attribute):
            return self.expr(node.value) + "." + self.name(node.attr)
        if isinstance(node, ast.UnaryOp):
            operator = "-" if isinstance(node.op, ast.USub) else ("not " if lang in {"python", "jett"} else "!")
            return "(" + operator + self.expr(node.operand) + ")"
        if isinstance(node, ast.BinOp):
            operator = {ast.Add: "+", ast.Sub: "-", ast.Mult: "*", ast.Mod: "modulo" if lang == "jett" else "%", ast.FloorDiv: "//" if lang == "python" else "/"}[type(node.op)]
            return "(" + self.expr(node.left) + " " + operator + " " + self.expr(node.right) + ")"
        if isinstance(node, ast.BoolOp):
            operator = (" and " if isinstance(node.op, ast.And) else " or ") if lang in {"python", "jett"} else (" && " if isinstance(node.op, ast.And) else " || ")
            return "(" + operator.join(self.expr(value) for value in node.values) + ")"
        if isinstance(node, ast.Compare):
            assert len(node.ops) == 1
            operator = {ast.Eq: "==", ast.NotEq: "!=", ast.Lt: "<", ast.LtE: "<=", ast.Gt: ">", ast.GtE: ">="}[type(node.ops[0])]
            return "(" + self.expr(node.left) + " " + operator + " " + self.expr(node.comparators[0]) + ")"
        if isinstance(node, ast.Dict):
            return {"jett": "map.new[int64, int64]()", "python": "{}", "typescript": "new Map<bigint, bigint>()", "go": "make(map[int64]int64)", "rust": "std::collections::BTreeMap::new()"}[lang]
        if isinstance(node, ast.Call):
            assert isinstance(node.func, ast.Name)
            name = node.func.id
            args = [self.expr(arg) for arg in node.args]
            if name in {"Entry", "Report"}:
                fields = self.input_fields if name == "Entry" else self.output_fields
                pairs = [self.name(field) + ": " + value for field, value in zip(fields, args, strict=True)]
                if lang == "python":
                    return name + "(" + ", ".join(args) + ")"
                if lang == "jett":
                    return name + "(" + ", ".join(pairs) + ")"
                if lang == "typescript":
                    return "{ " + ", ".join(pairs) + " }"
                return name + " { " + ", ".join(pairs) + " }"
            if name == "read":
                table, key, fallback = args
                return {"jett": f"map.get_or[int64, int64](clone {table}, {key}, {fallback})", "python": f"{table}.get({key}, {fallback})", "typescript": f"({table}.get({key}) ?? {fallback})", "go": f"read({table}, {key}, {fallback})", "rust": f"{table}.get(&{key}).copied().unwrap_or({fallback})"}[lang]
            if name == "put":
                table, key, value = args
                return {"jett": f"map.set[int64, int64]({table}, {key}, {value})", "python": f"put({table}, {key}, {value})", "typescript": f"{table}.set({key}, {value})", "go": f"put({table}, {key}, {value})", "rust": f"put({table}, {key}, {value})"}[lang]
            types = self.parameter_types[name]
            if lang == "jett":
                args = ["view " + arg if typ.startswith(("list[", "dict[")) else arg for arg, typ in zip(args, types, strict=True)]
            if lang == "rust":
                args = ["&" + arg if typ.startswith("dict[") else arg for arg, typ in zip(args, types, strict=True)]
            return name + "(" + ", ".join(args) + ")"
        raise ValueError(ast.dump(node))

    def statement(self, node: ast.stmt, indent: int) -> list[str]:
        lang = self.language
        pad = "    " * indent
        tail = "" if lang in {"python", "jett"} else ";"
        if isinstance(node, ast.AnnAssign):
            assert isinstance(node.target, ast.Name) and node.value
            name, typ = node.target.id, ast.unparse(node.annotation)
            self.local_types[name] = typ
            value = self.expr(node.value)
            forms = {"python": f"{name}: {typ} = {value}", "jett": f"mutable {self.type(typ)} {name} = {value}", "typescript": f"let {name}: {self.type(typ)} = {value};", "go": f"var {name} {self.type(typ)} = {value}", "rust": f"let mut {name}: {self.type(typ)} = {value};"}
            return [pad + forms[lang]]
        if isinstance(node, ast.Assign):
            if isinstance(node.value, ast.Call) and isinstance(node.value.func, ast.Name) and node.value.func.id == "put":
                temporary = f"updated_value_{self.temporary_index}"
                self.temporary_index += 1
                declaration = ast.AnnAssign(target=ast.Name(id=temporary), annotation=ast.Name(id="int"), value=node.value.args[2], simple=1)
                rewritten = copy.deepcopy(node)
                rewritten.value.args[2] = ast.Name(id=temporary)
                return self.statement(declaration, indent) + [pad + self.expr(rewritten.targets[0]) + " = " + self.expr(rewritten.value) + tail]
            return [pad + self.expr(node.targets[0]) + " = " + self.expr(node.value) + tail]
        if isinstance(node, ast.Return):
            assert node.value
            return [pad + "return " + self.expr(node.value) + tail]
        if isinstance(node, ast.If):
            condition = self.expr(node.test)
            if lang == "typescript":
                condition = "(" + condition + ")"
            head = (f"if {condition}:" if lang in {"jett", "python"} else f"if {condition} {{")
            lines = [pad + head] + self.block(node.body, indent + 1)
            if lang not in {"python", "jett"}:
                lines.append(pad + "}")
            if node.orelse:
                lines.append(pad + ("else:" if lang in {"python", "jett"} else "else {"))
                lines += self.block(node.orelse, indent + 1)
                if lang not in {"python", "jett"}:
                    lines.append(pad + "}")
            return lines
        if isinstance(node, ast.For):
            item, source = self.expr(node.target), self.expr(node.iter)
            forms = {"jett": f"for {item} in view {source}:", "python": f"for {item} in {source}:", "typescript": f"for (const {item} of {source}) {{", "go": f"for _, {item} := range {source} {{", "rust": f"for {item} in {source} {{"}
            lines = [pad + forms[lang]] + self.block(node.body, indent + 1)
            if lang not in {"python", "jett"}:
                lines.append(pad + "}")
            return lines
        if isinstance(node, ast.While):
            condition = self.expr(node.test)
            forms = {"jett": f"while {condition}:", "python": f"while {condition}:", "typescript": f"while ({condition}) {{", "go": f"for {condition} {{", "rust": f"while {condition} {{"}
            lines = [pad + forms[lang]] + self.block(node.body, indent + 1)
            if lang not in {"python", "jett"}:
                lines.append(pad + "}")
            return lines
        raise ValueError(ast.dump(node))

    def block(self, nodes: list[ast.stmt], indent: int) -> list[str]:
        return [line for node in nodes for line in self.statement(node, indent)]

    def declarations(self) -> str:
        lang = self.language
        lines = []
        for name, fields in [("Entry", self.input_fields), ("Report", self.output_fields)]:
            if lang == "python":
                lines += ["@dataclass(frozen=True)", f"class {name}:"] + [f"    {field}: int" for field in fields]
            elif lang == "jett":
                lines += [f"struct {name}:"] + [f"    {field}: int64" for field in fields]
            elif lang == "typescript":
                lines += [f"export interface {name} {{"] + [f"    readonly {field}: bigint;" for field in fields] + ["}"]
            elif lang == "go":
                lines += [f"type {name} struct {{"] + [f"    {self.name(field)} int64" for field in fields] + ["}"]
            else:
                lines += ["#[derive(Debug, PartialEq, Eq)]", f"pub struct {name} {{"] + [f"    pub {field}: i64," for field in fields] + ["}"]
            lines.append("")
        return "\n".join(lines)

    def render(self) -> tuple[str, str]:
        lang = self.language
        tree = ast.parse(str(self.spec["code"]))
        functions = [node for node in tree.body if isinstance(node, ast.FunctionDef)]
        for function in functions:
            assert function.returns
            self.function_types[function.name] = ast.unparse(function.returns)
            self.parameter_types[function.name] = [ast.unparse(arg.annotation) for arg in function.args.args if arg.annotation]
        imports = {"python": "from dataclasses import dataclass\n", "jett": "namespace benchmark\n", "typescript": "", "go": "package benchmark\n", "rust": ""}[lang]
        parts = [imports, self.declarations()]
        if "put(" in str(self.spec["code"]):
            if lang == "python":
                parts.append("def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:\n    table[key] = value\n    return table\n")
            if lang == "rust":
                parts.append("fn put(mut table: std::collections::BTreeMap<i64, i64>, key: i64, value: i64) -> std::collections::BTreeMap<i64, i64> {\n    table.insert(key, value);\n    table\n}\n")
            if lang == "go":
                parts.append("func put(table map[int64]int64, key int64, value int64) map[int64]int64 {\n    table[key] = value\n    return table\n}\n")
        if lang == "go" and "read(" in str(self.spec["code"]):
            parts.append("func read(table map[int64]int64, key int64, fallback int64) int64 {\n    value, exists := table[key]\n    if exists { return value }\n    return fallback\n}\n")
        public_head = ""
        for function in functions:
            self.local_types = dict(zip([arg.arg for arg in function.args.args], self.parameter_types[function.name], strict=True))
            params = []
            for arg, typ in zip(function.args.args, self.parameter_types[function.name], strict=True):
                if lang == "python":
                    params.append(arg.arg + ": " + typ)
                elif lang == "jett":
                    params.append(("view " if typ.startswith(("list[", "dict[")) else "") + arg.arg + ": " + self.type(typ))
                elif lang == "go":
                    params.append(arg.arg + " " + self.type(typ, True))
                else:
                    params.append(arg.arg + ": " + self.type(typ, True))
            ret = self.type(self.function_types[function.name])
            args = ", ".join(params)
            head = {"python": f"def {function.name}({args}) -> {ret}", "jett": f"function {function.name}({args}) returns {ret}", "typescript": f"export function {function.name}({args}): {ret}", "go": f"func {function.name}({args}) {ret}", "rust": f"pub fn {function.name}({args}) -> {ret}"}[lang]
            if function.name == "solve":
                public_head = head
            lines = [head + (":" if lang in {"python", "jett"} else " {")] + self.block(function.body, 1)
            if lang not in {"python", "jett"}:
                lines.append("}")
            parts.append("\n".join(lines) + "\n")
        source = "\n".join(parts)
        # Go's automatic semicolon insertion requires else on the closing-brace line.
        if lang == "go":
            source = re.sub(r"}\n(\s*)else", r"} else", source)
        return source, imports + "\n" + self.declarations() + "\n" + public_head

    def literal(self, values: list[int], record: str) -> str:
        return self.expr(ast.Call(func=ast.Name(id=record), args=[ast.Constant(value=value) if value >= 0 else ast.UnaryOp(op=ast.USub(), operand=ast.Constant(value=-value)) for value in values], keywords=[]))

    def hidden(self, cases: list[dict[str, object]]) -> str:
        lang = self.language
        lines = {"jett": [], "python": ["from solution import Entry, Report, solve", ""], "typescript": ['import { Entry, Report, solve } from "./solution.js";', ""], "go": ['package benchmark', 'import "testing"', '', 'func TestFixtures(t *testing.T) {'], "rust": ['include!("solution.rs");', "", "#[test]", "fn fixtures() {"]}[lang]
        for index, case in enumerate(cases):
            rows = case["rows"]
            assert isinstance(rows, list)
            entries = [self.literal(row, "Entry") for row in rows]
            limit = self.expr(ast.Constant(value=case.get("limit", 0)))
            extra_argument = ", " + limit if self.spec["uses_limit"] else ""
            expected = case["expected"]
            assert isinstance(expected, list)
            if lang == "jett":
                lines += [f"verify fixture_{index:03d}:", "    mutable list[Entry] rows = list.new[Entry]()"]
                lines += [f"    rows = list.append[Entry](rows, {entry})" for entry in entries]
                lines += [f"    Report actual = solve(view rows{extra_argument})"]
                lines += [f"    assert actual.{field} == {value}" for field, value in zip(self.output_fields, expected, strict=True)]
                lines.append("")
            elif lang == "python":
                lines.append(f"assert solve([{', '.join(entries)}]{extra_argument}) == {self.literal(expected, 'Report')}, 'fixture {index}'")
            elif lang == "typescript":
                lines += ["{", f"    const rows: readonly Entry[] = [{', '.join(entries)}];", f"    const actual: Report = solve(rows{extra_argument});"]
                mismatch = " || ".join(f"actual.{field} !== {self.expr(ast.Constant(value=value))}" for field, value in zip(self.output_fields, expected, strict=True))
                lines += [f'    if ({mismatch}) throw new Error("fixture {index}");', "}"]
            elif lang == "go":
                lines += ["    {", f"        actual := solve([]Entry{{{', '.join(entries)}}}{extra_argument})", f"        expected := {self.literal(expected, 'Report')}", f'        if actual != expected {{ t.Fatalf("fixture {index}: got %+v expected %+v", actual, expected) }}', "    }"]
            else:
                lines.append(f"    assert_eq!(solve(&[{', '.join(entries)}]{extra_argument}), {self.literal(expected, 'Report')}, \"fixture {index}\");")
        if lang in {"go", "rust"}:
            lines.append("}")
        return "\n".join(lines) + "\n"


def generate() -> None:
    template = json.loads((ROOT / "benchmarks/tasks/inventory_batch/task.json").read_text())
    formatting: dict[str, list[str]] = {language: [] for language in ["jett", "go", "rust"]}
    for spec in SPECS:
        directory = ROOT / "benchmarks/tasks" / str(spec["id"])
        directory.mkdir(parents=True, exist_ok=True)
        python_source, _ = Emitter("python", spec).render()
        namespace: dict[str, object] = {}
        exec(python_source, namespace)
        cases = []
        for rows, limit in spec["cases"]:
            arguments = [[namespace["Entry"](*row) for row in rows]]
            if spec["uses_limit"]:
                arguments.append(limit)
            answer = namespace["solve"](*arguments)
            actual = [getattr(answer, field) for field in str(spec["outputs"]).split()]
            expected = oracle_expected(spec["id"], [dict(zip(str(spec["fields"]).split(), row, strict=True)) for row in rows], limit)
            assert actual == expected, (spec["id"], rows, limit, actual, expected)
            case = {"rows": rows, "expected": expected, "oracle": "independent"}
            if spec["uses_limit"]:
                case["limit"] = limit
            cases.append(case)
        for rows, limit, expected in spec.get("anchors", []):
            arguments = [[namespace["Entry"](*row) for row in rows]]
            if spec["uses_limit"]:
                arguments.append(limit)
            answer = namespace["solve"](*arguments)
            actual = [getattr(answer, field) for field in str(spec["outputs"]).split()]
            assert actual == expected, (spec["id"], actual, expected)
            independently_expected = oracle_expected(spec["id"], [dict(zip(str(spec["fields"]).split(), row, strict=True)) for row in rows], limit)
            assert independently_expected == expected, (spec["id"], "anchor oracle", independently_expected, expected)
            case = {"rows": rows, "expected": expected, "hand_checked": True, "oracle": "independent"}
            if spec["uses_limit"]:
                case["limit"] = limit
            cases.append(case)
        distinct: dict[str, dict[str, object]] = {}
        for case in cases:
            inputs = {field: case[field] for field in ["rows", "limit"] if field in case}
            identity = json.dumps(inputs, sort_keys=True, separators=(",", ":"))
            if identity in distinct:
                assert distinct[identity]["expected"] == case["expected"], (spec["id"], "duplicate input disagrees", case)
                if case.get("hand_checked"):
                    distinct[identity]["hand_checked"] = True
            else:
                distinct[identity] = case
        cases = list(distinct.values())
        assert len(cases) >= 10
        bounds = "All arithmetic fits signed int64; there are at most 30 records."
        if spec["uses_limit"]:
            bounds += " limit is in [0, 30]."
        task = {"id": spec["id"], "version": "1.0.0", "title": spec["title"], "category": spec["category"], "difficulty": spec.get("difficulty", "medium"), "statement": spec["statement"], "constraints": [str(spec["domain"]), "Use the exact declared Entry and Report record fields and solve signature. Return all three Report fields; do not mutate the caller's records.", bounds + " Return fresh results; no I/O, tests, exceptions, panic, or static-check bypasses."], "adapters": {}}
        for lang, extension in LANGUAGES.items():
            emitter = Emitter(lang, spec)
            source, signature = emitter.render()
            adapter = copy.deepcopy(template["adapters"][lang])
            adapter["signature"] = signature
            adapter["forbidden_patterns"] = [policy for policy in adapter["forbidden_patterns"] if not any(word in policy["message"] for word in ["catch-all", "wildcard", "default switch"])]
            for policy in adapter["forbidden_patterns"]:
                policy["message"] = policy["message"].replace("return Rejected", "return the documented report").replace("return rejected", "return the documented report")
            task["adapters"][lang] = adapter
            (directory / adapter["baseline"]).write_text(source, encoding="utf-8", newline="\n")
            (directory / adapter["hidden"]).write_text(emitter.hidden(cases), encoding="utf-8", newline="\n")
            if "starter_code" in spec:
                starter_spec = dict(spec, code=spec["starter_code"])
                starter, _ = Emitter(lang, starter_spec).render()
                adapter["starter"] = "starter." + extension
                (directory / adapter["starter"]).write_text(starter, encoding="utf-8", newline="\n")
            if lang in formatting:
                formatting[lang].extend(str(directory / adapter[field]) for field in ["baseline", "hidden", "starter"] if field in adapter)
        (directory / "pyrightconfig.json").write_text(json.dumps({"include": ["solution.py", "hidden.py"], "pythonVersion": "3.12", "typeCheckingMode": "strict"}, indent=2) + "\n", encoding="utf-8")
        (directory / "task.json").write_text(json.dumps(task, indent=2) + "\n", encoding="utf-8")
        (directory / "cases.json").write_text(json.dumps({"fields": str(spec["fields"]).split(), "outputs": str(spec["outputs"]).split(), "cases": cases}, indent=2) + "\n", encoding="utf-8")
        print(f"{spec['id']}: {len(cases)} cases, five adapters")
    for path in formatting["jett"]:
        subprocess.run([str(jett_executable()), "format", path], check=True, capture_output=True, text=True)
    subprocess.run(["gofmt", "-w", *formatting["go"]], check=True)
    subprocess.run(["rustfmt", "--edition", "2024", *formatting["rust"]], check=True)


if __name__ == "__main__":
    generate()
