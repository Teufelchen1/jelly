use std::ops::Deref;
use std::ops::DerefMut;
use std::slice;
#[cfg(not(test))]
use std::time::SystemTime;

use chrono::prelude::DateTime;
use chrono::prelude::Utc;

#[cfg(test)]
use crate::tests::SystemTime;

pub struct LogEntry<T> {
    pub time: SystemTime,
    pub data: T,
}

impl<T> LogEntry<T> {
    pub fn new(data: T) -> Self {
        let time = SystemTime::now();

        Self { time, data }
    }

    pub fn get_timestamp(&self) -> String {
        let dt: DateTime<Utc> = self.time.into();
        format!("[{}]", dt.format("%H:%M:%S%.3f"))
    }
}

pub struct Log<T> {
    entries: Vec<LogEntry<T>>,
}

impl<T> Default for Log<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Log<T> {
    pub const fn new() -> Self {
        Self { entries: vec![] }
    }

    pub fn add_new(&mut self, data: T) {
        self.entries.push(LogEntry::new(data));
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn last_mut(&mut self) -> Option<&mut LogEntry<T>> {
        self.entries.last_mut()
    }
}

impl<'a, T> IntoIterator for &'a Log<T> {
    type Item = &'a LogEntry<T>;
    type IntoIter = slice::Iter<'a, LogEntry<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<T> Deref for Log<T> {
    type Target = [LogEntry<T>];
    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl<T> DerefMut for Log<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entries
    }
}
