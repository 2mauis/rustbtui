# rustbtui

Small native `ratatui` + `crossterm` demo with multiple panes and view switching.

## Run

```bash
cargo run
```

## Controls

- `j` / `Down`: move selection
- `k` / `Up`: move selection
- `Tab`: switch the right-hand view
- `Space`: cycle selected task through `idle -> running -> done`
- `a`: append a synthetic event
- `r`: reset the dashboard
- `q` / `Esc`: quit
