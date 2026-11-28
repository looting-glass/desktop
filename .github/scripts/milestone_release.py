"""Decides what the Milestone release workflow does after an issue in a milestone is closed.

The workflow fetches the milestone and the open release PRs with gh, passes them here as JSON
files and carries out the plan this writes to its step outputs:

    milestone_release.py MILESTONE_JSON RELEASE_PRS_JSON

MILESTONE_JSON is the milestone as the REST API returns it (`title`, `state`, `open_issues`);
RELEASE_PRS_JSON is `gh pr list --json number,title` for the open PRs labelled
`autorelease: pending`. With DRY_RUN=true in the environment the plan is only logged.
"""

import json
import os
import sys
from dataclasses import dataclass


@dataclass(frozen=True)
class Plan:
    release_pr: int | None
    """The release PR to merge, after which the Release workflow is started."""
    close_milestone: bool
    summary: str
    """What happens and why, for the run's log."""


class AmbiguousReleasePr(Exception):
    """More than one open PR carries the release label, so merging either could be wrong."""


def decide(milestone: dict, release_prs: list[dict]) -> Plan:
    title = milestone["title"]
    if milestone["state"] != "open":
        # A second issue closed at the same time, or one reopened and closed after a release.
        return Plan(
            release_pr=None,
            close_milestone=False,
            summary=f"Milestone {title} is already closed: nothing to do.",
        )

    open_issues = milestone["open_issues"]
    if open_issues:
        issues = "issue" if open_issues == 1 else "issues"
        return Plan(
            release_pr=None,
            close_milestone=False,
            summary=f"Milestone {title} still has {open_issues} open {issues}.",
        )

    if not release_prs:
        # Only docs:, ci: or chore: changes since the last release; the batch is still done.
        return Plan(
            release_pr=None,
            close_milestone=True,
            summary=f"Milestone {title} is complete, but there is nothing to release "
            "(no open release PR): closing the milestone.",
        )

    if len(release_prs) > 1:
        numbers = ", ".join(f"#{pr['number']}" for pr in release_prs)
        raise AmbiguousReleasePr(
            f"Milestone {title} is complete, but {numbers} are all labelled as release PRs. "
            "Merge the right one by hand and close the milestone."
        )

    pr = release_prs[0]
    return Plan(
        release_pr=pr["number"],
        close_milestone=True,
        summary=f"Milestone {title} is complete: merging #{pr['number']} ({pr['title']}), "
        "starting the Release workflow and closing the milestone.",
    )


def dry_run(plan: Plan) -> Plan:
    """The same plan, only logged: nothing is merged, started or closed."""
    return Plan(release_pr=None, close_milestone=False, summary=f"Dry run. {plan.summary}")


def main(milestone_path: str, release_prs_path: str) -> int:
    with open(milestone_path) as file:
        milestone = json.load(file)
    with open(release_prs_path) as file:
        release_prs = json.load(file)

    try:
        plan = decide(milestone, release_prs)
    except AmbiguousReleasePr as error:
        print(f"::error::{error}")
        return 1
    if os.environ.get("DRY_RUN") == "true":
        plan = dry_run(plan)

    print(f"::notice::{plan.summary}")
    with open(os.environ["GITHUB_OUTPUT"], "a") as outputs:
        outputs.write(f"release-pr={plan.release_pr or ''}\n")
        outputs.write(f"close-milestone={str(plan.close_milestone).lower()}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:]))
