use std::time::Instant;

use chrono::Datelike;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::AppConfig;
use crate::db::budget_repo::BudgetRepo;
use crate::db::connection::Database;
use crate::db::recurring_repo::RecurringRepo;
use crate::db::tag_repo::TagRepo;
use crate::db::transaction_repo::{TransactionFilter, TransactionRepo};
use crate::domain::models::{
    Budget, BudgetPeriod, RecurringEntry, RecurringInterval, Tag, Transaction,
};
use crate::error::Result;
use crate::ui::views::form::TransactionForm;

/// Which top-level view is displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Dashboard,
    Transactions,
    Stats,
    Budgets,
    Recurring,
}

/// Interaction mode, layered on top of the current view.
#[derive(Debug, Clone)]
pub enum Mode {
    /// Normal browsing / navigation.
    Normal,
    /// Transaction add form is open.
    Adding,
    /// Transaction edit form is open.
    Editing,
    /// Waiting for user to confirm an action.  The string is the prompt message.
    Confirming(String),
    /// Text filter input mode in the transactions view.
    Filtering(String),
}

/// Pending action to execute after confirmation.
#[derive(Debug, Clone)]
pub enum PendingAction {
    DeleteTransaction(i64),
    DeleteRecurring(i64),
    DeleteBudget(i64),
}

/// Central application state.
pub struct App {
    pub current_view: View,
    pub mode: Mode,
    pub db: Database,
    pub config: AppConfig,
    pub db_path_display: String,

    // Cached data.
    pub transactions: Vec<Transaction>,
    pub tags: Vec<Tag>,
    pub budgets: Vec<Budget>,
    pub recurring_entries: Vec<RecurringEntry>,
    pub totals: (i64, i64),
    pub monthly_totals: Vec<(String, i64, i64)>,
    /// (budget, amount_spent) pairs for all active budgets.
    pub budget_spending: Vec<(Budget, i64)>,

    // Selection indices.
    pub tx_selected: usize,
    pub budget_selected: usize,
    pub recurring_selected: usize,

    // Temporary status message.
    pub status_message: Option<(String, Instant)>,

    // Filter state for transactions view.
    pub filter: TransactionFilter,

    // Form state (used for Adding / Editing modes).
    pub form: Option<TransactionForm>,

    // Pending confirmation action.
    pub pending_action: Option<PendingAction>,

    // Whether the app should quit.
    pub should_quit: bool,
}

impl App {
    /// Create and initialise a new App, loading all data from the database.
    pub fn new(db: Database, config: AppConfig, db_path_display: String) -> Result<Self> {
        let mut app = Self {
            current_view: View::Dashboard,
            mode: Mode::Normal,
            db,
            config,
            db_path_display,
            transactions: Vec::new(),
            tags: Vec::new(),
            budgets: Vec::new(),
            recurring_entries: Vec::new(),
            totals: (0, 0),
            monthly_totals: Vec::new(),
            budget_spending: Vec::new(),
            tx_selected: 0,
            budget_selected: 0,
            recurring_selected: 0,
            status_message: None,
            filter: TransactionFilter::default(),
            form: None,
            pending_action: None,
            should_quit: false,
        };
        app.reload_all()?;
        Ok(app)
    }

    // -----------------------------------------------------------------------
    // Data loading
    // -----------------------------------------------------------------------

    pub fn reload_all(&mut self) -> Result<()> {
        self.reload_tags()?;
        self.reload_transactions()?;
        self.reload_budgets()?;
        self.reload_recurring()?;
        self.reload_totals()?;
        self.reload_monthly_totals()?;
        self.reload_budget_spending()?;
        Ok(())
    }

    pub fn reload_transactions(&mut self) -> Result<()> {
        let repo = TransactionRepo::new(&self.db);
        self.transactions = if self.has_active_filter() {
            repo.get_filtered(&self.filter)?
        } else {
            repo.get_all()?
        };
        // Clamp selection.
        if !self.transactions.is_empty() {
            self.tx_selected = self.tx_selected.min(self.transactions.len() - 1);
        } else {
            self.tx_selected = 0;
        }
        Ok(())
    }

    pub fn reload_tags(&mut self) -> Result<()> {
        let repo = TagRepo::new(&self.db);
        self.tags = repo.get_all()?;
        Ok(())
    }

    pub fn reload_budgets(&mut self) -> Result<()> {
        let repo = BudgetRepo::new(&self.db);
        self.budgets = repo.get_all()?;
        if !self.budgets.is_empty() {
            self.budget_selected = self.budget_selected.min(self.budgets.len() - 1);
        } else {
            self.budget_selected = 0;
        }
        Ok(())
    }

    pub fn reload_recurring(&mut self) -> Result<()> {
        let repo = RecurringRepo::new(&self.db);
        self.recurring_entries = repo.get_all()?;
        if !self.recurring_entries.is_empty() {
            self.recurring_selected = self.recurring_selected.min(self.recurring_entries.len() - 1);
        } else {
            self.recurring_selected = 0;
        }
        Ok(())
    }

    pub fn reload_totals(&mut self) -> Result<()> {
        let repo = TransactionRepo::new(&self.db);
        self.totals = repo.get_totals()?;
        Ok(())
    }

    pub fn reload_monthly_totals(&mut self) -> Result<()> {
        let repo = TransactionRepo::new(&self.db);
        self.monthly_totals = repo.get_monthly_totals(6)?;
        Ok(())
    }

    pub fn reload_budget_spending(&mut self) -> Result<()> {
        let budget_repo = BudgetRepo::new(&self.db);
        let active = budget_repo.get_active()?;
        let mut spending = Vec::new();
        for b in active {
            let spent = budget_repo.get_spent_for_budget(&b)?;
            spending.push((b, spent));
        }
        self.budget_spending = spending;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Look up a tag name by id, returning "Unknown" if not found.
    pub fn tag_name(&self, tag_id: i64) -> String {
        self.tags
            .iter()
            .find(|t| t.id == Some(tag_id))
            .map(|t| t.name.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// Set a temporary status message that will auto-clear.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    /// Clear the status message if it has expired (3 seconds).
    pub fn tick_status(&mut self) {
        if let Some((_, instant)) = &self.status_message {
            if instant.elapsed().as_secs() >= 3 {
                self.status_message = None;
            }
        }
    }

    fn has_active_filter(&self) -> bool {
        self.filter.search.is_some()
            || self.filter.kind.is_some()
            || self.filter.tag_id.is_some()
            || self.filter.date_from.is_some()
            || self.filter.date_to.is_some()
            || self.filter.min_amount.is_some()
            || self.filter.max_amount.is_some()
    }

    fn tag_names_and_ids(&self) -> (Vec<String>, Vec<i64>) {
        let names: Vec<String> = self.tags.iter().map(|t| t.name.clone()).collect();
        let ids: Vec<i64> = self.tags.iter().filter_map(|t| t.id).collect();
        (names, ids)
    }

    // -----------------------------------------------------------------------
    // Key handling
    // -----------------------------------------------------------------------

    /// Main entry point for key handling. Routes based on current mode and view.
    pub fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+C always quits.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        match &self.mode {
            Mode::Confirming(_) => self.handle_confirm_key(key),
            Mode::Adding | Mode::Editing => self.handle_form_key(key),
            Mode::Filtering(_) => self.handle_filter_key(key),
            Mode::Normal => self.handle_normal_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        // Global keys first.
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('1') => {
                self.current_view = View::Dashboard;
                return;
            }
            KeyCode::Char('2') => {
                self.current_view = View::Transactions;
                return;
            }
            KeyCode::Char('3') => {
                self.current_view = View::Stats;
                return;
            }
            KeyCode::Char('4') => {
                self.current_view = View::Budgets;
                return;
            }
            KeyCode::Char('5') => {
                self.current_view = View::Recurring;
                return;
            }
            KeyCode::Tab => {
                self.current_view = match self.current_view {
                    View::Dashboard => View::Transactions,
                    View::Transactions => View::Stats,
                    View::Stats => View::Budgets,
                    View::Budgets => View::Recurring,
                    View::Recurring => View::Dashboard,
                };
                return;
            }
            KeyCode::BackTab => {
                self.current_view = match self.current_view {
                    View::Dashboard => View::Recurring,
                    View::Transactions => View::Dashboard,
                    View::Stats => View::Transactions,
                    View::Budgets => View::Stats,
                    View::Recurring => View::Budgets,
                };
                return;
            }
            KeyCode::Esc => {
                self.current_view = View::Dashboard;
                return;
            }
            _ => {}
        }

        // View-specific keys.
        match self.current_view {
            View::Dashboard => {} // Dashboard only uses global keys.
            View::Transactions => self.handle_transactions_key(key),
            View::Stats => {}
            View::Budgets => self.handle_budgets_key(key),
            View::Recurring => self.handle_recurring_key(key),
        }
    }

    fn handle_transactions_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.tx_selected > 0 {
                    self.tx_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.transactions.is_empty() && self.tx_selected < self.transactions.len() - 1
                {
                    self.tx_selected += 1;
                }
            }
            KeyCode::Char('a') => {
                let (names, ids) = self.tag_names_and_ids();
                let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
                self.form = Some(TransactionForm::new(names, ids, &today));
                self.mode = Mode::Adding;
            }
            KeyCode::Char('e') => {
                if let Some(tx) = self.transactions.get(self.tx_selected) {
                    let (names, ids) = self.tag_names_and_ids();
                    self.form = Some(TransactionForm::from_transaction(tx, names, ids));
                    self.mode = Mode::Editing;
                } else {
                    self.set_status("No transaction selected.");
                }
            }
            KeyCode::Char('d') => {
                if let Some(tx) = self.transactions.get(self.tx_selected) {
                    if let Some(id) = tx.id {
                        self.pending_action = Some(PendingAction::DeleteTransaction(id));
                        self.mode =
                            Mode::Confirming(format!("Delete transaction '{}'?", tx.source));
                    }
                } else {
                    self.set_status("No transaction selected.");
                }
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Filtering(String::new());
            }
            KeyCode::Char('c') => {
                self.filter = TransactionFilter::default();
                if let Err(e) = self.reload_transactions() {
                    self.set_status(e.user_message());
                } else {
                    self.set_status("Filters cleared.");
                }
            }
            _ => {}
        }
    }

    fn handle_budgets_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.budget_selected > 0 {
                    self.budget_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.budget_spending.is_empty()
                    && self.budget_selected < self.budget_spending.len() - 1
                {
                    self.budget_selected += 1;
                }
            }
            KeyCode::Char('a') => {
                // Quick-add a budget: create a monthly budget for the first tag.
                if self.tags.is_empty() {
                    self.set_status("No tags available to create a budget.");
                    return;
                }
                let tag_id = self.tags[0].id;
                let budget = Budget {
                    id: None,
                    tag_id,
                    amount: 100_00, // Default $100
                    period: BudgetPeriod::Monthly,
                    active: true,
                };
                let repo = BudgetRepo::new(&self.db);
                match repo.create(&budget) {
                    Ok(_) => {
                        if let Err(e) = self.reload_budgets().and_then(|_| self.reload_budget_spending()) {
                            self.set_status(e.user_message());
                        } else {
                            self.set_status("Budget created. Edit via DB for now.");
                        }
                    }
                    Err(e) => self.set_status(e.user_message()),
                }
            }
            KeyCode::Char('d') => {
                if let Some((budget, _)) = self.budget_spending.get(self.budget_selected) {
                    if let Some(id) = budget.id {
                        self.pending_action = Some(PendingAction::DeleteBudget(id));
                        self.mode = Mode::Confirming("Delete this budget?".into());
                    }
                } else {
                    self.set_status("No budget selected.");
                }
            }
            _ => {}
        }
    }

    fn handle_recurring_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.recurring_selected > 0 {
                    self.recurring_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.recurring_entries.is_empty()
                    && self.recurring_selected < self.recurring_entries.len() - 1
                {
                    self.recurring_selected += 1;
                }
            }
            KeyCode::Char(' ') => {
                if let Some(entry) = self.recurring_entries.get(self.recurring_selected) {
                    if let Some(id) = entry.id {
                        let repo = RecurringRepo::new(&self.db);
                        match repo.toggle_active(id) {
                            Ok(()) => {
                                if let Err(e) = self.reload_recurring() {
                                    self.set_status(e.user_message());
                                } else {
                                    self.set_status("Toggled recurring entry.");
                                }
                            }
                            Err(e) => self.set_status(e.user_message()),
                        }
                    }
                }
            }
            KeyCode::Char('d') => {
                if let Some(entry) = self.recurring_entries.get(self.recurring_selected) {
                    if let Some(id) = entry.id {
                        self.pending_action = Some(PendingAction::DeleteRecurring(id));
                        self.mode = Mode::Confirming(format!(
                            "Delete recurring entry '{}'?",
                            entry.source
                        ));
                    }
                } else {
                    self.set_status("No recurring entry selected.");
                }
            }
            _ => {}
        }
    }

    fn handle_form_key(&mut self, key: KeyEvent) {
        let Some(ref mut form) = self.form else {
            self.mode = Mode::Normal;
            return;
        };

        match key.code {
            KeyCode::Esc => {
                self.form = None;
                self.mode = Mode::Normal;
            }
            KeyCode::Tab => {
                form.next_field();
            }
            KeyCode::BackTab => {
                form.prev_field();
            }
            KeyCode::Char(' ') => {
                // Toggle/cycle on toggle fields.
                let field = form.current_field();
                match field {
                    crate::ui::views::form::FormField::Kind
                    | crate::ui::views::form::FormField::Recurring => {
                        form.toggle_field();
                    }
                    crate::ui::views::form::FormField::Tag
                    | crate::ui::views::form::FormField::Interval => {
                        form.cycle_option();
                    }
                    _ => {
                        form.type_char(' ');
                    }
                }
            }
            KeyCode::Enter => {
                self.save_form();
            }
            KeyCode::Backspace => {
                form.backspace();
            }
            KeyCode::Char(c) => {
                form.type_char(c);
            }
            _ => {}
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(action) = self.pending_action.take() {
                    self.execute_pending_action(action);
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.pending_action = None;
                self.mode = Mode::Normal;
                self.set_status("Cancelled.");
            }
            _ => {}
        }
    }

    fn handle_filter_key(&mut self, key: KeyEvent) {
        let input = if let Mode::Filtering(ref s) = self.mode {
            s.clone()
        } else {
            return;
        };

        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                if input.trim().is_empty() {
                    self.filter.search = None;
                } else {
                    self.filter.search = Some(input.trim().to_string());
                }
                self.mode = Mode::Normal;
                if let Err(e) = self.reload_transactions() {
                    self.set_status(e.user_message());
                }
            }
            KeyCode::Backspace => {
                let mut s = input;
                s.pop();
                self.mode = Mode::Filtering(s);
            }
            KeyCode::Char(c) => {
                let mut s = input;
                s.push(c);
                self.mode = Mode::Filtering(s);
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Actions
    // -----------------------------------------------------------------------

    fn save_form(&mut self) {
        let Some(ref mut form) = self.form else {
            return;
        };

        let interval = form.get_interval();
        let is_editing = form.editing_id.is_some();

        match form.to_transaction() {
            Ok(tx) => {
                let tx_repo = TransactionRepo::new(&self.db);
                let result = if is_editing {
                    tx_repo.update(&tx).map(|_| tx.id.unwrap_or(0))
                } else {
                    tx_repo.create(&tx)
                };

                match result {
                    Ok(_id) => {
                        // If recurring was enabled and this is a new transaction,
                        // also create a recurring entry.
                        if !is_editing {
                            if let Some(interval) = interval {
                                let entry = RecurringEntry {
                                    id: None,
                                    source: tx.source.clone(),
                                    amount: tx.amount,
                                    kind: tx.kind,
                                    tag_id: tx.tag_id,
                                    interval,
                                    start_date: tx.date,
                                    last_inserted_date: Some(tx.date),
                                    active: true,
                                };
                                let rec_repo = RecurringRepo::new(&self.db);
                                if let Err(e) = rec_repo.create(&entry) {
                                    self.set_status(format!(
                                        "Transaction saved but recurring failed: {}",
                                        e.user_message()
                                    ));
                                }
                            }
                        }

                        self.form = None;
                        self.mode = Mode::Normal;
                        if let Err(e) = self.reload_all() {
                            self.set_status(e.user_message());
                        } else {
                            let action = if is_editing { "updated" } else { "added" };
                            self.set_status(format!("Transaction {}.", action));
                        }
                    }
                    Err(e) => {
                        self.set_status(e.user_message());
                    }
                }
            }
            Err(_errors) => {
                // Errors are displayed in the form itself.
            }
        }
    }

    fn execute_pending_action(&mut self, action: PendingAction) {
        match action {
            PendingAction::DeleteTransaction(id) => {
                let repo = TransactionRepo::new(&self.db);
                match repo.delete(id) {
                    Ok(()) => {
                        if let Err(e) = self.reload_all() {
                            self.set_status(e.user_message());
                        } else {
                            self.set_status("Transaction deleted.");
                        }
                    }
                    Err(e) => self.set_status(e.user_message()),
                }
            }
            PendingAction::DeleteRecurring(id) => {
                let repo = RecurringRepo::new(&self.db);
                match repo.delete(id) {
                    Ok(()) => {
                        if let Err(e) = self.reload_recurring() {
                            self.set_status(e.user_message());
                        } else {
                            self.set_status("Recurring entry deleted.");
                        }
                    }
                    Err(e) => self.set_status(e.user_message()),
                }
            }
            PendingAction::DeleteBudget(id) => {
                let repo = BudgetRepo::new(&self.db);
                match repo.delete(id) {
                    Ok(()) => {
                        if let Err(e) = self
                            .reload_budgets()
                            .and_then(|_| self.reload_budget_spending())
                        {
                            self.set_status(e.user_message());
                        } else {
                            self.set_status("Budget deleted.");
                        }
                    }
                    Err(e) => self.set_status(e.user_message()),
                }
            }
        }
    }

    /// Process recurring entries: insert transactions for any that are due.
    pub fn process_recurring(&mut self) -> Result<()> {
        let today = chrono::Local::now().date_naive();
        let rec_repo = RecurringRepo::new(&self.db);
        let tx_repo = TransactionRepo::new(&self.db);

        let active_entries = rec_repo.get_active()?;
        let mut count = 0u32;

        for entry in &active_entries {
            let last = entry.last_inserted_date.unwrap_or(entry.start_date);
            let next_due = next_date(last, entry.interval);

            if next_due <= today {
                // Insert the transaction.
                let tx = Transaction {
                    id: None,
                    source: entry.source.clone(),
                    amount: entry.amount,
                    kind: entry.kind,
                    tag_id: entry.tag_id,
                    date: next_due,
                    notes: Some(format!("Auto: recurring {}", entry.interval)),
                    created_at: None,
                    updated_at: None,
                };
                tx_repo.create(&tx)?;

                // Update last_inserted_date.
                if let Some(id) = entry.id {
                    rec_repo.update_last_inserted(id, next_due)?;
                }
                count += 1;
            }
        }

        if count > 0 {
            self.reload_all()?;
            self.set_status(format!("{count} recurring transaction(s) inserted."));
        }

        Ok(())
    }
}

/// Calculate the next date after `from` according to the given interval.
fn next_date(from: chrono::NaiveDate, interval: RecurringInterval) -> chrono::NaiveDate {
    match interval {
        RecurringInterval::Daily => from + chrono::Duration::days(1),
        RecurringInterval::Weekly => from + chrono::Duration::weeks(1),
        RecurringInterval::Monthly => {
            let month = from.month();
            let year = from.year();
            if month == 12 {
                chrono::NaiveDate::from_ymd_opt(year + 1, 1, from.day().min(28))
                    .unwrap_or(from + chrono::Duration::days(30))
            } else {
                chrono::NaiveDate::from_ymd_opt(year, month + 1, from.day().min(28))
                    .unwrap_or(from + chrono::Duration::days(30))
            }
        }
        RecurringInterval::Yearly => {
            chrono::NaiveDate::from_ymd_opt(from.year() + 1, from.month(), from.day().min(28))
                .unwrap_or(from + chrono::Duration::days(365))
        }
    }
}
