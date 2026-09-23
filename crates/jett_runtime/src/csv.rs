//! Strict CSV parsing and quoting shared by interpreted and native execution.

use std::collections::HashSet;

// ---------------------------------------------------------------------------
// CSV helpers
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CsvFieldState {
    Start,
    Unquoted,
    Quoted,
    AfterQuote,
}

/// Parse strict CSV records. Quotes may only open at the start of a field,
/// doubled quotes escape a quote inside a quoted field, and a closing quote
/// must be followed by a delimiter, record terminator, or end of input. Record
/// terminators are LF or CRLF; a bare CR is rejected outside quoted fields.
/// Leading and trailing whitespace in unquoted fields is data and is never
/// trimmed implicitly.
pub fn parse_csv_records(input: &str) -> Result<Vec<Vec<String>>, String> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    // Empty CSV contains no records. A physical line terminator is different:
    // it is consumed below and preserves one explicitly empty record.
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    let mut row = Vec::new();
    let mut current = String::new();
    let mut state = CsvFieldState::Start;
    let mut has_record_data = false;
    let mut record_number = 1usize;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match state {
            CsvFieldState::Start => match ch {
                '"' => {
                    has_record_data = true;
                    state = CsvFieldState::Quoted;
                }
                ',' => {
                    has_record_data = true;
                    row.push(std::mem::take(&mut current));
                }
                '\n' => {
                    push_csv_record(
                        &mut records,
                        &mut row,
                        &mut current,
                        &mut has_record_data,
                        true,
                    );
                    record_number += 1;
                }
                '\r' => {
                    if chars.peek() != Some(&'\n') {
                        return Err(csv_parse_error(
                            record_number,
                            row.len() + 1,
                            "bare carriage return; use LF or CRLF record endings",
                        ));
                    }
                    chars.next();
                    push_csv_record(
                        &mut records,
                        &mut row,
                        &mut current,
                        &mut has_record_data,
                        true,
                    );
                    record_number += 1;
                }
                _ => {
                    has_record_data = true;
                    current.push(ch);
                    state = CsvFieldState::Unquoted;
                }
            },
            CsvFieldState::Unquoted => match ch {
                '"' => {
                    return Err(csv_parse_error(
                        record_number,
                        row.len() + 1,
                        "unexpected quote in unquoted field",
                    ));
                }
                ',' => {
                    row.push(std::mem::take(&mut current));
                    state = CsvFieldState::Start;
                }
                '\n' => {
                    push_csv_record(
                        &mut records,
                        &mut row,
                        &mut current,
                        &mut has_record_data,
                        true,
                    );
                    state = CsvFieldState::Start;
                    record_number += 1;
                }
                '\r' => {
                    if chars.peek() != Some(&'\n') {
                        return Err(csv_parse_error(
                            record_number,
                            row.len() + 1,
                            "bare carriage return; use LF or CRLF record endings",
                        ));
                    }
                    chars.next();
                    push_csv_record(
                        &mut records,
                        &mut row,
                        &mut current,
                        &mut has_record_data,
                        true,
                    );
                    state = CsvFieldState::Start;
                    record_number += 1;
                }
                _ => current.push(ch),
            },
            CsvFieldState::Quoted => {
                if ch == '"' {
                    if chars.peek() == Some(&'"') {
                        current.push('"');
                        chars.next();
                    } else {
                        state = CsvFieldState::AfterQuote;
                    }
                } else {
                    current.push(ch);
                }
            }
            CsvFieldState::AfterQuote => match ch {
                ',' => {
                    row.push(std::mem::take(&mut current));
                    state = CsvFieldState::Start;
                }
                '\n' => {
                    push_csv_record(
                        &mut records,
                        &mut row,
                        &mut current,
                        &mut has_record_data,
                        true,
                    );
                    state = CsvFieldState::Start;
                    record_number += 1;
                }
                '\r' => {
                    if chars.peek() != Some(&'\n') {
                        return Err(csv_parse_error(
                            record_number,
                            row.len() + 1,
                            "bare carriage return; use LF or CRLF record endings",
                        ));
                    }
                    chars.next();
                    push_csv_record(
                        &mut records,
                        &mut row,
                        &mut current,
                        &mut has_record_data,
                        true,
                    );
                    state = CsvFieldState::Start;
                    record_number += 1;
                }
                _ => {
                    return Err(csv_parse_error(
                        record_number,
                        row.len() + 1,
                        "unexpected character after closing quote",
                    ));
                }
            },
        }
    }

    if state == CsvFieldState::Quoted {
        return Err(csv_parse_error(
            record_number,
            row.len() + 1,
            "unterminated quoted field",
        ));
    }

    // A trailing record terminator has already preserved its record and must
    // not add another empty record at end of input.
    push_csv_record(
        &mut records,
        &mut row,
        &mut current,
        &mut has_record_data,
        false,
    );
    Ok(records)
}

fn csv_parse_error(record: usize, field: usize, message: &str) -> String {
    format!("CSV parse error at record {record}, field {field}: {message}")
}

fn push_csv_record(
    records: &mut Vec<Vec<String>>,
    row: &mut Vec<String>,
    current: &mut String,
    has_record_data: &mut bool,
    preserve_empty: bool,
) {
    if preserve_empty || *has_record_data || !current.is_empty() || !row.is_empty() {
        row.push(std::mem::take(current));
        records.push(std::mem::take(row));
    } else {
        row.clear();
        current.clear();
    }
    *has_record_data = false;
}

/// Quote a CSV field if it contains commas, quotes, or newlines.
pub fn csv_quote_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

pub fn parse_csv_with_header(input: &str) -> Result<Vec<Vec<(String, String)>>, String> {
    let records = parse_csv_records(input)?;
    let mut records = records.into_iter();
    let Some(headers) = records.next() else {
        return Ok(Vec::new());
    };
    let mut seen_headers = HashSet::new();
    for (index, header) in headers.iter().enumerate() {
        if header.is_empty() {
            return Err(format!(
                "CSV header error at field {}: header must not be empty",
                index + 1
            ));
        }
        if !seen_headers.insert(header.as_str()) {
            return Err(format!(
                "CSV header error at field {}: duplicate header '{}'",
                index + 1,
                header
            ));
        }
    }
    let mut rows = Vec::new();
    for (index, cols) in records.enumerate() {
        if cols.len() != headers.len() {
            return Err(format!(
                "CSV header error at record {}: expected {} fields, got {}",
                index + 2,
                headers.len(),
                cols.len()
            ));
        }
        rows.push(headers.iter().cloned().zip(cols).collect());
    }
    Ok(rows)
}

pub fn stringify_csv_records(rows: &[Vec<String>]) -> String {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|field| csv_quote_field(field))
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
