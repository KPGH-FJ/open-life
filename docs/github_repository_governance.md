# GitHub Repository Governance

## Branch Model

- `main` is the only long-lived branch.
- Work happens on short-lived branches and returns through a pull request.
- Do not keep feature branches as historical archives; Git commits and tags
  provide recovery.
- Do not create extra OpenLife worktrees or sibling development checkouts.

## Pull Requests

Each PR should state:

- product or infrastructure outcome;
- files and behavior in scope;
- checks run;
- privacy, durable-write, provider, and migration risks;
- known limitations.

CI and source review are required in proportion to risk. A solo maintainer may
merge their own PR after reviewing the exact diff and passing required checks;
the repository must not fake independent approval.

## Documents

Keep current product, architecture, decisions, and one active plan in the
working tree. Superseded execution records remain in Git history.
