use std::{cmp::Ordering, collections::HashMap};

#[derive(Clone, Debug)]
struct KeyRange {
    min: f64,
    max: f64,
    count: usize,
}

impl KeyRange {
    fn new(value: f64) -> Self {
        Self {
            min: value,
            max: value,
            count: 1,
        }
    }

    fn update(&mut self, value: f64) {
        if value < self.min {
            self.min = value;
        }
        if value > self.max {
            self.max = value;
        }
        self.count += 1;
    }

    fn range(&self) -> f64 {
        self.max - self.min
    }
}

#[derive(Clone, Debug)]
struct SortItem(HashMap<String, f64>);

impl SortItem {
    fn compare(&self, other: &Self, keys: &[String]) -> Ordering {
        for key in keys {
            match (self.0.get(key), other.0.get(key)) {
                (None, None) => continue,
                (None, Some(_)) => return Ordering::Greater,
                (Some(_), None) => return Ordering::Less,
                (Some(left), Some(right)) => {
                    let order = left.total_cmp(right);
                    if order != Ordering::Equal {
                        return order;
                    }
                }
            }
        }
        Ordering::Equal
    }
}

/// Reproduces Mango's ChapterSorter for the default entry order.
pub(crate) struct ChapterSorter {
    keys: Vec<String>,
    items: HashMap<String, SortItem>,
}

impl ChapterSorter {
    pub(crate) fn new(names: &[&str]) -> Self {
        let mut ranges: Vec<(String, KeyRange)> = Vec::new();
        for name in names {
            for (key, value) in scan(name) {
                if let Some((_, range)) = ranges.iter_mut().find(|(existing, _)| *existing == key) {
                    range.update(value);
                } else {
                    ranges.push((key, KeyRange::new(value)));
                }
            }
        }
        let items: HashMap<String, SortItem> = names
            .iter()
            .map(|name| ((*name).to_owned(), parse_item(name)))
            .collect();
        let mut keys: Vec<(String, KeyRange)> = ranges
            .into_iter()
            .filter(|(_, range)| range.count >= names.len() / 2)
            .collect();
        keys.sort_by(|(_, left), (_, right)| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| right.range().total_cmp(&left.range()))
        });
        Self {
            keys: keys.into_iter().map(|(key, _)| key).collect(),
            items,
        }
    }

    pub(crate) fn compare(&self, left: &str, right: &str) -> Ordering {
        self.items
            .get(left)
            .expect("chapter sorter compares only its input names")
            .compare(
                self.items
                    .get(right)
                    .expect("chapter sorter compares only its input names"),
                &self.keys,
            )
    }
}
pub(crate) fn compare_numerically(left: &str, right: &str) -> Ordering {
    let mut left_cursor = 0;
    let mut right_cursor = 0;
    loop {
        match (next_part(left, left_cursor), next_part(right, right_cursor)) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (
                Some((left_end, left_part, left_numeric)),
                Some((right_end, right_part, right_numeric)),
            ) => {
                let order = if left_numeric && right_numeric {
                    compare_integer_strings(left_part, right_part)
                } else {
                    left_part.cmp(right_part)
                };
                if order != Ordering::Equal {
                    return order;
                }
                left_cursor = left_end;
                right_cursor = right_end;
            }
        }
    }
}

fn next_part(value: &str, start: usize) -> Option<(usize, &str, bool)> {
    let mut cursor = start;
    while cursor < value.len() {
        let character = value[cursor..].chars().next()?;
        if matches!(character, '\n' | '\r') {
            cursor += character.len_utf8();
        } else {
            break;
        }
    }
    if cursor == value.len() {
        return None;
    }
    let start = cursor;
    let first = value[cursor..].chars().next()?;
    let numeric = first.is_ascii_digit();
    cursor += first.len_utf8();
    while cursor < value.len() {
        let character = value[cursor..].chars().next()?;
        if matches!(character, '\n' | '\r') || character.is_ascii_digit() != numeric {
            break;
        }
        cursor += character.len_utf8();
    }
    Some((cursor, &value[start..cursor], numeric))
}

fn compare_integer_strings(left: &str, right: &str) -> Ordering {
    let left = left.trim_start_matches('0');
    let right = right.trim_start_matches('0');
    let left = if left.is_empty() { "0" } else { left };
    let right = if right.is_empty() { "0" } else { right };
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

fn scan(name: &str) -> Vec<(String, f64)> {
    let bytes = name.as_bytes();
    let mut matches = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let mut found = None;
        for (start, _) in name.char_indices().filter(|(index, _)| *index >= cursor) {
            let mut key_end = start;
            while key_end < bytes.len() && !is_key_delimiter(bytes[key_end]) {
                key_end += 1;
            }
            let mut number_start = key_end;
            while number_start < bytes.len() && bytes[number_start] == b' ' {
                number_start += 1;
            }
            if let Some((value, end)) = parse_number(bytes, number_start) {
                found = Some((name[start..key_end].to_owned(), value, end));
                break;
            }
        }
        if let Some((key, value, end)) = found {
            matches.push((key, value));
            cursor = end;
        } else {
            break;
        }
    }
    matches
}

fn is_key_delimiter(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b' ' | b'\n' | b'\r')
}

fn parse_number(bytes: &[u8], start: usize) -> Option<(f64, usize)> {
    let mut integer_end = start;
    while integer_end < bytes.len() && bytes[integer_end].is_ascii_digit() {
        integer_end += 1;
    }
    let mut dot_end = integer_end;
    while dot_end < bytes.len() && bytes[dot_end] == b'.' {
        dot_end += 1;
    }
    let mut fraction_end = dot_end;
    while fraction_end < bytes.len() && bytes[fraction_end].is_ascii_digit() {
        fraction_end += 1;
    }
    let end = if fraction_end > dot_end {
        if dot_end - integer_end > 1 {
            return None;
        }
        fraction_end
    } else if integer_end > start {
        integer_end
    } else {
        return None;
    };
    let value = std::str::from_utf8(&bytes[start..end])
        .ok()?
        .parse::<f64>()
        .ok()?;
    Some(((value * 100.0).round() / 100.0, end))
}

fn parse_item(name: &str) -> SortItem {
    let mut items = HashMap::new();
    for (key, value) in scan(name) {
        items.insert(key, value);
    }
    SortItem(items)
}
