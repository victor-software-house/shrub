---
shrub: minor
---

`SpawnOptions` gains `pids_limits`, a list of `options::PidsLimit` (an open `pids.current` file and the highest count allowed) that each external process checks after it joins `cgroup_procs` and before its program runs.
The kernel does not apply `pids.max` to a process that moves into a cgroup, so a child whose count is above a limit, or cannot be read, exits with status 126 without running its program.
The check reads only descriptors opened before the fork and allocates nothing; an empty list, the default, checks nothing, and on a platform without the check a non-empty list fails every external command.
