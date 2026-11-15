3efb84947716c040074aa020ef0af6c3809a0e11
No playbook fallback taken.
Network: plugins start under `unshare -rn` (empty network namespace), else `bwrap --unshare-net`; with neither, spawn fails.
