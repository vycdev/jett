import json
import unittest

from tools import jett_bench as bench


class ExpandedCatalogTests(unittest.TestCase):
    def test_expansion_keeps_original_ten_and_ninety_distinct_new_tasks(self) -> None:
        tasks = bench.load_tasks()
        original = {"triangle_kind", "signed_gcd", "bounded_weighted_sum", "order_lifecycle",
                    "recursive_expression", "account_state_evolution", "first_duplicate",
                    "merge_sorted_intervals", "inventory_batch", "score_lines"}
        ids = {task["id"] for _, task in tasks}
        self.assertGreaterEqual(len(tasks), 100)
        self.assertTrue(original.issubset(ids), original - ids)
        for prefix in ("num_", "text_", "data_"):
            self.assertGreaterEqual(sum(task_id.startswith(prefix) for task_id in ids), 30)
        statements = [task["statement"] for _, task in tasks]
        self.assertEqual(len(statements), len(set(statements)))

    def test_new_fixtures_have_distinct_inputs_and_nonconstant_answers(self) -> None:
        for directory, task in bench.load_tasks():
            if not task["id"].startswith(("num_", "text_", "data_")):
                continue
            with self.subTest(task=task["id"]):
                cases = bench.read_json(directory / "cases.json")["cases"]
                self.assertGreaterEqual(len(cases), 10)
                identities: set[str] = set()
                expected: set[str] = set()
                for case in cases:
                    inputs = case.get("inputs", {key: case[key] for key in ("rows", "limit") if key in case})
                    self.assertTrue("inputs" in case or "rows" in case)
                    identity = json.dumps(inputs, sort_keys=True)
                    self.assertNotIn(identity, identities, "duplicated public fixture input")
                    identities.add(identity)
                    expected.add(json.dumps(case["expected"], sort_keys=True))
                self.assertGreaterEqual(len(expected), 2, "constant answer would pass the entire grader")
                self.assertTrue(task["category"])
                self.assertIn(task["difficulty"], ("easy", "medium", "hard"))

    def test_catalog_metadata_is_allowed_by_published_schema(self) -> None:
        schema = bench.read_json(bench.BENCHMARKS / "schemas/task.schema.json")
        allowed = set(schema["properties"])
        required = set(schema["required"])
        for _, task in bench.load_tasks():
            with self.subTest(task=task["id"]):
                self.assertTrue(set(task).issubset(allowed), set(task) - allowed)
                self.assertTrue(required.issubset(task))
                self.assertEqual(set(task["adapters"]), set(bench.LANGUAGE_SKILLS))
                for adapter in task["adapters"].values():
                    self.assertGreaterEqual(len(adapter["commands"]), 2)


if __name__ == "__main__":
    unittest.main()
