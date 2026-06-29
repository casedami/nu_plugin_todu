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

- Integrates with Nushell pipelines for powerful task management

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

See the [wiki](https://github.com/casedami/nu_plugin_todu/wiki) for details.

## Contributing

Contributions are welcome. Feel free to open a PR or create an issue.
