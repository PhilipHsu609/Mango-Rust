use std::{cmp::Ordering, collections::HashMap};

#[derive(Clone, Debug)]
struct Decimal {
    digits: Vec<u8>,
    scale: usize,
}

impl Decimal {
    fn parse(integer: &str, fraction: &str) -> Option<Self> {
        let mut digits: Vec<u8> = integer
            .bytes()
            .chain(fraction.bytes())
            .map(|digit| digit - b'0')
            .collect();
        let mut scale = fraction.len();
        if let Some(first_nonzero) = digits.iter().position(|digit| *digit != 0) {
            digits.drain(..first_nonzero);
            while scale > 0 && digits.last() == Some(&0) {
                digits.pop();
                scale -= 1;
            }
        } else {
            digits.clear();
            scale = 0;
        }
        Some(Self { digits, scale })
    }

    fn aligned_digits(&self, scale: usize) -> Vec<u8> {
        let mut digits = self.digits.clone();
        digits.resize(digits.len() + scale - self.scale, 0);
        digits
    }

    fn integer_digits(&self) -> usize {
        self.digits.len().saturating_sub(self.scale)
    }

    fn subtract(&self, other: &Self) -> Self {
        let scale = self.scale.max(other.scale);
        let left = self.aligned_digits(scale);
        let right = other.aligned_digits(scale);
        let mut result = vec![0; left.len().max(right.len())];
        let mut borrow = 0i16;
        for offset in 0..result.len() {
            let left_digit = left
                .len()
                .checked_sub(offset + 1)
                .map_or(0, |index| left[index] as i16);
            let right_digit = right
                .len()
                .checked_sub(offset + 1)
                .map_or(0, |index| right[index] as i16);
            let mut digit = left_digit - right_digit - borrow;
            if digit < 0 {
                digit += 10;
                borrow = 1;
            } else {
                borrow = 0;
            }
            let index = result.len() - offset - 1;
            result[index] = digit as u8;
        }
        let mut result_scale = scale;
        if let Some(first_nonzero) = result.iter().position(|digit| *digit != 0) {
            result.drain(..first_nonzero);
        } else {
            result.clear();
            result_scale = 0;
        }
        while result_scale > 0 && result.last() == Some(&0) {
            result.pop();
            result_scale -= 1;
        }
        Self {
            digits: result,
            scale: result_scale,
        }
    }
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Decimal {}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> Ordering {
        let left_integer = self.integer_digits();
        let right_integer = other.integer_digits();
        match left_integer.cmp(&right_integer) {
            Ordering::Equal => {}
            order => return order,
        }
        let scale = self.scale.max(other.scale);
        let width = left_integer + scale;
        for index in 0..width {
            let left_offset = width - (self.digits.len() + scale - self.scale);
            let right_offset = width - (other.digits.len() + scale - other.scale);
            let left_digit = index
                .checked_sub(left_offset)
                .and_then(|digit_index| self.digits.get(digit_index))
                .copied()
                .unwrap_or(0);
            let right_digit = index
                .checked_sub(right_offset)
                .and_then(|digit_index| other.digits.get(digit_index))
                .copied()
                .unwrap_or(0);
            match left_digit.cmp(&right_digit) {
                Ordering::Equal => {}
                order => return order,
            }
        }
        Ordering::Equal
    }
}

#[derive(Clone, Debug)]
struct KeyRange {
    min: Decimal,
    max: Decimal,
    count: usize,
}

impl KeyRange {
    fn new(value: Decimal) -> Self {
        Self {
            min: value.clone(),
            max: value,
            count: 1,
        }
    }

    fn update(&mut self, value: Decimal) {
        if value < self.min {
            self.min = value.clone();
        }
        if value > self.max {
            self.max = value;
        }
        self.count += 1;
    }

    fn range(&self) -> Decimal {
        self.max.subtract(&self.min)
    }
}

#[derive(Clone, Debug)]
struct SortItem(HashMap<String, Decimal>);

impl SortItem {
    fn compare(&self, other: &Self, keys: &[String]) -> Ordering {
        for key in keys {
            match (self.0.get(key), other.0.get(key)) {
                (None, None) => continue,
                (None, Some(_)) => return Ordering::Greater,
                (Some(_), None) => return Ordering::Less,
                (Some(left), Some(right)) => {
                    let order = left.cmp(right);
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
                .then_with(|| right.range().cmp(&left.range()))
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

#[cfg(test)]
mod numeric_sort_tests {
    use super::{compare_numerically, Decimal};
    use std::cmp::Ordering;

    #[test]
    fn numeric_sort_matches_mango_integer_and_token_rules() {
        assert_eq!(
            compare_numerically("Chapter 2", "Chapter 10"),
            Ordering::Less
        );
        assert_eq!(
            compare_numerically("Chapter 01", "Chapter 1"),
            Ordering::Equal
        );
        assert_eq!(compare_numerically("Chapter", "Chapter 1"), Ordering::Less);
    }

    #[test]
    fn numeric_sort_compares_large_integer_segments_exactly() {
        assert_eq!(
            compare_numerically(
                "Chapter 999999999999999999999999",
                "Chapter 1000000000000000000000000"
            ),
            Ordering::Less
        );
    }
    #[test]
    fn decimal_comparison_preserves_leading_fractional_zeros() {
        let hundredth = Decimal::parse("0", "01").unwrap();
        let tenth = Decimal::parse("0", "1").unwrap();
        let zero = Decimal::parse("0", "").unwrap();
        assert!(zero < hundredth);
        assert!(hundredth < tenth);
    }
}

fn scan(name: &str) -> Vec<(String, Decimal)> {
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

fn parse_number(bytes: &[u8], start: usize) -> Option<(Decimal, usize)> {
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
    if fraction_end > dot_end {
        let integer = std::str::from_utf8(&bytes[start..integer_end]).ok()?;
        let fraction = std::str::from_utf8(&bytes[dot_end..fraction_end]).ok()?;
        if dot_end - integer_end > 1 {
            return None;
        }
        return Some((Decimal::parse(integer, fraction)?, fraction_end));
    }
    if integer_end > start {
        let integer = std::str::from_utf8(&bytes[start..integer_end]).ok()?;
        return Some((Decimal::parse(integer, "")?, integer_end));
    }
    None
}

fn parse_item(name: &str) -> SortItem {
    let mut items = HashMap::new();
    for (key, value) in scan(name) {
        items.insert(key, value);
    }
    SortItem(items)
}

#[cfg(test)]
mod tests {
    use super::ChapterSorter;

    #[test]
    fn matches_mango_chapter_sort_fixture() {
        let mut names = vec![
            "Vol.1 Ch.01",
            "Vol.1 Ch.02",
            "Vol.2 Ch. 2.5",
            "Ch. 3",
            "Ch.04",
        ];
        let sorter = ChapterSorter::new(&names);
        names.reverse();
        names.sort_by(|left, right| sorter.compare(left, right));
        assert_eq!(
            names,
            [
                "Vol.1 Ch.01",
                "Vol.1 Ch.02",
                "Vol.2 Ch. 2.5",
                "Ch. 3",
                "Ch.04",
            ]
        );
    }

    #[test]
    fn equal_frequency_keys_use_exact_numeric_range_order() {
        let mut names = vec![
            "Vol. 1 Ch. 1",
            "Vol. 2 Ch. 2",
            "Season 1 Episode 100",
            "Season 2 Episode 200",
        ];
        let sorter = ChapterSorter::new(&names);
        names.sort_by(|left, right| sorter.compare(left, right));
        assert_eq!(
            names,
            [
                "Season 1 Episode 100",
                "Season 2 Episode 200",
                "Vol. 1 Ch. 1",
                "Vol. 2 Ch. 2",
            ]
        );
    }
}
