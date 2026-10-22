---
shrub: minor
---

`ExecutionParameters` gains `spawn: SpawnOptions`, which an embedder uses to see each external process the shell starts (`on_spawn`, called with its pid and process group) and to move each one into a cgroup v2 before its program runs (`cgroup_procs`, a `cgroup.procs` path the child writes `0` to).
A child that cannot write that file fails to execute, so no command runs outside its cgroup; on a platform without cgroups, setting the path fails every external command.
The default does neither, so existing callers behave as before.
