//! A route's query string, read as Python's `parse_qs` reads it (the Python atm's server, whose answers these keep).

/// Percent-decoding of a query's part, `+` as a space.
pub(super) fn decode(text: &str) -> String {
    let hex = |b: u8| {
        char::from(b)
            .to_digit(16)
            .and_then(|d| u8::try_from(d).ok())
    };
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let pair = (
            bytes.get(i + 1).copied().and_then(hex),
            bytes.get(i + 2).copied().and_then(hex),
        );
        match (bytes[i], pair) {
            (b'+', _) => out.push(b' '),
            (b'%', (Some(high), Some(low))) => {
                out.push(high * 16 + low);
                i += 2;
            }
            (b, _) => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A query string's parameters: each name's values in order, blank ones left out.
pub(super) fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter_map(|part| {
            let (name, value) = part.split_once('=')?;
            let value = decode(value);
            (!value.is_empty()).then(|| (decode(name), value))
        })
        .collect()
}

/// The first value of the query parameter `name`, or an empty text.
pub(super) fn parameter(query: &str, name: &str) -> String {
    parse_query(query)
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_is_read_as_parse_qs_reads_it() {
        assert_eq!(
            parse_query("status=todo&status=blocked&q=a+b%21&empty=&x"),
            [
                ("status".to_string(), "todo".to_string()),
                ("status".into(), "blocked".into()),
                ("q".into(), "a b!".into())
            ]
        );
        assert_eq!(parameter("q=one&q=two", "q"), "one");
        assert_eq!(decode("%zz%4"), "%zz%4");
    }
}
