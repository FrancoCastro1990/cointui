# CoinTUI - Claude Code Guidelines

## Project Overview

CoinTUI is a terminal-based personal finance manager built with Rust, Ratatui 0.29, and SQLite (rusqlite with bundled feature). It uses a layered architecture: UI -> Event Bus -> App State -> Repositories -> SQLite.

## Build & Test Commands

```bash
cargo check          # Fast compilation check
cargo build          # Debug build
cargo build --release # Release build
cargo test           # Run all tests (37 unit + integration)
cargo clippy         # Lint
cargo run            # Run the TUI app
cargo run -- --help  # Show CLI help
```

## Architecture

### Layers (top to bottom)

1. **UI** (`src/ui/`) - Ratatui widgets, views, theme. Renders `&App` state into terminal frames.
2. **Event** (`src/event.rs`) - `AppCommand` enum + `EventHandler` polling crossterm events.
3. **App** (`src/app.rs`) - Central state machine. Owns `Database`, dispatches commands, manages cached data.
4. **Domain** (`src/domain/models.rs`) - Pure data structs: `Transaction`, `Tag`, `Budget`, `RecurringEntry`.
5. **Repository** (`src/db/`) - SQLite CRUD. Each repo takes `&Database` reference.

### Key types

- `App` - Central state: current `View`, `Mode`, cached data, selection indices, form state
- `View` enum - `Dashboard | Transactions | Stats | Budgets | Recurring`
- `Mode` enum - `Normal | Adding | Editing | Confirming(String) | Filtering(String)`
- `AppCommand` - All possible user actions (defined in `event.rs`)
- `TransactionForm` - Form state for add/edit (defined in `ui/views/form.rs`)
- `TransactionFilter` - Dynamic query filters (defined in `db/transaction_repo.rs`)

### Data flow

```
KeyEvent -> app.handle_key() -> modifies App state + calls repos -> ui::draw() reads App state
```

The event loop in `main.rs`: draw -> poll event -> handle -> tick -> repeat.

## File Map

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point, terminal setup/restore, event loop |
| `src/app.rs` | App state machine, key handling, command dispatch |
| `src/event.rs` | `AppCommand`, `AppEvent`, `EventHandler` |
| `src/error.rs` | `AppError` enum (thiserror), `Result<T>` alias |
| `src/config.rs` | `AppConfig` TOML loading/saving, XDG paths |
| `src/domain/models.rs` | All domain structs + `format_centavos()` helper |
| `src/db/connection.rs` | `Database` struct, schema creation, migrations |
| `src/db/transaction_repo.rs` | `TransactionRepo` + `TransactionFilter` |
| `src/db/tag_repo.rs` | `TagRepo` (hierarchical tags) |
| `src/db/budget_repo.rs` | `BudgetRepo` + period-aware spent calculation |
| `src/db/recurring_repo.rs` | `RecurringRepo` |
| `src/ui/mod.rs` | Main `draw()` dispatcher, tabs, status bar, confirm dialog |
| `src/ui/theme.rs` | Tokyo Night color palette, style helpers |
| `src/ui/views/dashboard.rs` | Header (income/balance/expenses), recent transactions, alerts |
| `src/ui/views/transactions.rs` | Standalone transaction table (unused, inline in ui/mod.rs) |
| `src/ui/views/form.rs` | `TransactionForm` + `draw_form()` popup overlay |
| `src/ui/views/stats.rs` | BarChart, summary, monthly table |
| `src/ui/views/budget.rs` | Budget list with Gauge progress bars |
| `src/ui/views/recurring.rs` | Recurring entry table (standalone version) |

## Conventions

### Amounts
All monetary amounts are stored as **centavos** (`i64`). Use `format_centavos(amount, currency)` from `domain::models` for display. Never use `f64` for money.

### Error handling
- All fallible operations return `crate::error::Result<T>`
- DB errors surface in the TUI status bar via `app.set_status(err.user_message())`
- Never `.unwrap()` on DB or I/O operations in non-test code
- Use `?` operator to propagate errors

### Database
- All queries use parameterized `rusqlite::params![]` - never interpolate user data into SQL
- Repos take `&Database` references, not raw `Connection`
- Schema lives in `Database::initialize_schema()` in `connection.rs`
- Migrations use `PRAGMA user_version` (currently at version 1)
- Foreign keys are enforced, WAL mode is enabled

### UI / Ratatui patterns
- Use `ratatui::init()` / `ratatui::restore()` for terminal lifecycle
- Layouts use `Layout::vertical/horizontal` with `.areas()` destructuring
- Theme colors and styles come from `ui::theme` - don't hardcode colors in views
- Use `theme::styled_block(title)` for consistent bordered blocks
- Tables needing selection use `render_stateful_widget` with `TableState`
- Popups render `Clear` widget first, then the popup content on top
- Only filter `KeyEventKind::Press` events (crossterm sends Press + Release)

### Config
- TOML format at `~/.config/cointui/config.toml`
- Auto-created with defaults on first run
- `AppConfig` uses `directories` crate for XDG paths
- DB at `~/.local/share/cointui/cointui.db` by default

### Adding a new view
1. Add variant to `View` enum in `app.rs`
2. Create `src/ui/views/newview.rs` with public draw functions
3. Add `pub mod newview;` in `src/ui/views/mod.rs`
4. Add match arm in `ui::draw()` in `src/ui/mod.rs`
5. Add keybinding in `App::handle_key()` for the new view
6. Update tab titles in `draw_tabs()`

### Adding a new DB table
1. Add `CREATE TABLE IF NOT EXISTS` in `Database::initialize_schema()`
2. Bump `user_version` in `run_migrations()` if altering existing tables
3. Create `src/db/newrepo.rs` with `NewRepo<'a>` struct taking `&'a Database`
4. Add `pub mod newrepo;` in `src/db/mod.rs`
5. Add model struct in `domain/models.rs`

### Tests
- DB tests use `Database::in_memory()` for isolated in-memory SQLite
- Tag repos must seed at least one tag before creating transactions (FK constraint)
- Run `cargo test` before committing - all 37 tests must pass
- Test files live alongside source in `#[cfg(test)] mod tests` blocks

## Pending work (Roadmap)

- Filtering: date range, amount range, tag filter (beyond text search)
- CSV import with column mapping (`csv` crate already in deps)
- CSV/JSON export
- Database backup/restore
- Extended CLI args (`--import`, `--export`, `--backup`)
- Help overlay (`?` key)
- Sortable transaction table columns
