# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

CoinTUI is a terminal-based personal finance manager built with Rust, Ratatui 0.29, and SQLite (rusqlite with bundled feature). It uses a layered architecture: UI -> Event Bus -> App State -> Repositories -> SQLite.

## Build & Test Commands

```bash
cargo check                        # Fast compilation check
cargo build                        # Debug build
cargo build --release              # Release build
cargo test                         # Run all tests (62 unit + integration)
cargo test test_name               # Run a single test by name
cargo test module::tests           # Run tests in a specific module (e.g. cargo test cli::add::tests)
cargo clippy                       # Lint (must pass with zero warnings)
cargo run                          # Run the TUI app
cargo run -- --help                # Show CLI help
```

## Architecture

### Layers (top to bottom)

1. **CLI** (`src/cli/`) - Flag-based commands (`--add`, `--tags`, `--add-tag`, `--rename-tag`, `--delete-tag`, `--import`, `--export`, `--backup`, `--restore`, `--report`, `--insights`, `--ask`) that run before TUI launch and exit.
2. **UI** (`src/ui/`) - Ratatui widgets, views, theme. Renders `&App` state into terminal frames.
3. **Event** (`src/event.rs`) - `AppCommand` enum + `EventHandler` polling crossterm events.
4. **App** (`src/app.rs`) - Central state machine. Owns `Database`, dispatches commands, manages cached data.
5. **Domain** (`src/domain/models.rs`) - Pure data structs: `Transaction`, `Tag`, `Budget`, `RecurringEntry`.
6. **Repository** (`src/db/`) - SQLite CRUD. Each repo takes `&Database` reference.
7. **AI** (`src/ai/`) - Ollama integration for AI insights and natural language search. `OllamaClient` (sync HTTP via `ureq`), prompt templates in `prompts.rs`.

### Key types

- `App` - Central state: current `View`, `Mode`, cached data, selection indices, form state
- `View` enum - `Dashboard | Transactions | Stats | Budgets | Recurring | Tags`
- `Mode` enum - `Normal | Adding | Editing | Confirming(String) | Filtering | BudgetAdding | BudgetEditing | RecurringAdding | RecurringEditing | TagEditing | TagDeleting | Help`
- `AppCommand` - All possible user actions (defined in `event.rs`)
- `TransactionForm` - Form state for add/edit (defined in `ui/views/form.rs`)
- `TransactionFilter` - Dynamic query filters (defined in `db/transaction_repo.rs`)
- `TagForm` - Tag add/edit form state (defined in `ui/views/tags.rs`)
- `TagDeleteInfo` - Tag delete modal state with reassignment (defined in `ui/views/tags.rs`)
- `BudgetForm` - Budget add/edit form state (defined in `ui/views/budget.rs`)
- `RecurringForm` - Recurring entry add/edit form state (defined in `ui/views/recurring.rs`)
- `SortColumn` / `SortDirection` - Transaction table sorting state
- `OverviewPeriod` enum - `Monthly | Yearly` for Stats Overview time filter
- Stats sub-tab state: `stats_tab` (0=Overview, 1=Trends, 2=Budgets, 3=AI Insights), `stats_months_range` (6/12/24), `stats_overview_period`, `overview_totals`, `overview_prev_totals`, `overview_expense_by_tag`
- AI state: `ai_insights: Vec<String>`, `ai_loading: bool` — cached AI-generated insights, triggered by `[g]` key in Stats AI tab

### Data flow

```
KeyEvent -> app.handle_key() -> modifies App state + calls repos -> ui::draw() reads App state
```

The event loop in `main.rs`: draw -> poll event -> handle -> tick -> repeat.

CLI commands (`--add`, `--import`, etc.) bypass the event loop entirely — they run in `main()` before terminal initialization and return early.

## Conventions

### Amounts
All monetary amounts are stored as **whole currency units** (`i64`). Use `format_cents(amount, currency, thousands_sep, decimal_sep)` from `domain::models` for display. The separators come from `AppConfig` (default: `.` for thousands, `,` for decimal — Chilean format). Never use `f64` for money (only at CLI input boundary, immediately converted via `val.round() as i64`).

### Error handling
- All fallible operations return `crate::error::Result<T>`
- DB errors surface in the TUI status bar via `app.set_status(err.user_message())`
- Never `.unwrap()` on DB or I/O operations in non-test code
- Use `?` operator to propagate errors

### Database
- All queries use parameterized `rusqlite::params![]` — never interpolate user data into SQL
- Repos take `&Database` references, not raw `Connection`
- Schema lives in `Database::initialize_schema()` in `connection.rs`
- Migrations use `PRAGMA user_version` (currently at version 2)
- Foreign keys are enforced, WAL mode is enabled

### UI / Ratatui patterns
- Layouts use `Layout::vertical/horizontal` with `.areas()` destructuring
- Theme colors and styles come from `ui::theme` — don't hardcode colors in views
- Use `theme::styled_block(title)` for consistent bordered blocks
- Tables needing selection use `render_stateful_widget` with `TableState`
- Popups render `Clear` widget first, then the popup content on top
- Only filter `KeyEventKind::Press` events (crossterm sends Press + Release)
- Views with sub-tabs (Stats) use `Tabs` widget + match on tab index to route to sub-draw functions
- Stats uses `BarChart::grouped` (via fold pattern), `LineGauge`, and Unicode bar chars for visualizations
- Stats Overview reads from `overview_totals`/`overview_expense_by_tag` (period-scoped), not `totals`/`expense_by_tag` (all-time)
- Dashboard uses a separate `dashboard_transactions` cache (always 10 most recent, unfiltered) to avoid showing filtered results

### Config
- TOML format at `~/.config/cointui/config.toml`
- Auto-created with defaults on first run
- `AppConfig` uses `directories` crate for XDG paths
- DB at `~/.local/share/cointui/cointui.db` by default
- Number format defaults: `thousands_separator = "."`, `decimal_separator = ","` (Chilean). New config fields use `#[serde(default)]` for backward compatibility with existing config files.
- Tags are managed via CLI (`--tags`, `--add-tag`, `--rename-tag`, `--delete-tag`) or TUI (view 6). Initial seeds are hardcoded as "Other" and "Salary" in `main.rs`.
- AI config: `[ai]` section with `enabled` (default false), `ollama_url`, `ollama_model`, `timeout_secs`. Uses `#[serde(default)]` for backward compatibility.

### Adding a new view
1. Add variant to `View` enum in `app.rs`
2. Create `src/ui/views/newview.rs` with public draw functions
3. Add `pub mod newview;` in `src/ui/views/mod.rs`
4. Add match arm in `ui::draw()` in `src/ui/mod.rs`
5. Add keybinding in `App::handle_key()` for the new view
6. Update tab titles in `draw_tabs()`

### Adding a new CLI command
1. Create `src/cli/newcmd.rs` with `pub fn run(..., db: &Database) -> Result<()>`
2. Add `pub mod newcmd;` in `src/cli/mod.rs`
3. Add flag(s) to `Cli` struct in `main.rs`
4. Add dispatch block in `main()` before TUI launch (after existing CLI dispatches)

### Adding a new DB table
1. Add `CREATE TABLE IF NOT EXISTS` in `Database::initialize_schema()`
2. Bump `user_version` in `run_migrations()` if altering existing tables
3. Create `src/db/newrepo.rs` with `NewRepo<'a>` struct taking `&'a Database`
4. Add `pub mod newrepo;` in `src/db/mod.rs`
5. Add model struct in `domain/models.rs`

### Tests
- DB tests use `Database::in_memory()` for isolated in-memory SQLite
- Tag repos must seed at least one tag before creating transactions (FK constraint)
- Run `cargo test` before committing — all 62 tests must pass
- Test files live alongside source in `#[cfg(test)] mod tests` blocks

## Pending work (Roadmap)

- Advanced filtering: date range, amount range, tag filter UI (filter_form.rs exists but not fully wired)
