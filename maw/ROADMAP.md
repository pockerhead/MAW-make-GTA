# Roadmap graph (derived from task.md Dependencies — task.md is source of truth)

TASK-006  (blocked by TASK-005 [waits on TASK-005 (in_progress)])
TASK-007  (blocked by TASK-006)
TASK-008  (blocked by TASK-006)
TASK-009  (blocked by TASK-005 [waits on TASK-005 (in_progress)], TASK-006)
TASK-010  (blocked by TASK-007, TASK-008, TASK-009)
TASK-011  (blocked by TASK-007, TASK-009)
TASK-012  (blocked by TASK-010, TASK-011)
TASK-013  (blocked by TASK-011, TASK-012)
TASK-014  (blocked by TASK-007, TASK-008, TASK-012)
TASK-015  (blocked by TASK-006, TASK-009, TASK-013)
TASK-016  (blocked by TASK-012, TASK-015)
TASK-017  (blocked by TASK-005 [waits on TASK-005 (in_progress)], TASK-006, TASK-007, TASK-008, TASK-009, TASK-010, TASK-011, TASK-012, TASK-013, TASK-014, TASK-015, TASK-016)

Soft:
- TASK-008 prefer after TASK-007
