use std::io::{self, IsTerminal};

/// Print a key-value pair with a right-aligned numeric index and key-column
/// alignment so that values line up across rows.
///
/// `index_width` is the number of digits in the largest index (used to
/// right-align indices). `max_key_width` is the display width of the
/// longest key (used to space-pad keys so values start at the same column).
pub fn print_indexed_kv(
    index: usize,
    index_width: usize,
    key: &[u8],
    max_key_width: usize,
    value: &[u8],
    delimiter: &str,
    show_binary: bool,
    max_value_width: usize,
) {
    if show_binary || !io::stdout().is_terminal() {
        // Piped or --show-binary: passthrough raw bytes, no truncation.
        // Key-column alignment still applies.
        let key_str = String::from_utf8_lossy(key);
        println!(
            "{:>iw$} {:<kw$}{delimiter}{}",
            index,
            key_str,
            String::from_utf8_lossy(value),
            iw = index_width,
            kw = max_key_width,
        );
    } else {
        let key_str = safe_string(key);
        let val_str = safe_string(value);
        println!(
            "{:>iw$} {:<kw$}{delimiter}{}",
            index,
            key_str,
            truncate_value(&val_str, max_value_width),
            iw = index_width,
            kw = max_key_width,
        );
    }
}

/// Print a key with a right-aligned index prefix.
pub fn print_indexed_key(index: usize, index_width: usize, key: &[u8]) {
    println!("{:>iw$} {}", index, safe_string(key), iw = index_width);
}

/// Print a value with a right-aligned index prefix.
pub fn print_indexed_value(index: usize, index_width: usize, value: &[u8]) {
    let stdout = io::stdout();
    if !stdout.is_terminal() {
        use std::io::Write;
        let mut handle = stdout.lock();
        let _ = write!(handle, "{:>iw$} ", index, iw = index_width);
        let _ = handle.write_all(value);
        let _ = handle.write_all(b"\n");
    } else {
        println!("{:>iw$} {}", index, safe_string(value), iw = index_width);
    }
}

/// Return the terminal display width of a key's safe representation.
/// Used to compute alignment padding in list output.
pub fn display_width(data: &[u8]) -> usize {
    safe_string(data).chars().count()
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

/// Returns a display-safe string: if valid UTF-8, return as-is;
/// otherwise return "(omitted binary data)".
fn safe_string(data: &[u8]) -> String {
    match std::str::from_utf8(data) {
        Ok(s) => s.to_string(),
        Err(_) => "(omitted binary data)".to_string(),
    }
}
