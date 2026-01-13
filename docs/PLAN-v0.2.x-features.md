# v0.2.x Feature Plan: Progress Tracking & Planning

> **Target**: v0.2.2 - v0.2.x releases before v0.3.0
>
> **Prerequisite**: v0.2.1 ships Lua-based captures/macros (current branch)

## Overview

Add productivity features that leverage the existing project/task infrastructure:

1. **Progress Tracking** - Visualize task completion and project health
2. **Monthly Reporting** - Time-based aggregates and summaries
3. **Daily Planning Helpers** - Streamline daily workflow setup

## Current State

### What Exists

| Feature | Status | Location |
|---------|--------|----------|
| `mdv project list` | Working | Shows open/done/total counts |
| `mdv project status <ID>` | Working | Task breakdown by status |
| `mdv task list` | Working | Filter by project, status |
| `mdv task done` | Working | Mark complete with timestamp |
| Daily note logging | Working | Auto-logs created tasks/projects |
| `mdv stale` | Working | Find old unreferenced notes |

### Gaps

| Gap | Impact |
|-----|--------|
| No progress percentage | Can't quickly see "how done" a project is |
| No time-based views | Can't see "what I did this week/month" |
| No daily planning workflow | Manual setup each morning |
| No velocity/trend data | No insight into productivity patterns |

---

## Feature 1: Progress Tracking

### 1.1 Project Progress Command

**Command**: `mdv project progress [PROJECT_ID]`

Show progress metrics for one or all projects:

```
$ mdv project progress MCP

Project: My Cool Project [MCP]

Progress: ████████████░░░░░░░░ 60% (3/5 tasks done)

By Status:
  ✓ Done:        3
  → In Progress: 1
  ○ Todo:        1
  ⊘ Blocked:     0

Recent Activity (7 days):
  - MCP-005 completed (2 days ago)
  - MCP-004 completed (5 days ago)

Velocity: 1.4 tasks/week (last 4 weeks)
```

**All projects view**:
```
$ mdv project progress

╭─────┬───────────────────┬──────────┬──────────────────────╮
│ ID  │ Title             │ Progress │ Bar                  │
├─────┼───────────────────┼──────────┼──────────────────────┤
│ MCP │ My Cool Project   │ 60%      │ ████████████░░░░░░░░ │
│ HAU │ Home Automation   │ 20%      │ ████░░░░░░░░░░░░░░░░ │
│ WER │ Website Redesign  │ 0%       │ ░░░░░░░░░░░░░░░░░░░░ │
╰─────┴───────────────────┴──────────┴──────────────────────╯
```

**Flags**:
- `--json` - JSON output for scripting
- `--include-archived` - Include archived projects

### 1.2 Task Velocity Tracking

Track task completion over time. Requires storing completion timestamps (already in `completed_at`).

**Data model**: Query index for tasks with `completed_at` in date range.

```sql
SELECT date(json_extract(frontmatter, '$.completed_at')) as day, COUNT(*) as completed
FROM notes
WHERE json_extract(frontmatter, '$.type') = 'task'
  AND json_extract(frontmatter, '$.completed_at') IS NOT NULL
  AND json_extract(frontmatter, '$.completed_at') >= date('now', '-30 days')
GROUP BY day
ORDER BY day
```

### 1.3 Implementation Notes

**Files to modify**:
- `crates/cli/src/cmd/project.rs` - Add `progress` subcommand
- `crates/core/src/index/queries.rs` - Add velocity queries

**Estimated scope**: Small (~200-300 lines)

---

## Feature 2: Monthly Reporting

### 2.1 Report Command

**Command**: `mdv report [--month YYYY-MM] [--week YYYY-WXX]`

Generate a summary report for a time period:

```
$ mdv report --month 2025-01

═══════════════════════════════════════════════════════════════
                    Monthly Report: January 2025
═══════════════════════════════════════════════════════════════

SUMMARY
  Tasks Completed:    23
  Tasks Created:      31
  Projects Started:   2
  Daily Notes:        22/31 days

TASKS BY PROJECT
╭─────┬───────────────────┬──────────┬─────────╮
│ ID  │ Project           │ Created  │ Done    │
├─────┼───────────────────┼──────────┼─────────┤
│ MCP │ My Cool Project   │ 8        │ 6       │
│ HAU │ Home Automation   │ 12       │ 9       │
│ INB │ Inbox             │ 11       │ 8       │
╰─────┴───────────────────┴──────────┴─────────╯

ACTIVITY HEATMAP (tasks completed)
    Mon Tue Wed Thu Fri Sat Sun
W01  2   3   1   4   2   0   1
W02  1   2   3   2   1   0   0
W03  0   1   2   1   3   1   0
W04  2   1   0   2   1   0   1

TOP COMPLETED TASKS
  1. MCP-012: Implement search feature
  2. HAU-008: Install smart thermostat
  3. MCP-015: Write API documentation
  ...
```

### 2.2 Report to Markdown

**Command**: `mdv report --month 2025-01 --output reports/2025-01.md`

Generate the report as a markdown file that can be stored in the vault:

```markdown
---
type: report
period: 2025-01
generated: 2025-02-01T09:00:00
---

# Monthly Report: January 2025

## Summary

| Metric | Value |
|--------|-------|
| Tasks Completed | 23 |
| Tasks Created | 31 |
| Projects Started | 2 |
| Daily Notes | 22/31 days |

## Tasks by Project

...
```

### 2.3 Weekly Report

Same format but for a week:

```
$ mdv report --week 2025-W03
```

### 2.4 Implementation Notes

**Files to create**:
- `crates/cli/src/cmd/report.rs` - Report command
- `crates/core/src/reporting/mod.rs` - Report generation logic
- `crates/core/src/reporting/aggregates.rs` - Time-based queries

**Data requirements**:
- Task `created` dates (already tracked)
- Task `completed_at` timestamps (already tracked)
- Daily note dates (already tracked)

**Estimated scope**: Medium (~400-500 lines)

---

## Feature 3: Daily Planning Helpers

### 3.1 Today Command

**Command**: `mdv today`

Quick view of today's context:

```
$ mdv today

═══════════════════════════════════════════════════════════════
                    Monday, January 13, 2025
═══════════════════════════════════════════════════════════════

DAILY NOTE: Journal/Daily/2025-01-13.md [exists]

OPEN TASKS (12 total)
  In Progress (2):
    → MCP-015: Write API documentation
    → HAU-009: Configure MQTT broker

  Due Today (1):
    ! MCP-016: Submit proposal [due: 2025-01-13]

  Blocked (1):
    ⊘ HAU-010: Waiting for hardware delivery

YESTERDAY'S COMPLETIONS (3):
  ✓ MCP-014: Review pull request
  ✓ INB-023: Reply to email
  ✓ HAU-008: Test sensor readings

SUGGESTED FOCUS:
  Based on due dates and in-progress work:
  1. MCP-016: Submit proposal (DUE TODAY)
  2. MCP-015: Write API documentation (in progress)
  3. HAU-009: Configure MQTT broker (in progress)
```

### 3.2 Plan Command

**Command**: `mdv plan`

Interactive daily planning workflow:

```
$ mdv plan

Daily Planning for Monday, January 13, 2025

? Create daily note? (Y/n) y
  ✓ Created Journal/Daily/2025-01-13.md

? Review yesterday's incomplete tasks? (Y/n) y
  These tasks were in-progress yesterday:
    → MCP-015: Write API documentation
    → HAU-009: Configure MQTT broker

  ? Continue working on these today? (Y/n) y
  ✓ Added to today's focus

? Select additional tasks to focus on today:
  ❯ [ ] MCP-016: Submit proposal [due: today!]
    [ ] MCP-017: Update README
    [ ] INB-024: Schedule meeting
    [x] HAU-010: Waiting for hardware (blocked)

? Any quick tasks to add to inbox?
  > Buy coffee filters
  ✓ Created INB-025: Buy coffee filters

Today's Plan:
  1. MCP-016: Submit proposal
  2. MCP-015: Write API documentation
  3. HAU-009: Configure MQTT broker
  4. INB-025: Buy coffee filters

✓ Plan saved to daily note
```

### 3.3 End of Day Review

**Command**: `mdv review`

End of day wrap-up:

```
$ mdv review

End of Day Review: Monday, January 13, 2025

TODAY'S ACTIVITY:
  ✓ Completed: 3 tasks
  → Still in progress: 2 tasks
  + Created: 1 task

COMPLETED TODAY:
  ✓ MCP-016: Submit proposal
  ✓ MCP-015: Write API documentation
  ✓ INB-025: Buy coffee filters

STILL IN PROGRESS:
  → HAU-009: Configure MQTT broker

? Add notes to daily note? (Y/n) y
  > Good progress on documentation. MQTT config taking longer than expected.
  ✓ Added to Journal/Daily/2025-01-13.md

? Carry over in-progress tasks to tomorrow? (Y/n) y
  ✓ Tasks will appear in tomorrow's planning
```

### 3.4 Daily Note Template Enhancement

Enhance daily note creation to include planning sections:

```markdown
---
type: daily
date: 2025-01-13
---

# Monday, January 13, 2025

## Plan
<!-- Tasks selected during `mdv plan` -->
- [ ] [[MCP-016]]: Submit proposal
- [ ] [[MCP-015]]: Write API documentation
- [ ] [[HAU-009]]: Configure MQTT broker

## Log
<!-- Auto-populated by task creation/completion -->

## Notes
<!-- Manual notes added during `mdv review` -->

## Reflection
<!-- End of day thoughts -->
```

### 3.5 Implementation Notes

**Files to create**:
- `crates/cli/src/cmd/today.rs` - Today summary
- `crates/cli/src/cmd/plan.rs` - Interactive planning
- `crates/cli/src/cmd/review.rs` - End of day review

**Integration points**:
- Daily note creation/modification
- Task queries by date/status
- Captures for quick task creation

**Estimated scope**: Medium-Large (~600-800 lines)

---

## Implementation Phases

### Phase A: Progress Tracking (v0.2.2)

1. Add `mdv project progress` command
2. Add progress bar visualization
3. Add velocity calculation
4. Update `mdv project list` to show progress %

### Phase B: Monthly Reporting (v0.2.3)

1. Add `mdv report --month` command
2. Add `mdv report --week` command
3. Add `--output` flag for markdown generation
4. Add activity heatmap visualization

### Phase C: Daily Planning (v0.2.4)

1. Add `mdv today` command
2. Add `mdv plan` interactive workflow
3. Add `mdv review` end-of-day workflow
4. Enhance daily note template

---

## Open Questions

1. **Storage for daily plans**: Store in daily note frontmatter or separate file?
   - Proposal: Store in daily note frontmatter as `plan: [task-ids]`

2. **Velocity calculation window**: How many weeks to average?
   - Proposal: 4 weeks rolling average

3. **Report file naming**: What convention?
   - Proposal: `reports/YYYY-MM.md` for monthly, `reports/YYYY-WXX.md` for weekly

4. **Due date support**: Tasks don't currently have due dates by default
   - Proposal: Add optional `due` field to task schema, filter in queries

---

## Success Criteria

| Feature | Criteria |
|---------|----------|
| Progress tracking | Can see % complete for any project |
| Monthly reporting | Can generate report for any past month |
| Daily planning | Can set up day in < 2 minutes with `mdv plan` |
| Integration | All features use existing index, no new storage |

---

## Dependencies

- v0.2.1 must be released first (Lua captures/macros)
- Index must have `completed_at` timestamps (already present)
- Daily notes must exist (already supported)

---

## Timeline

| Version | Features |
|---------|----------|
| v0.2.1 | Lua captures/macros (current branch) |
| v0.2.2 | Progress tracking |
| v0.2.3 | Monthly/weekly reporting |
| v0.2.4 | Daily planning helpers |
| v0.3.0 | Remove YAML, new note types |
