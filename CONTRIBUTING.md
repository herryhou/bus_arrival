# Contributing

## Branch Naming

Use these prefixes:

- `feat/` — New features
- `fix/` — Bug fixes
- `refactor/` — Code refactoring
- `docs/` — Documentation only
- `test/` — Test additions or changes

Examples: `feat/off-route-detection`, `fix/heading-filter`, `refactor/kalman-state`

## Commit Messages

Format: `type(scope): description`

- `feat` — New feature
- `fix` — Bug fix
- `refactor` — Refactoring
- `test` — Test changes
- `docs` — Documentation
- `chore` — Build/config changes

Scope (optional): Module name like `(kalman)`, `(map_match)`, `(pipeline)`

```
feat(pipeline): add off-route recovery
test(kalman): add property-based tests for heading filter
docs: update architecture diagram
```

## Workflow

1. Create branch from `master`: `git checkout -b feat/your-feature`
2. Make commits, squash WIPs before finalizing
3. Push and create PR
4. After review, squash merge WIP commits
5. Delete branch after merge

## Worktree vs Branch

**Use a regular branch when:**
- Working on one feature at a time
- Straightforward, sequential work
- Don't need to context-switch

**Use a worktree when:**
- Need to work on multiple branches simultaneously
- Hotfix comes up while mid-feature
- Testing or comparing different branches side-by-side
- Code review requires testing changes

```bash
# Create worktree for parallel work
git worktree add ../bus_arrival-hotfix fix/urgent-bug

# Remove when done
git worktree remove ../bus_arrival-hotfix
```

## Before PR

- `cargo test` passes
- `cargo clippy` has no warnings
- Commits are clean (no `wip`, `tmp`, etc.)
- Update `docs/bus_arrival_tech_report_v8.md` if algorithm/architecture changed
- Update other docs if needed

## Branch Cleanup

**After merge:**
```bash
git branch -d feat/finished-feature    # local
git push origin --delete feat/finished-feature  # remote
```

**Periodic cleanup** (remove obsolete/abandoned branches):
```bash
# List branches merged to master (safe to delete)
git branch --merged master

# List branches NOT merged (review before deleting)
git branch --no-merged master

# Delete obsolete local branch
git branch -D feat/abandoned-experiment

# Clean up stale remote branches
git remote prune origin
```

**What to clean:**
- Superseded by other work
- No longer relevant (requirements changed)
- Experimental dead-ends
- Older than 3 months with no activity
