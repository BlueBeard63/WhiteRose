//! What kind of file a path is: a name for the status bar, and the KDE
//! syntax-highlighting definition to colour it with ("" for plain text).

pub fn detect(path: &str) -> (&'static str, &'static str) {
    let name = path.rsplit('/').next().unwrap_or(path);
    let lower = name.to_lowercase();
    match lower.as_str() {
        "makefile" | "gnumakefile" => return ("Makefile", "Makefile"),
        "dockerfile" | "containerfile" => return ("Dockerfile", "Dockerfile"),
        "cmakelists.txt" => return ("CMake", "CMake"),
        ".zshrc" | ".zprofile" | ".bashrc" | ".profile" | ".bash_profile" => {
            return ("Shell", "Bash")
        }
        _ => {}
    }
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "rs" => ("Rust", "Rust"),
        "go" => ("Go", "Go"),
        "py" => ("Python", "Python"),
        "js" | "mjs" | "cjs" => ("JavaScript", "JavaScript"),
        "ts" => ("TypeScript", "TypeScript"),
        "qml" => ("QML", "QML"),
        "c" | "h" => ("C", "C"),
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => ("C++", "C++"),
        "java" => ("Java", "Java"),
        "sh" | "bash" | "zsh" | "zsh-theme" => ("Shell", "Bash"),
        "md" | "markdown" => ("Markdown", "Markdown"),
        "json" => ("JSON", "JSON"),
        "yaml" | "yml" => ("YAML", "YAML"),
        "toml" => ("TOML", "TOML"),
        "ini" | "conf" | "cfg" | "desktop" | "service" => ("INI", "INI Files"),
        "xml" | "svg" | "policy" => ("XML", "XML"),
        "html" | "htm" => ("HTML", "HTML"),
        "css" => ("CSS", "CSS"),
        "sql" => ("SQL", "SQL"),
        "diff" | "patch" => ("Diff", "Diff"),
        "lua" => ("Lua", "Lua"),
        "kt" | "kts" => ("Kotlin", "Kotlin"),
        "cs" => ("C#", "C#"),
        "rb" => ("Ruby", "Ruby"),
        "php" => ("PHP", "PHP/PHP"),
        _ => ("Plain text", ""),
    }
}

#[cfg(test)]
mod tests {
    use super::detect;

    #[test]
    fn by_name_and_extension() {
        assert_eq!(detect("/home/me/src/main.rs"), ("Rust", "Rust"));
        assert_eq!(detect("/x/Makefile"), ("Makefile", "Makefile"));
        assert_eq!(detect("/x/.zshrc"), ("Shell", "Bash"));
        assert_eq!(detect("/x/custom.zsh-theme"), ("Shell", "Bash"));
        assert_eq!(detect("/x/NOTES.MD"), ("Markdown", "Markdown"));
        assert_eq!(detect("/x/readme"), ("Plain text", ""));
        assert_eq!(detect("/x/archive.tar.gz"), ("Plain text", ""));
    }
}
