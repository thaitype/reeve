# Standard — Builder Scope Bleed

## Rule

When a builder-agent implements items that belong to a future task's
scope, it MUST note this explicitly in its completion summary. The
chief-agent MUST then mark those items as already done in the relevant
future task before delegating it — to prevent redundant work or, more
dangerously, missed wiring assumptions.

## Why

In milestone-2, task-2's builder implemented `build_engine_with_args`
signature change and `log_*` audit wiring — both scoped to task-4.
task-4 ran lighter than expected. Benign in this case, but the same
pattern can mask missed wiring: task-N assumes task-M set something up;
task-M's builder did it differently; the gap is invisible until runtime.

## How to apply

**Builder-agent:** If you implement something outside your task spec,
end your summary with an explicit section:

```
## Out-of-scope work completed
- <item>: belongs to task-N. That task can skip or verify this.
```

**Chief-agent:** Before delegating task-N, re-read task-(N-1)'s
completion summary. If it contains out-of-scope items that overlap
with task-N, update the task-N prompt to reflect what is already done
and what remains.

## Origin

Milestone 2, task-2 builder absorbed task-4 scope items silently.
Chief-agent noticed mid-execution but did not update task-4's spec.
