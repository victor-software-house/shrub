---
shrub: minor
---

`SpawnOptions` gains `pids_limits`, a list of `options::PidsLimit` (an open `pids.current` file and the highest count allowed) that each external process checks after it joins `cgroup_procs` and before its program runs.
The kernel does not apply `pids.max` to a process that moves into a cgroup, so a child whose count is above a limit, or cannot be read, fails its spawn without running its program: the command fails to execute, with status 126, and `on_spawn` is not called.
The check reads only descriptors opened before the fork, allocates nothing, and returns an error instead of exiting, so the `exec` builtin, which runs it in the shell's own process, fails without ending the process; on that path the count, which includes every thread of the shell, has the shell's threads other than one subtracted on Linux; an empty list, the default, checks nothing, and on a platform without the check a non-empty list fails every external command.
