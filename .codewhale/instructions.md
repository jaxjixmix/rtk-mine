# rtk-mine integration for this project

When starting work in this project, activate rtk-mine by running:

```bash
eval "$(rtk-mine init --agent --quiet)"
```

This wraps `ls`, `cat`, `grep`, `find`, `git`, `cargo`, `npm`, and other commands through rtk-mine's proxy, reducing token usage by 60-90% on command output.

## Verification
```bash
type ls
# Should show: ls is a shell function
```

## Tips
- `rtk-mine audit` — see what commands ran through the proxy
- `rtk-mine audit stats` — savings dashboard
- `command ls -la` — bypass the proxy for raw output
