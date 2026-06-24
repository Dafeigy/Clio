use std::io::{self, IsTerminal};

/// Prints key-value pairs to stdout, handling binary data gracefully.
///
/// When stdout is a terminal and `show_binary` is false, binary values
/// are replaced with "(omitted binary data)". When piped, all data is
/// passed through raw.
pub fn print_kv(key: &[u8], value: &[u8], delimiter: &str, show_binary: bool, max_value_width: usize) {
    if show_binary || !io::stdout().is_terminal() {
        // When piped or --show-binary: pass everything through, never truncate
        println!(
            "{}{delimiter}{}",
            String::from_utf8_lossy(key),
            String::from_utf8_lossy(value)
        );
    } else {
        let key_str = safe_string(key);
        let val_str = safe_string(value);
        println!(
            "{}{delimiter}{}",
            key_str,
            truncate_value(&val_str, max_value_width)
        );
    }
}

/// Truncate a value string to `max_width` visible characters, appending "..."
/// when the string is longer.  A width of 0 means no truncation.
fn truncate_value(s: &str, max_width: usize) -> std::borrow::Cow<'_, str> {
    if max_width == 0 {
        return std::borrow::Cow::Borrowed(s);
    }
    // Count chars (not bytes) to stay under the limit
    if s.chars().count() <= max_width {
        return std::borrow::Cow::Borrowed(s);
    }
    let truncated: String = s.chars().take(max_width).chain("...".chars()).collect();
    std::borrow::Cow::Owned(truncated)
}

/// Prints a single value to stdout.
pub fn print_value(data: &[u8]) {
    let stdout = io::stdout();
    if !stdout.is_terminal() {
        // When piped, output raw bytes
        use std::io::Write;
        let mut handle = stdout.lock();
        let _ = handle.write_all(data);
    } else {
        println!("{}", safe_string(data));
    }
}

/// Prints a key only.
pub fn print_key(key: &[u8]) {
    println!("{}", safe_string(key));
}

/// Returns a display-safe string: if valid UTF-8, return as-is;
/// otherwise return "(omitted binary data)".
fn safe_string(data: &[u8]) -> String {
    match std::str::from_utf8(data) {
        Ok(s) => s.to_string(),
        Err(_) => "(omitted binary data)".to_string(),
    }
}
