# Todu Plugin

A [Nushell](https://www.nushell.sh/) plugin for managing project-scoped todos, with optional sync to GitHub and Jira.

## Features

- Intuitive inline parsing for quick task creation with natural-language dates

```nu
todu add "!!write tests for backend #tests @tomorrow // need more test cases for feature"
╭───┬─────────────────────────┬─────────┬──────────┬──────┬────────────┬──────────┬───────╮
│ # │          task           │ status  │ priority │ desc │    due     │ subtasks │  tag  │
├───┼─────────────────────────┼─────────┼──────────┼──────┼────────────┼──────────┼───────┤
│ 1 │ write tests for backend │ pending │ medium   │ ...  │ in 8 hours │ ---      │ tests │
╰───┴─────────────────────────┴─────────┴──────────┴──────┴────────────┴──────────┴───────╯
```

- Plays nice with the Nushell ecosystem for powerful task management

```nu
todu | sort-by status
todu | group-by tag 
todu | where priority > medium | get task desc
todu | where status == "pending" | get index | todu tag work
todu add "refactor auth module" | [$"write unit tests ^($in.index)" $"update docs ^($in.index)"] | todu add
```

- Pull GitHub issues and Jira tasks into your project list, with status changes pushed back automatically (requires `--features remote`) — [see Remote setup below](#remote-setup)

## Installation

### From source

```nushell
# Local todos only
cargo build --release
plugin add target/release/nu_plugin_todu
plugin use todu

# With GitHub and Jira support
cargo build --release --features remote
plugin add target/release/nu_plugin_todu
plugin use todu
```

### Requirements

- Nushell 0.113+
- Rust toolchain (for building from source)

## Usage

Run `todu` with no arguments to list todos for the current project (scoped to the git root, or the current directory if not in a repo):

```nushell
todu          # compact view
todu --long   # full view with created date, full description, and expanded subtasks
todu --global # use home directory as the project instead of git root
```

Most subcommands accept `--global` / `-g` to operate on the global list.

## Commands

### Local todos

| Command | Description |
|---|---|
| `todu` | List active todos |
| `todu add <task>` | Add one or more todos |
| `todu start <id>` | Mark as in progress |
| `todu done <id>` | Mark as done |
| `todu pause <id>` | Mark as paused |
| `todu stop <id>` | Mark as stopped |
| `todu reopen <id>` | Reset to pending |
| `todu desc <id> <text>` | Set or update description (`""` to clear) |
| `todu due <id> <date>` | Set or clear due date (`""` to clear) |
| `todu tag <tag> <id...>` | Set or clear a tag (`""` to clear) |
| `todu rm <id>` | Delete a todo |
| `todu clear` | Archive done/stopped todos |
| `todu clear --all` | Archive every todo |
| `todu clear --hard` | Permanently delete archived todos |

### Remote (requires `--features remote`)

| Command | Description |
|---|---|
| `todu remote` | List configured remotes for the project |
| `todu remote add github <url>` | Add a GitHub repo as a remote |
| `todu remote add jira <url>` | Add a Jira instance as a remote |
| `todu remote rm <type>` | Remove the configured remote (`github` or `jira`) |
| `todu pull github` | Pull open issues from all GitHub remotes |
| `todu pull jira` | Pull assigned issues from all Jira remotes |

- [see Remote setup below](#remote-setup)

## Inline syntax

Tokens can appear anywhere in the task string:

```nushell
todu add "buy milk"
todu add "!!ship feature // needs review"        # medium priority, with description
todu add "!!!fix bug #work @2026-07-15"          # high priority, tagged, due date
todu add "write tests ^2 @friday // see parent"  # subtask of #2, natural-language date
```

| Token | Meaning |
|---|---|
| `!` | Low priority |
| `!!` | Medium priority |
| `!!!` | High priority |
| `#<tag>` | Categorisation label |
| `^<N>` | Subtask of todo #N |
| `@<date>` | Due date — YYYY-MM-DD or natural language (`friday`, `next week`) |
| `// <text>` | Inline description (must be last) |

## Pipeline support

```nushell
["task one" "task two" "task three"] | todu add
[1 2 3] | todu done
todu | where status == "pending" | get index | todu tag work

# add a parent task, then pipe its index into subtask creation
todu add "refactor auth module" | todu add $"write unit tests ^($in.index)" $"update docs ^($in.index)"
todu add "refactor auth module" | [$"write unit tests ^($in.index)" $"update docs ^($in.index)"] | todu add
```

## Remote setup

Remotes are configured per-project via commands and stored in the local database. Credentials are never stored in the database — they are read from a token file or the OS keychain at runtime.

### 1. Register remotes

```nushell
cd ~/dev/myproject
todu remote add github https://github.com/myorg/myrepo
todu remote add jira https://myorg.atlassian.net/PROJ
```

The Jira project key is required as a path segment in the URL. Pulls are scoped to that project.

### 2. Configure credentials

Add to `config.nu`:

```nushell
$env.config.plugins.todu = {
    github: {
        token_file: "~/.config/todu/github_token"  # file containing a GitHub PAT
    }
    jira: {
        email:      "me@example.com"
        token_file: "~/.config/todu/jira_token"    # file containing a Jira API token
    }
}
```

Token files should contain only the token string and be `chmod 600`.

Alternatively, omit `token_file` and store tokens in the OS keychain:

| Remote | Service | Account |
|---|---|---|
| GitHub | `todu-github` | `{owner}/{repo}` |
| Jira | `todu-jira` | `{email}` |

```bash
# macOS
security add-generic-password -s todu-github -a myorg/myrepo -w "ghp_xxxx"
security add-generic-password -s todu-jira   -a me@example.com -w "xxxx"

# Linux (secret-service)
secret-tool store --label='todu github' service todu-github account myorg/myrepo
secret-tool store --label='todu jira'   service todu-jira   account me@example.com
```

> **Note:** Linux keychain support requires a running Secret Service (GNOME Keyring or KWallet). Use `token_file` in headless environments.

### 3. Pull and sync

```nushell
todu pull github   # imports open issues from all GitHub remotes
todu pull jira     # imports assigned, non-done issues from all Jira remotes
```

**Status changes are pushed back automatically.** When you run any status command on a remote-sourced todo, todu updates the remote in the same operation — no separate sync step needed:

```nushell
todu done 3     # closes the GitHub issue (reason: completed) / transitions Jira to Done
todu stop 3     # closes the GitHub issue (reason: not_planned) / transitions Jira to Done
todu start 4    # reopens the GitHub issue / transitions Jira to In Progress
todu reopen 5   # reopens the GitHub issue / transitions Jira to To Do
```

For Jira, todu matches transitions by status category rather than by name, so it works regardless of how your board's workflow is named:

| todu status | Jira category |
|---|---|
| `done`, `stop` | Done |
| `start`, `in-review` | In Progress |
| `pending`, `pause` | To Do |

The `tag` column holds the issue identifier (`#42` for GitHub, `PROJ-123` for Jira) and is read-only on remote todos.

## Contributing

Contributions are welcome. Feel free to open a PR or create an issue.
