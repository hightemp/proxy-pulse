use crate::{
    checker::{self, Control},
    model::{AppError, AppResult, CheckResult, CheckSettings, Protocol, Proxy, Status},
    parser::{self, ImportOptions, ParsedImport, ParsedRow, MAX_ROWS},
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    thread,
};

#[derive(Clone)]
pub struct Entry {
    pub id: u64,
    pub version: u64,
    pub parsed: ParsedRow,
    pub status: Status,
    pub result: Option<CheckResult>,
    pub revision: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowView {
    pub id: u64,
    pub address: String,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub has_credentials: bool,
    pub requested_protocol: Protocol,
    pub protocol: Protocol,
    pub status: Status,
    pub label: String,
    pub source: String,
    pub line: usize,
    pub error: Option<AppError>,
    pub result: Option<CheckResult>,
}

impl Entry {
    pub fn view(&self) -> RowView {
        RowView {
            id: self.id,
            address: self
                .parsed
                .proxy
                .as_ref()
                .map_or_else(|| "Invalid record".into(), Proxy::address),
            host: self
                .parsed
                .proxy
                .as_ref()
                .map_or_else(String::new, |p| p.host.clone()),
            port: self.parsed.proxy.as_ref().map(|p| p.port),
            username: self
                .parsed
                .proxy
                .as_ref()
                .and_then(|p| p.credentials.as_ref())
                .map_or_else(String::new, |c| c.username.clone()),
            has_credentials: self
                .parsed
                .proxy
                .as_ref()
                .is_some_and(|p| p.credentials.is_some()),
            requested_protocol: self
                .parsed
                .proxy
                .as_ref()
                .map_or(Protocol::Auto, |p| p.protocol),
            protocol: self
                .result
                .as_ref()
                .and_then(|r| r.detected)
                .or_else(|| self.parsed.proxy.as_ref().map(|p| p.protocol))
                .unwrap_or(Protocol::Auto),
            status: self.status,
            label: self.parsed.label.clone(),
            source: self.parsed.source.clone(),
            line: self.parsed.line,
            error: self.parsed.error.clone(),
            result: self.result.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub rows: Vec<RowView>,
    pub valid: usize,
    pub invalid: usize,
    pub duplicates: usize,
    pub ignored: usize,
    pub total: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: u64,
    pub reset: bool,
    pub rows: Vec<RowView>,
    pub running: bool,
    pub run_id: u64,
    pub scheduled: usize,
    pub completed: usize,
    pub total: usize,
    pub counts: HashMap<String, usize>,
}

#[derive(Default)]
pub struct Session {
    pub entries: Vec<Entry>,
    pub revision: u64,
    next_id: u64,
    reset_revision: u64,
    pending: Option<ParsedImport>,
    pub running: bool,
    pub run_id: u64,
    pub scheduled: usize,
    pub completed: usize,
    pub control: Option<Arc<Control>>,
}

impl Session {
    pub fn ensure_idle(&self) -> AppResult<()> {
        if self.running {
            Err(AppError::new(
                "CHECK_RUNNING",
                "Stop the current check before changing the list.",
            ))
        } else {
            Ok(())
        }
    }
    pub fn preview(&mut self, text: &str, options: &ImportOptions) -> AppResult<Preview> {
        self.ensure_idle()?;
        let parsed = parser::parse_import(text, options)?;
        let mut keys: HashSet<&Proxy> = self
            .entries
            .iter()
            .filter_map(|entry| entry.parsed.proxy.as_ref())
            .collect();
        let mut duplicates = 0;
        let mut valid = 0;
        for row in &parsed.rows {
            if let Some(proxy) = &row.proxy {
                valid += 1;
                if !keys.insert(proxy) {
                    duplicates += 1;
                }
            }
        }
        let views = parsed
            .rows
            .iter()
            .take(200)
            .enumerate()
            .map(|(i, row)| {
                Entry {
                    id: i as u64,
                    version: 0,
                    parsed: row.clone(),
                    status: if row.error.is_some() {
                        Status::Invalid
                    } else {
                        Status::Unchecked
                    },
                    result: None,
                    revision: 0,
                }
                .view()
            })
            .collect();
        let result = Preview {
            rows: views,
            valid,
            invalid: parsed.rows.len() - valid,
            duplicates,
            ignored: parsed.ignored,
            total: parsed.rows.len(),
        };
        self.pending = Some(parsed);
        Ok(result)
    }
    pub fn commit_import(
        &mut self,
        replace: bool,
        keep_duplicates: bool,
        include_invalid: bool,
    ) -> AppResult<usize> {
        self.ensure_idle()?;
        let pending = self.pending.as_ref().ok_or_else(|| {
            AppError::new("NO_PREVIEW", "Preview the import before adding records.")
        })?;
        let mut keys: HashSet<Proxy> = if replace {
            HashSet::new()
        } else {
            self.entries
                .iter()
                .filter_map(|e| e.parsed.proxy.clone())
                .collect()
        };
        let accepted: Vec<_> = pending
            .rows
            .iter()
            .filter(|r| r.proxy.is_some() || include_invalid)
            .filter(|r| keep_duplicates || r.proxy.as_ref().is_none_or(|p| keys.insert(p.clone())))
            .cloned()
            .collect();
        let total = if replace {
            accepted.len()
        } else {
            accepted.len() + self.entries.len()
        };
        if total > MAX_ROWS {
            return Err(AppError::new(
                "TOO_MANY_ROWS",
                "The list cannot exceed 100,000 records.",
            ));
        }
        if replace {
            self.entries.clear();
            self.revision += 1;
            self.reset_revision = self.revision;
        }
        let added = accepted.len();
        for parsed in accepted {
            self.next_id += 1;
            self.revision += 1;
            self.entries.push(Entry {
                id: self.next_id,
                version: 1,
                status: if parsed.error.is_some() {
                    Status::Invalid
                } else {
                    Status::Unchecked
                },
                parsed,
                result: None,
                revision: self.revision,
            });
        }
        // Rejected records remain in the preview until the next import.
        Ok(added)
    }
    pub fn clear(&mut self, ids: &[u64]) -> AppResult<()> {
        self.ensure_idle()?;
        let ids: HashSet<_> = ids.iter().collect();
        self.entries
            .retain(|e| !ids.is_empty() && !ids.contains(&e.id));
        self.revision += 1;
        self.reset_revision = self.revision;
        self.pending = None;
        Ok(())
    }
    pub fn edit(&mut self, id: u64, text: &str) -> AppResult<()> {
        self.ensure_idle()?;
        let proxy = parser::parse_line(text, false)?;
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| AppError::new("NOT_FOUND", "Record no longer exists."))?;
        self.revision += 1;
        entry.parsed.proxy = Some(proxy);
        entry.parsed.error = None;
        entry.parsed.raw = text.into();
        entry.parsed.header = None;
        entry.parsed.delimiter = None;
        entry.result = None;
        entry.status = Status::Unchecked;
        entry.version += 1;
        entry.revision = self.revision;
        self.pending = None;
        Ok(())
    }
    pub fn snapshot(&self, since: u64) -> Snapshot {
        let reset = since == 0 || since < self.reset_revision;
        let mut counts = HashMap::new();
        for entry in &self.entries {
            *counts.entry(format!("{:?}", entry.status)).or_default() += 1;
        }
        Snapshot {
            revision: self.revision,
            reset,
            rows: self
                .entries
                .iter()
                .filter(|e| reset || e.revision > since)
                .map(Entry::view)
                .collect(),
            running: self.running,
            run_id: self.run_id,
            scheduled: self.scheduled,
            completed: self.completed,
            total: self.entries.len(),
            counts,
        }
    }
}

pub type SharedSession = Arc<Mutex<Session>>;

pub fn lock(session: &SharedSession) -> AppResult<std::sync::MutexGuard<'_, Session>> {
    session.lock().map_err(|_| {
        AppError::new(
            "INTERNAL_ERROR",
            "Application state is unavailable. Restart the app.",
        )
    })
}

pub fn start(
    session: SharedSession,
    ids: Vec<u64>,
    settings: CheckSettings,
    detect_again: bool,
) -> AppResult<u64> {
    settings.validate()?;
    let mut state = lock(&session)?;
    state.ensure_idle()?;
    let selected: HashSet<_> = ids.into_iter().collect();
    let mut jobs = VecDeque::new();
    let mut revision = state.revision;
    for (index, entry) in state
        .entries
        .iter_mut()
        .enumerate()
        .filter(|(_, e)| selected.contains(&e.id))
    {
        if let Some(proxy) = &mut entry.parsed.proxy {
            if detect_again {
                proxy.protocol = Protocol::Auto;
                entry.version += 1;
            }
            let preferred = entry
                .result
                .as_ref()
                .filter(|r| r.status == Status::Working)
                .and_then(|r| r.detected);
            jobs.push_back((index, entry.id, entry.version, proxy.clone(), preferred));
            revision += 1;
            entry.revision = revision;
            entry.status = Status::Queued;
            entry.result = None;
        }
    }
    if jobs.is_empty() {
        return Err(AppError::new(
            "EMPTY_SELECTION",
            "Select at least one valid proxy to check.",
        ));
    }
    state.revision = revision;
    state.run_id += 1;
    let run_id = state.run_id;
    state.scheduled = jobs.len();
    state.completed = 0;
    state.running = true;
    let control = Arc::new(Control::default());
    state.control = Some(Arc::clone(&control));
    let count = settings.concurrency.min(jobs.len());
    drop(state);
    let queue = Arc::new(Mutex::new(jobs));
    thread::spawn(move || {
        thread::scope(|scope| {
            for _ in 0..count {
                let session = Arc::clone(&session);
                let queue = Arc::clone(&queue);
                let control = Arc::clone(&control);
                let settings = &settings;
                scope.spawn(move || loop {
                    if control.is_cancelled() {
                        break;
                    }
                    let job = queue.lock().ok().and_then(|mut queue| queue.pop_front());
                    let Some((index, id, version, proxy, preferred)) = job else {
                        break;
                    };
                    if let Ok(mut state) = session.lock() {
                        if state.run_id != run_id {
                            break;
                        }
                        state.revision += 1;
                        let revision = state.revision;
                        if let Some(entry) = state.entries.get_mut(index) {
                            entry.status = Status::Checking;
                            entry.revision = revision;
                        }
                    }
                    let result = checker::check(&proxy, settings, &control, preferred);
                    if let Ok(mut state) = session.lock() {
                        if state.run_id != run_id {
                            break;
                        }
                        state.revision += 1;
                        let revision = state.revision;
                        if let Some(entry) = state
                            .entries
                            .get_mut(index)
                            .filter(|e| e.id == id && e.version == version)
                        {
                            entry.status = result.status;
                            entry.result = Some(result);
                            entry.revision = revision;
                            state.completed += 1;
                        }
                    }
                });
            }
        });
        if let Ok(mut state) = session.lock() {
            if state.run_id == run_id {
                let mut revision = state.revision;
                for entry in &mut state.entries {
                    if matches!(entry.status, Status::Checking | Status::Queued) {
                        revision += 1;
                        entry.status = Status::Cancelled;
                        entry.revision = revision;
                    }
                }
                state.revision = revision;
                state.running = false;
                state.control = None;
            }
        }
    });
    Ok(run_id)
}
