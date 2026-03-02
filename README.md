# CoinTUI

A terminal-based personal finance manager built with Rust, Ratatui, and SQLite.

Track income, expenses, budgets, and recurring transactions — all from the comfort of your terminal.

## Features

- **Dashboard** — At-a-glance view of your financial health: total income, expenses, balance, recent transactions, and budget alerts
- **Transaction management** — Add, edit, and delete income/expense records with source, amount, date, category, and notes
- **Hierarchical tags** — Organize transactions with categories and subcategories (e.g., Comida > Restaurantes)
- **Budgets with alerts** — Set spending limits per category or globally (weekly/monthly/yearly) with visual progress bars and warnings at 80%+ usage
- **Recurring transactions** — Full CRUD for recurring entries (daily/weekly/monthly/yearly) with configurable day-of-month and month; auto-insert on startup
- **Statistics** — Four sub-tabs (Overview, Trends, Budgets, AI Insights) with totals, savings rate gauge, expense breakdown by tag, monthly trends with configurable range (6/12/24 months), budget status gauges, and AI-generated insights
- **AI insights** — Local AI-powered spending analysis via Ollama: monthly/yearly insights with trend detection, anomalies, and savings opportunities (press `g` in Stats > AI Insights tab)
- **Smart search** — Natural language transaction search via Ollama (e.g., `--ask "how much did I spend on food last month"`)
- **Reports** — Generate monthly, yearly, or comparison reports in terminal or Markdown format
- **Filtering** — Search transactions by text, date range, amount range, kind, and tag
- **Tag management** — Create, rename, and delete tags from CLI or TUI with safe reassignment when tags are in use
- **CLI quick-add** — Add transactions directly from the command line without entering the TUI
- **CSV import** — Import transactions from CSV files with interactive column mapping
- **CSV/JSON export** — Export all transactions to CSV or JSON format
- **Database backup/restore** — Create and restore full database backups
- **Persistent storage** — SQLite database with WAL mode, automatic schema migrations, and safe parameterized queries
- **Locale-aware formatting** — Configurable thousands/decimal separators (default: Chilean format `$ 2.700.000,00`)
- **Configurable** — TOML config for currency symbol, number format, database path, and AI settings
- **Email sync** — Automatic transaction import from Gmail bank notifications (Santander, Scotiabank, CMR Falabella, Uber, PedidosYa) via IMAP with deduplication
- **AI rules engine** — Natural language rules for smart tag assignment during email sync (e.g., "transfers to KARINA of $300.000 → Pensión", "transfers from FRANCO CASTRO → SKIP")
- **Privacy-first** — All data stays local: SQLite database, local Ollama for AI — nothing leaves your machine

## Installation

### From source

```bash
git clone https://github.com/tu-usuario/cointui.git
cd cointui
cargo build --release
```

The binary will be at `target/release/cointui`.

### Requirements

- Rust 2024 edition (1.85+)
- No external runtime dependencies — SQLite is bundled via `rusqlite`
- Optional: [Ollama](https://ollama.ai) for AI features

## Usage

```bash
# Launch the TUI
cointui

# Use a custom config file
cointui --config /path/to/config.toml
```

### Quick-add transactions from the CLI

```bash
# Minimal (defaults: kind=expense, date=today, tag=Otros)
cointui --add "Supermercado" --amount 50.00

# Full options
cointui --add "Supermercado" --amount 50.00 \
  --tag Comida --kind expense \
  --date 2026-02-25 --notes "weekly groceries"

# Income
cointui --add "Salario" --amount 3000 --kind income --tag Salario
```

### Tag management

```bash
# List all tags
cointui --tags

# Add a new tag
cointui --add-tag "Food"

# Rename a tag
cointui --rename-tag "Food:Groceries"

# Delete a tag (blocks if transactions or recurring entries reference it)
cointui --delete-tag "Groceries"
```

### Import / Export

```bash
# Import from CSV (interactive column mapping)
cointui --import transactions.csv

# Export to JSON (format detected from extension)
cointui --export transactions.json

# Export to CSV
cointui --export transactions.csv

# Force a specific format
cointui --export data.txt --format json
```

### Backup / Restore

```bash
# Create a backup (auto-generated timestamped filename)
cointui --backup

# Create a backup at a specific path
cointui --backup /path/to/backup.db

# Restore from a backup
cointui --restore /path/to/backup.db
```

### Reports

```bash
# Monthly report (current month, terminal output)
cointui --report monthly

# Monthly report for a specific month
cointui --report monthly 2026-01

# Yearly report
cointui --report yearly
cointui --report yearly 2025

# Compare two months
cointui --report compare 2026-01 2026-02

# Export report to Markdown
cointui --report monthly --output report.md
```

### Email sync (Gmail)

```bash
# Sync bank notification emails and import transactions
cointui --sync-email
```

Supported senders: Santander, Scotiabank, CMR Falabella, Uber (rides + Eats), PedidosYa. Requires Gmail app password (see [Gmail Config](#gmail-options-gmail-section)).

### AI Features (requires Ollama)

```bash
# Generate spending insights for the current month
cointui --insights

# Insights for a specific month
cointui --insights 2026-01

# Insights for a full year
cointui --insights 2025

# Natural language search
cointui --ask "how much did I spend on food last month"
cointui --ask "cuanto gaste en uber el trimestre pasado"
```

> **Setup**: Install [Ollama](https://ollama.ai), pull a model (`ollama pull qwen2.5:14b`), and enable AI in your config (see [Configuration](#configuration)).

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `1` - `6` | Switch between views |
| `Tab` / `Shift+Tab` | Cycle views |
| `q` | Quit |
| `Esc` | Return to Dashboard |

### Transaction list

| Key | Action |
|-----|--------|
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `a` | Add new transaction |
| `e` | Edit selected transaction |
| `d` | Delete selected transaction |
| `/` | Filter by text |

### Transaction form

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Next / previous field |
| `Space` | Toggle or cycle option fields (Kind, Tag) |
| `Enter` | Save transaction |
| `Esc` | Cancel |

### Budgets

| Key | Action |
|-----|--------|
| `a` | Add new budget |
| `e` | Edit selected budget |
| `d` | Delete selected budget |

### Stats

| Key | Action |
|-----|--------|
| `h` / `Left` | Previous sub-tab |
| `l` / `Right` | Next sub-tab |
| `m` | Toggle period / cycle month range |
| `g` | Generate AI insights (AI Insights tab) |

### Recurring entries

| Key | Action |
|-----|--------|
| `a` | Add new recurring entry |
| `e` | Edit selected entry |
| `Space` | Toggle active/inactive |
| `d` | Delete entry |

### Tags

| Key | Action |
|-----|--------|
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `a` | Add new tag |
| `e` | Edit selected tag |
| `d` | Delete selected tag |

## Views

### 1. Dashboard

```
┌─ INCOME ──────┐┌─ BALANCE ─────┐┌─ EXPENSES ────┐
│ $ 3.500.000,00 ││ $ 1.200.000,00 ││ $ 2.300.000,00 │
└────────────────┘└────────────────┘└────────────────┘
┌─ Recent Transactions ─────────────────────────────┐
│ Date       Source          Amount    Type    Tag   │
│ 2026-02-25 Supermercado  $ 45.000,00 EXP   Comida │
│ 2026-02-01 Salario    $ 2.700.000,00 INC   Salario│
└───────────────────────────────────────────────────┘
┌─ Budget Alerts ───────────────────────────────────┐
│ ⚠ Comida: 85% used ($ 425.000,00 / $ 500.000,00) │
└───────────────────────────────────────────────────┘
```

### 2. Transactions

Full scrollable list with all transactions, filterable by text, date, amount, kind, and tag. Color-coded amounts (green for income, red for expenses).

### 3. Stats

Four sub-tabs navigable with `h`/`l`:

- **Overview** — Income/Balance/Expenses header, savings rate gauge, and expense breakdown by tag with horizontal bars and percentages
- **Trends** — Monthly income vs. expenses with line gauges, configurable range (6/12/24 months via `m` key)
- **Budgets** — Budget progress gauges with on-track/warning/over-budget summary
- **AI Insights** — AI-generated spending analysis via Ollama (press `g` to generate)

### 4. Budgets

List of budget rules with gauge progress bars. Color indicators: green (< 60%), yellow (60-80%), red (> 80%).

### 5. Recurring

Full CRUD for recurring transaction templates. Add, edit, toggle active/inactive, and delete entries. Monthly entries let you pick the day of month (1-31), yearly entries let you pick month and day. Schedule is shown in the Interval column (e.g. "Monthly (15)", "Yearly (Mar 15)").

### 6. Tags

Manage category tags. Add, rename, and delete tags. When deleting a tag that has transactions or recurring entries, a reassignment modal lets you pick which tag to move them to before deletion.

## Configuration

Config file location: `~/.config/cointui/config.toml`

A default config is auto-created on first run:

```toml
currency = "$"
thousands_separator = "."
decimal_separator = ","

[ai]
enabled = false
ollama_url = "http://localhost:11434"
ollama_model = "qwen2.5:14b"
timeout_secs = 30
```

### Options

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `currency` | String | `"$"` | Currency symbol displayed next to amounts |
| `thousands_separator` | String | `"."` | Thousands grouping separator (e.g. `"."` for `1.000`, `","` for `1,000`) |
| `decimal_separator` | String | `","` | Decimal separator (e.g. `","` for `1.000,50`, `"."` for `1,000.50`) |
| `db_path` | String | (auto) | Override database file path |

### AI Options (`[ai]` section)

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Bool | `false` | Enable AI features (requires Ollama) |
| `ollama_url` | String | `"http://localhost:11434"` | Ollama API endpoint |
| `ollama_model` | String | `"qwen2.5:14b"` | Ollama model to use |
| `timeout_secs` | Integer | `30` | Request timeout in seconds |

### Gmail Options (`[gmail]` section)

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Bool | `false` | Enable Gmail email sync |
| `email` | String | `""` | Gmail address |
| `app_password` | String | (none) | Gmail app password (or use `COINTUI_GMAIL_PASSWORD` env var) |
| `imap_host` | String | `"imap.gmail.com"` | IMAP server host |
| `imap_port` | Integer | `993` | IMAP server port |
| `lookback_days` | Integer | `90` | How many days back to search for emails |
| `ai_tag_fallback` | Bool | `false` | Use AI to assign tags when no rule matches |
| `rules_prompt` | String | `""` | Natural language rules for AI tag assignment (see below) |
| `tag_rules` | Array | `[]` | Keyword-based tag rules (simple matching) |

#### AI Rules Engine

When `rules_prompt` is set, each synced transaction is sent to Ollama with your rules for smart tag assignment. Rules are written in natural language:

```toml
[gmail]
rules_prompt = """
- Transfers to KARINA OLIVERO of exactly $300.000 → tag "Pensión"
- Transfers to KARINA OLIVERO of any other amount → tag "Otros"
- Transfers from FRANCO CASTRO → SKIP (own-account transfer)
- Payments for Metrogas, Enel, Aguas Andinas → tag "Departamento"
- Supermarket purchases (Lider, Jumbo, Santa Isabel) → tag "Comida"
"""
```

The AI can reply with a tag name or "SKIP" to ignore a transaction. If no rule matches, it assigns the most appropriate tag from your tag list.

## Data storage

- **Database**: SQLite at `~/.local/share/cointui/cointui.db`
- **Config**: TOML at `~/.config/cointui/config.toml`
- Amounts are stored as **whole currency units** (integer) to avoid floating-point errors
- WAL journal mode enabled for performance
- Foreign keys enforced

### Schema

Five tables: `tags`, `transactions`, `budgets`, `recurring_entries`, `processed_emails`. See `src/db/connection.rs` for the full schema.

## Architecture

```
src/
├── main.rs              # Entry point, CLI flag dispatch, terminal setup, event loop
├── app.rs               # App state machine (View, Mode, command dispatch)
├── event.rs             # Event bus (crossterm events + tick)
├── error.rs             # Error types (thiserror)
├── config.rs            # TOML configuration
├── ai/
│   ├── ollama.rs        # OllamaClient (sync HTTP via ureq)
│   └── prompts.rs       # Prompt templates for insights, search, and AI rules
├── cli/
│   ├── add.rs           # --add transaction from CLI
│   ├── ask.rs           # --ask natural language search
│   ├── insights.rs      # --insights AI spending analysis
│   ├── report.rs        # --report monthly/yearly/compare
│   ├── sync_email.rs    # --sync-email Gmail sync
│   ├── tags.rs          # --tags, --add-tag, --rename-tag, --delete-tag
│   ├── import.rs        # --import CSV with column mapping
│   ├── export.rs        # --export to CSV/JSON
│   └── backup.rs        # --backup / --restore
├── domain/
│   └── models.rs        # Transaction, Tag, Budget, RecurringEntry
├── db/
│   ├── connection.rs    # SQLite connection, schema, migrations
│   ├── transaction_repo.rs  # Transaction CRUD + filtering
│   ├── tag_repo.rs      # Tag CRUD (hierarchical)
│   ├── budget_repo.rs   # Budget CRUD + spent calculation
│   ├── recurring_repo.rs    # Recurring entry CRUD
│   └── email_repo.rs    # Processed email deduplication
├── email/
│   ├── imap_client.rs   # IMAP connection and email fetching
│   ├── sync.rs          # Sync orchestration, AI rules engine
│   └── parsers/         # Bank-specific email parsers (Santander, Scotiabank, CMR, Uber, PedidosYa)
└── ui/
    ├── theme.rs         # Tokyo Night color palette
    └── views/
        ├── dashboard.rs # Balance header, recent transactions, alerts
        ├── transactions.rs  # Full transaction list
        ├── form.rs      # Add/edit transaction popup form
        ├── filter_form.rs   # Advanced filter popup form
        ├── stats.rs     # Charts and financial summaries
        ├── budget.rs    # Budget list with progress gauges
        ├── recurring.rs # Recurring entry management
        ├── tags.rs      # Tag management with add/edit/delete
        └── help.rs      # Help overlay
```

**Layered architecture**: CLI / UI -> Event Bus -> App State -> Repository (SQLite)

## Tech stack

| Crate | Version | Purpose |
|-------|---------|---------|
| [ratatui](https://ratatui.rs) | 0.29 | Terminal UI framework |
| [crossterm](https://github.com/crossterm-rs/crossterm) | 0.28 | Terminal backend |
| [rusqlite](https://github.com/rusqlite/rusqlite) | 0.32 | SQLite (bundled) |
| [chrono](https://github.com/chronotope/chrono) | 0.4 | Date/time handling |
| [clap](https://github.com/clap-rs/clap) | 4 | CLI argument parsing |
| [serde](https://serde.rs) + [toml](https://github.com/toml-rs/toml) | 1 / 0.8 | Config serialization |
| [serde_json](https://github.com/serde-rs/json) | 1 | JSON export/AI response parsing |
| [thiserror](https://github.com/dtolnay/thiserror) | 2 | Error derivation |
| [csv](https://github.com/BurntSushi/rust-csv) | 1.3 | CSV import/export |
| [directories](https://github.com/dirs-dev/directories-rs) | 6 | XDG paths |
| [ureq](https://github.com/algesten/ureq) | 3 | Sync HTTP client (Ollama API) |
| [imap](https://crates.io/crates/imap) | 2.4 | IMAP client for Gmail |
| [native-tls](https://crates.io/crates/native-tls) | 0.2 | TLS for IMAP |
| [mailparse](https://crates.io/crates/mailparse) | 0.15 | Email MIME parsing |
| [regex](https://crates.io/crates/regex) | 1 | Bank email content extraction |

## Development

```bash
cargo check          # Fast compilation check
cargo test           # Run all tests (105 tests)
cargo clippy         # Lint (must pass with zero warnings)
cargo build --release
```

## Roadmap

- [x] Help overlay (`?` key)
- [x] Sortable columns in transaction table
- [x] Tag management (CLI + TUI)
- [x] Stats redesign with sub-tabs (Overview, Trends, Budgets)
- [x] Recurring CRUD with configurable intervals (day of month, month)
- [x] AI insights via Ollama (CLI + TUI)
- [x] Natural language search (`--ask`)
- [x] Reports (monthly, yearly, compare, Markdown export)
- [x] Email sync from Gmail bank notifications (Santander, Scotiabank, CMR Falabella)
- [x] Uber (rides + Eats) and PedidosYa email parsers with keyword tag rules
- [x] AI rules engine for smart tag assignment during email sync
- [x] Content-based dedup for senders that send duplicate emails (e.g. Uber)
- [x] Marketing email filtering for Scotiabank (rejects promotional emails)

## License

MIT
