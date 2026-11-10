//! JSON, JSONC and JSON5 shown formatted: comments, trailing commas,
//! unquoted keys and single quotes are all read (JSON5 is a superset of
//! the others), and it's shown as tidy JSON in the order it was written.
//! Something that doesn't parse says where.

/// {text} formatted, or why not.
pub fn pretty(text: &str) -> Result<String, String> {
    let v: serde_json::Value = json5::from_str(text).map_err(|e| {
        let s = e.to_string();
        // json5's messages carry " --> line:col" and a caret picture: the first line says it.
        s.lines()
            .next()
            .unwrap_or("not valid JSON")
            .trim()
            .to_string()
    })?;
    serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_jsonc_and_json5_in_order() {
        let jsonc = "{\n  // settings\n  \"zeta\": 1,\n  \"alpha\": [1, 2,],\n  /* block */ \"nested\": {\"b\": true, \"a\": null},\n}";
        assert_eq!(
            pretty(jsonc).unwrap(),
            "{\n  \"zeta\": 1,\n  \"alpha\": [\n    1,\n    2\n  ],\n  \"nested\": {\n    \"b\": true,\n    \"a\": null\n  }\n}"
        );
        assert_eq!(
            pretty("{unquoted: 'single', hex: 0x10}").unwrap(),
            "{\n  \"unquoted\": \"single\",\n  \"hex\": 16\n}"
        );
        assert!(pretty("{\"a\": }").is_err());
    }
}
