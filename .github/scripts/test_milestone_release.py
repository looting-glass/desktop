import json
import os
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from milestone_release import AmbiguousReleasePr, Plan, decide, dry_run, main

RELEASE_PR = {"number": 69, "title": "chore(main): release 0.10.0"}


def milestone(open_issues: int, state: str = "open") -> dict:
    return {"title": "0.10", "state": state, "open_issues": open_issues}


class DecideTest(unittest.TestCase):
    def test_last_issue_closed_releases_and_closes_the_milestone(self):
        plan = decide(milestone(0), [RELEASE_PR])
        self.assertEqual(plan.release_pr, 69)
        self.assertTrue(plan.close_milestone)
        self.assertIn("merging #69 (chore(main): release 0.10.0)", plan.summary)

    def test_open_issues_left_waits(self):
        self.assertEqual(
            decide(milestone(1), [RELEASE_PR]),
            Plan(
                release_pr=None,
                close_milestone=False,
                summary="Milestone 0.10 still has 1 open issue.",
            ),
        )
        self.assertIn("2 open issues", decide(milestone(2), [RELEASE_PR]).summary)

    def test_nothing_to_release_only_closes_the_milestone(self):
        plan = decide(milestone(0), [])
        self.assertIsNone(plan.release_pr)
        self.assertTrue(plan.close_milestone)
        self.assertIn("nothing to release", plan.summary)

    def test_closed_milestone_is_left_alone(self):
        plan = decide(milestone(0, state="closed"), [RELEASE_PR])
        self.assertIsNone(plan.release_pr)
        self.assertFalse(plan.close_milestone)
        self.assertIn("already closed", plan.summary)

    def test_several_release_prs_are_not_guessed_between(self):
        other = {"number": 70, "title": "chore(main): release 0.10.1"}
        with self.assertRaisesRegex(AmbiguousReleasePr, "#69, #70"):
            decide(milestone(0), [RELEASE_PR, other])

    def test_dry_run_only_logs_the_plan(self):
        plan = dry_run(decide(milestone(0), [RELEASE_PR]))
        self.assertIsNone(plan.release_pr)
        self.assertFalse(plan.close_milestone)
        self.assertTrue(plan.summary.startswith("Dry run. Milestone 0.10 is complete: merging #69"))


class MainTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.dir = Path(directory.name)
        self.outputs = self.dir / "outputs"
        self.outputs.touch()
        os.environ["GITHUB_OUTPUT"] = str(self.outputs)
        self.addCleanup(os.environ.pop, "GITHUB_OUTPUT")
        self.addCleanup(os.environ.pop, "DRY_RUN", None)

    def run_main(self, milestone: dict, release_prs: list[dict]) -> tuple[int, str]:
        (self.dir / "milestone.json").write_text(json.dumps(milestone))
        (self.dir / "release-prs.json").write_text(json.dumps(release_prs))
        log = StringIO()
        with redirect_stdout(log):
            status = main(str(self.dir / "milestone.json"), str(self.dir / "release-prs.json"))
        return status, log.getvalue()

    def test_writes_the_plan_as_step_outputs(self):
        status, log = self.run_main(milestone(0), [RELEASE_PR])
        self.assertEqual(status, 0)
        self.assertTrue(log.startswith("::notice::Milestone 0.10 is complete"))
        self.assertEqual(self.outputs.read_text(), "release-pr=69\nclose-milestone=true\n")

    def test_open_issues_left_write_no_release(self):
        self.run_main(milestone(3), [RELEASE_PR])
        self.assertEqual(self.outputs.read_text(), "release-pr=\nclose-milestone=false\n")

    def test_dry_run_writes_no_release(self):
        os.environ["DRY_RUN"] = "true"
        status, log = self.run_main(milestone(0), [RELEASE_PR])
        self.assertEqual(status, 0)
        self.assertTrue(log.startswith("::notice::Dry run. Milestone 0.10 is complete"))
        self.assertEqual(self.outputs.read_text(), "release-pr=\nclose-milestone=false\n")

    def test_ambiguous_release_prs_fail_the_run(self):
        status, log = self.run_main(milestone(0), [RELEASE_PR, RELEASE_PR])
        self.assertEqual(status, 1)
        self.assertTrue(log.startswith("::error::"))
        self.assertEqual(self.outputs.read_text(), "")


if __name__ == "__main__":
    unittest.main()
