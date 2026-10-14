//! Line-local colouring of fenced code whose language is known. One pass over each
//! line; only a small state (open block comment, multi-line string, open tag) carries
//! to the next line of the same fence, so a page never needs anything outside itself.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Keyword,
    Type,
    Function,
    String,
    /// Numbers and literal constants (true, null, None…).
    Number,
    Comment,
    /// Attributes, annotations, directives and variables.
    Meta,
    /// Diff lines.
    Inserted,
    Deleted,
}

/// What an unfinished line leaves open for the next one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Normal,
    /// Inside a block comment.
    Comment,
    /// Inside a string closed by `count` copies of `quote`.
    Text { quote: u8, count: u8 },
    /// Markup: inside a tag, between its name and `>`.
    Tag,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lexer {
    Code,
    Markup,
    Diff,
}

/// `name(` is a call.
const CALLS: u16 = 1;
/// Capitalised names are types.
const CAPITALS: u16 = 1 << 1;
/// `#include`-style directives at line start (C family, C#).
const DIRECTIVES: u16 = 1 << 2;
/// `@annotation` / `@decorator`.
const AT: u16 = 1 << 3;
/// `$variable` and `${…}`.
const DOLLAR: u16 = 1 << 4;
/// `name:` and `"name":` keys.
const KEYS: u16 = 1 << 5;
/// `name =` keys and `[section]` lines (INI, TOML).
const ASSIGN: u16 = 1 << 6;
/// Dashes inside names (CSS, shells, YAML).
const DASHES: u16 = 1 << 7;
/// Keywords ignore ASCII case.
const NOCASE: u16 = 1 << 8;
/// Comment markers count only at the start of a word.
const WORDS: u16 = 1 << 9;
/// Rust: `'a` lifetimes, `name!` macros and `#[attributes]`.
const RUST: u16 = 1 << 10;

pub struct Syntax {
    names: &'static [&'static str],
    lexer: Lexer,
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    literals: &'static [&'static str],
    comments: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    quotes: &'static [u8],
    /// Quote bytes whose strings may continue on the next line.
    multiline: &'static [u8],
    /// `'''` and `"""` strings.
    triple: bool,
    flags: u16,
}

impl Syntax {
    const fn code(names: &'static [&'static str]) -> Self {
        Self {
            names,
            lexer: Lexer::Code,
            keywords: &[],
            types: &[],
            literals: &[],
            comments: &[],
            block: None,
            quotes: b"\"'",
            multiline: &[],
            triple: false,
            flags: 0,
        }
    }
    fn has(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }
}

const C_BLOCK: Option<(&str, &str)> = Some(("/*", "*/"));
const C_LITERALS: &[&str] = &["true", "false", "NULL", "nullptr"];
const C_KEYWORDS: &[&str] = &[
    "auto",
    "break",
    "case",
    "const",
    "continue",
    "default",
    "do",
    "else",
    "enum",
    "extern",
    "for",
    "goto",
    "if",
    "inline",
    "register",
    "restrict",
    "return",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "volatile",
    "while",
    "alignas",
    "alignof",
    "and",
    "asm",
    "catch",
    "class",
    "constexpr",
    "consteval",
    "constinit",
    "const_cast",
    "decltype",
    "delete",
    "dynamic_cast",
    "explicit",
    "export",
    "friend",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "not",
    "operator",
    "or",
    "private",
    "protected",
    "public",
    "reinterpret_cast",
    "static_assert",
    "static_cast",
    "template",
    "this",
    "thread_local",
    "throw",
    "try",
    "typeid",
    "typename",
    "using",
    "virtual",
    "co_await",
    "co_return",
    "co_yield",
    "concept",
    "requires",
    "override",
    "final",
];
const C_TYPES: &[&str] = &[
    "void",
    "char",
    "short",
    "int",
    "long",
    "float",
    "double",
    "signed",
    "unsigned",
    "bool",
    "_Bool",
    "wchar_t",
    "char8_t",
    "char16_t",
    "char32_t",
    "size_t",
    "ssize_t",
    "ptrdiff_t",
    "int8_t",
    "int16_t",
    "int32_t",
    "int64_t",
    "uint8_t",
    "uint16_t",
    "uint32_t",
    "uint64_t",
    "intptr_t",
    "uintptr_t",
    "FILE",
    "std",
    "string",
    "vector",
    "map",
    "unique_ptr",
    "shared_ptr",
];
const JS_KEYWORDS: &[&str] = &[
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "finally",
    "for",
    "from",
    "function",
    "get",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "of",
    "return",
    "set",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "as",
    "type",
    "interface",
    "enum",
    "implements",
    "declare",
    "namespace",
    "abstract",
    "private",
    "protected",
    "public",
    "readonly",
    "keyof",
    "satisfies",
];
const JS_TYPES: &[&str] = &[
    "string", "number", "boolean", "any", "unknown", "never", "object", "symbol", "bigint",
    "Array", "Object", "String", "Number", "Boolean", "Promise", "Map", "Set", "Date", "RegExp",
    "Error", "JSON", "Math", "console", "window", "document",
];
const JS_LITERALS: &[&str] = &["true", "false", "null", "undefined", "NaN", "Infinity"];

static LANGUAGES: &[Syntax] = &[
    Syntax {
        keywords: &[
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
            "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
            "type", "union", "unsafe", "use", "where", "while", "yield",
        ],
        types: &[
            "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32",
            "i64", "i128", "isize", "f32", "f64",
        ],
        literals: &["true", "false"],
        comments: &["//"],
        block: C_BLOCK,
        quotes: b"\"",
        multiline: b"\"",
        flags: CALLS | CAPITALS | RUST,
        ..Syntax::code(&["rust", "rs"])
    },
    Syntax {
        keywords: C_KEYWORDS,
        types: C_TYPES,
        literals: C_LITERALS,
        comments: &["//"],
        block: C_BLOCK,
        flags: CALLS | DIRECTIVES,
        ..Syntax::code(&[
            "c",
            "h",
            "cpp",
            "c++",
            "cc",
            "cxx",
            "hpp",
            "hh",
            "hxx",
            "objc",
            "objective-c",
            "arduino",
            "ino",
            "cuda",
            "cu",
        ])
    },
    Syntax {
        keywords: &[
            "abstract",
            "as",
            "base",
            "break",
            "case",
            "catch",
            "checked",
            "class",
            "const",
            "continue",
            "default",
            "delegate",
            "do",
            "else",
            "enum",
            "event",
            "explicit",
            "extern",
            "finally",
            "fixed",
            "for",
            "foreach",
            "goto",
            "if",
            "implicit",
            "in",
            "interface",
            "internal",
            "is",
            "lock",
            "namespace",
            "new",
            "operator",
            "out",
            "override",
            "params",
            "private",
            "protected",
            "public",
            "readonly",
            "ref",
            "return",
            "sealed",
            "sizeof",
            "stackalloc",
            "static",
            "struct",
            "switch",
            "this",
            "throw",
            "try",
            "typeof",
            "unchecked",
            "unsafe",
            "using",
            "virtual",
            "volatile",
            "while",
            "async",
            "await",
            "get",
            "set",
            "init",
            "value",
            "var",
            "yield",
            "record",
            "required",
            "partial",
            "where",
            "when",
        ],
        types: &[
            "bool", "byte", "char", "decimal", "double", "dynamic", "float", "int", "long",
            "object", "sbyte", "short", "string", "uint", "ulong", "ushort", "void",
        ],
        literals: &["true", "false", "null"],
        comments: &["//"],
        block: C_BLOCK,
        flags: CALLS | CAPITALS | DIRECTIVES,
        ..Syntax::code(&["csharp", "cs", "c#", "dotnet"])
    },
    Syntax {
        keywords: &[
            "abstract",
            "assert",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "default",
            "do",
            "else",
            "enum",
            "extends",
            "final",
            "finally",
            "for",
            "goto",
            "if",
            "implements",
            "import",
            "instanceof",
            "interface",
            "native",
            "new",
            "package",
            "private",
            "protected",
            "public",
            "return",
            "static",
            "strictfp",
            "super",
            "switch",
            "synchronized",
            "this",
            "throw",
            "throws",
            "transient",
            "try",
            "volatile",
            "while",
            "var",
            "record",
            "yield",
            "sealed",
            "permits",
            "non-sealed",
        ],
        types: &[
            "boolean", "byte", "char", "double", "float", "int", "long", "short", "void",
        ],
        literals: &["true", "false", "null"],
        comments: &["//"],
        block: C_BLOCK,
        flags: CALLS | CAPITALS | AT,
        ..Syntax::code(&["java", "groovy", "scala"])
    },
    Syntax {
        keywords: &[
            "as",
            "break",
            "by",
            "catch",
            "class",
            "companion",
            "continue",
            "data",
            "do",
            "else",
            "enum",
            "finally",
            "for",
            "fun",
            "if",
            "import",
            "in",
            "infix",
            "init",
            "inline",
            "interface",
            "internal",
            "is",
            "lateinit",
            "object",
            "open",
            "operator",
            "out",
            "override",
            "package",
            "private",
            "protected",
            "public",
            "return",
            "sealed",
            "super",
            "suspend",
            "this",
            "throw",
            "try",
            "typealias",
            "val",
            "var",
            "vararg",
            "when",
            "where",
            "while",
        ],
        literals: &["true", "false", "null"],
        comments: &["//"],
        block: C_BLOCK,
        quotes: b"\"'",
        triple: true,
        flags: CALLS | CAPITALS | AT | DOLLAR,
        ..Syntax::code(&["kotlin", "kt", "kts"])
    },
    Syntax {
        keywords: &[
            "actor",
            "any",
            "as",
            "associatedtype",
            "async",
            "await",
            "break",
            "case",
            "catch",
            "class",
            "continue",
            "default",
            "defer",
            "deinit",
            "do",
            "else",
            "enum",
            "extension",
            "fallthrough",
            "fileprivate",
            "final",
            "for",
            "func",
            "guard",
            "if",
            "import",
            "in",
            "init",
            "inout",
            "internal",
            "is",
            "lazy",
            "let",
            "mutating",
            "open",
            "operator",
            "override",
            "private",
            "protocol",
            "public",
            "repeat",
            "rethrows",
            "return",
            "self",
            "Self",
            "some",
            "static",
            "struct",
            "subscript",
            "super",
            "switch",
            "throw",
            "throws",
            "try",
            "typealias",
            "var",
            "weak",
            "where",
            "while",
        ],
        literals: &["true", "false", "nil"],
        comments: &["//"],
        block: C_BLOCK,
        quotes: b"\"",
        triple: true,
        flags: CALLS | CAPITALS | AT,
        ..Syntax::code(&["swift"])
    },
    Syntax {
        keywords: &[
            "break",
            "case",
            "chan",
            "const",
            "continue",
            "default",
            "defer",
            "else",
            "fallthrough",
            "for",
            "func",
            "go",
            "goto",
            "if",
            "import",
            "interface",
            "map",
            "package",
            "range",
            "return",
            "select",
            "struct",
            "switch",
            "type",
            "var",
        ],
        types: &[
            "any",
            "bool",
            "byte",
            "complex64",
            "complex128",
            "error",
            "float32",
            "float64",
            "int",
            "int8",
            "int16",
            "int32",
            "int64",
            "rune",
            "string",
            "uint",
            "uint8",
            "uint16",
            "uint32",
            "uint64",
            "uintptr",
        ],
        literals: &["true", "false", "nil", "iota"],
        comments: &["//"],
        block: C_BLOCK,
        quotes: b"\"'`",
        multiline: b"`",
        flags: CALLS,
        ..Syntax::code(&["go", "golang"])
    },
    Syntax {
        keywords: JS_KEYWORDS,
        types: JS_TYPES,
        literals: JS_LITERALS,
        comments: &["//"],
        block: C_BLOCK,
        quotes: b"\"'`",
        multiline: b"`",
        flags: CALLS | DOLLAR,
        ..Syntax::code(&[
            "javascript",
            "js",
            "jsx",
            "mjs",
            "cjs",
            "node",
            "typescript",
            "ts",
            "tsx",
            "mts",
            "cts",
            "deno",
            "es6",
            "dart",
        ])
    },
    Syntax {
        keywords: &[
            "and", "as", "assert", "async", "await", "break", "case", "class", "continue", "def",
            "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
            "in", "is", "lambda", "match", "nonlocal", "not", "or", "pass", "raise", "return",
            "try", "while", "with", "yield",
        ],
        types: &[
            "int",
            "float",
            "str",
            "bool",
            "list",
            "dict",
            "set",
            "tuple",
            "bytes",
            "object",
            "type",
            "self",
            "cls",
            "super",
            "len",
            "print",
            "range",
            "open",
            "isinstance",
            "Exception",
        ],
        literals: &["True", "False", "None"],
        comments: &["#"],
        triple: true,
        flags: CALLS | AT,
        ..Syntax::code(&["python", "py", "python3", "py3", "pyw", "ipython", "sage"])
    },
    Syntax {
        keywords: &[
            "alias",
            "and",
            "begin",
            "break",
            "case",
            "class",
            "def",
            "defined?",
            "do",
            "else",
            "elsif",
            "end",
            "ensure",
            "for",
            "if",
            "in",
            "module",
            "next",
            "not",
            "or",
            "redo",
            "rescue",
            "retry",
            "return",
            "self",
            "super",
            "then",
            "undef",
            "unless",
            "until",
            "when",
            "while",
            "yield",
            "require",
            "include",
            "extend",
            "attr_reader",
            "attr_writer",
            "attr_accessor",
            "puts",
            "raise",
        ],
        literals: &["true", "false", "nil"],
        comments: &["#"],
        flags: CALLS | CAPITALS | AT | DOLLAR,
        ..Syntax::code(&["ruby", "rb", "gemfile", "rake"])
    },
    Syntax {
        keywords: &[
            "abstract",
            "and",
            "as",
            "break",
            "case",
            "catch",
            "class",
            "clone",
            "const",
            "continue",
            "declare",
            "default",
            "do",
            "echo",
            "else",
            "elseif",
            "empty",
            "enddeclare",
            "endfor",
            "endforeach",
            "endif",
            "endswitch",
            "endwhile",
            "enum",
            "extends",
            "final",
            "finally",
            "fn",
            "for",
            "foreach",
            "function",
            "global",
            "goto",
            "if",
            "implements",
            "include",
            "include_once",
            "instanceof",
            "insteadof",
            "interface",
            "isset",
            "list",
            "match",
            "namespace",
            "new",
            "or",
            "print",
            "private",
            "protected",
            "public",
            "readonly",
            "require",
            "require_once",
            "return",
            "static",
            "switch",
            "throw",
            "trait",
            "try",
            "unset",
            "use",
            "var",
            "while",
            "xor",
            "yield",
        ],
        literals: &["true", "false", "null", "TRUE", "FALSE", "NULL"],
        comments: &["//", "#"],
        block: C_BLOCK,
        flags: CALLS | DOLLAR,
        ..Syntax::code(&["php"])
    },
    Syntax {
        keywords: &[
            "and", "break", "do", "else", "elseif", "end", "for", "function", "goto", "if", "in",
            "local", "not", "or", "repeat", "return", "then", "until", "while",
        ],
        literals: &["true", "false", "nil"],
        comments: &["--"],
        block: Some(("--[[", "]]")),
        flags: CALLS,
        ..Syntax::code(&["lua"])
    },
    Syntax {
        keywords: &[
            "select",
            "from",
            "where",
            "and",
            "or",
            "not",
            "insert",
            "into",
            "values",
            "update",
            "set",
            "delete",
            "create",
            "table",
            "drop",
            "alter",
            "add",
            "column",
            "index",
            "view",
            "primary",
            "key",
            "foreign",
            "references",
            "join",
            "inner",
            "left",
            "right",
            "outer",
            "full",
            "cross",
            "on",
            "as",
            "group",
            "by",
            "order",
            "having",
            "limit",
            "offset",
            "distinct",
            "union",
            "all",
            "case",
            "when",
            "then",
            "else",
            "end",
            "in",
            "is",
            "like",
            "between",
            "exists",
            "default",
            "constraint",
            "unique",
            "check",
            "with",
            "begin",
            "commit",
            "rollback",
            "transaction",
            "if",
            "replace",
            "returning",
            "asc",
            "desc",
            "count",
            "sum",
            "avg",
            "min",
            "max",
            "trigger",
            "procedure",
            "function",
            "return",
            "returns",
            "declare",
            "grant",
            "revoke",
            "database",
            "schema",
            "use",
            "show",
            "explain",
        ],
        types: &[
            "int",
            "integer",
            "bigint",
            "smallint",
            "tinyint",
            "decimal",
            "numeric",
            "float",
            "real",
            "double",
            "char",
            "varchar",
            "nvarchar",
            "text",
            "date",
            "time",
            "timestamp",
            "datetime",
            "boolean",
            "bool",
            "blob",
            "json",
            "jsonb",
            "uuid",
            "serial",
        ],
        literals: &["null", "true", "false"],
        comments: &["--"],
        block: C_BLOCK,
        quotes: b"'\"`",
        flags: CALLS | NOCASE,
        ..Syntax::code(&[
            "sql",
            "mysql",
            "postgres",
            "postgresql",
            "psql",
            "pgsql",
            "sqlite",
            "tsql",
            "plsql",
            "mssql",
        ])
    },
    Syntax {
        keywords: &[
            "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case",
            "esac", "in", "function", "return", "local", "export", "source", "alias", "unset",
            "readonly", "declare", "set", "exit", "break", "continue", "shift", "trap", "eval",
            "exec", "select", "time",
        ],
        types: &[
            "echo", "cd", "printf", "read", "test", "pwd", "ls", "cp", "mv", "rm", "mkdir", "cat",
            "grep", "sed", "awk", "find", "chmod", "chown", "curl", "wget", "git", "sudo", "apt",
            "npm", "cargo", "make", "docker", "pip", "python", "node", "tar",
        ],
        literals: &["true", "false"],
        comments: &["#"],
        flags: DOLLAR | DASHES | WORDS,
        ..Syntax::code(&[
            "bash",
            "sh",
            "shell",
            "zsh",
            "ksh",
            "fish",
            "shellscript",
            "console",
            "shell-session",
            "terminal",
        ])
    },
    Syntax {
        keywords: &[
            "begin",
            "break",
            "catch",
            "class",
            "continue",
            "data",
            "do",
            "dynamicparam",
            "else",
            "elseif",
            "end",
            "enum",
            "exit",
            "filter",
            "finally",
            "for",
            "foreach",
            "function",
            "if",
            "in",
            "param",
            "process",
            "return",
            "switch",
            "throw",
            "trap",
            "try",
            "until",
            "using",
            "while",
            "workflow",
        ],
        literals: &["$true", "$false", "$null"],
        comments: &["#"],
        block: Some(("<#", "#>")),
        flags: DOLLAR | DASHES | NOCASE | WORDS,
        ..Syntax::code(&["powershell", "ps1", "pwsh", "ps", "psm1"])
    },
    Syntax {
        keywords: &[
            "echo",
            "set",
            "if",
            "else",
            "for",
            "in",
            "do",
            "goto",
            "call",
            "exit",
            "not",
            "exist",
            "defined",
            "errorlevel",
            "setlocal",
            "endlocal",
            "shift",
            "pause",
            "rem",
            "cd",
            "start",
            "copy",
            "del",
            "move",
            "mkdir",
            "type",
        ],
        comments: &["::", "rem ", "REM ", "Rem ", "@rem ", "@REM "],
        quotes: b"\"",
        flags: NOCASE | WORDS,
        ..Syntax::code(&["bat", "cmd", "batch", "dos"])
    },
    Syntax {
        literals: &["true", "false", "null"],
        comments: &["//"],
        block: C_BLOCK,
        quotes: b"\"",
        flags: KEYS,
        ..Syntax::code(&["json", "jsonc", "json5", "geojson", "jsonl"])
    },
    Syntax {
        literals: &[
            "true", "false", "null", "yes", "no", "on", "off", "True", "False", "Null", "~",
        ],
        comments: &["#"],
        flags: KEYS | DASHES | WORDS,
        ..Syntax::code(&["yaml", "yml"])
    },
    Syntax {
        literals: &["true", "false", "yes", "no", "on", "off"],
        comments: &["#", ";"],
        triple: true,
        flags: ASSIGN | DASHES | WORDS,
        ..Syntax::code(&[
            "toml",
            "ini",
            "cfg",
            "conf",
            "config",
            "properties",
            "editorconfig",
            "desktop",
            "gitconfig",
            "env",
            "dotenv",
        ])
    },
    Syntax {
        keywords: &[
            "from",
            "run",
            "cmd",
            "copy",
            "add",
            "env",
            "workdir",
            "expose",
            "entrypoint",
            "arg",
            "label",
            "user",
            "volume",
            "shell",
            "healthcheck",
            "onbuild",
            "stopsignal",
            "maintainer",
            "as",
        ],
        comments: &["#"],
        flags: DOLLAR | NOCASE | WORDS,
        ..Syntax::code(&["dockerfile", "docker", "containerfile"])
    },
    Syntax {
        keywords: &["important"],
        literals: &["inherit", "initial", "unset", "none", "auto"],
        comments: &["//"],
        block: C_BLOCK,
        flags: KEYS | DASHES | AT | WORDS,
        ..Syntax::code(&["css", "scss", "sass", "less", "stylus", "postcss"])
    },
    Syntax {
        lexer: Lexer::Markup,
        block: Some(("<!--", "-->")),
        ..Syntax::code(&[
            "html", "htm", "xhtml", "xml", "svg", "vue", "svelte", "xaml", "plist", "rss", "atom",
            "xsl", "xslt", "wsdl", "csproj", "props", "targets", "resx", "jsp", "aspx", "razor",
            "cshtml",
        ])
    },
    Syntax {
        lexer: Lexer::Diff,
        ..Syntax::code(&["diff", "patch", "udiff"])
    },
];

/// The syntax named by a fence info string such as `rust`, `C++` or `{.python}`.
pub fn find(info: &str) -> Option<&'static Syntax> {
    let word = info
        .trim()
        .trim_start_matches(['{', '.'])
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | '{' | '}' | '='))
        .next()?;
    let word = word.strip_prefix("language-").unwrap_or(word);
    if word.is_empty() || word.len() > 32 {
        return None;
    }
    LANGUAGES
        .iter()
        .find(|s| s.names.iter().any(|n| n.eq_ignore_ascii_case(word)))
}

fn ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}
fn ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}
fn utf8_len(lead: u8) -> usize {
    match lead {
        0xf0.. => 4,
        0xe0.. => 3,
        0xc0.. => 2,
        _ => 1,
    }
}
fn find_bytes(b: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    (from..=b.len().saturating_sub(needle.len())).find(|&i| b[i..].starts_with(needle))
}

/// Coloured pieces covering `text`: byte ranges with their token, plain text as
/// None. Splits fall on ASCII bytes, so every range is valid UTF-8.
pub fn line(syntax: &Syntax, state: &mut State, text: &str) -> Vec<(usize, usize, Option<Token>)> {
    let mut out = Pieces(Vec::new());
    match syntax.lexer {
        Lexer::Diff => {
            let token = match text.as_bytes().first() {
                _ if text.starts_with("+++") || text.starts_with("---") => Some(Token::Meta),
                Some(b'+') | Some(b'>') => Some(Token::Inserted),
                Some(b'-') | Some(b'<') => Some(Token::Deleted),
                Some(b'@') => Some(Token::Meta),
                _ => None,
            };
            out.push(0, text.len(), token);
        }
        Lexer::Markup => markup(syntax, state, text.as_bytes(), &mut out),
        Lexer::Code => code(syntax, state, text.as_bytes(), &mut out),
    }
    out.0
}

struct Pieces(Vec<(usize, usize, Option<Token>)>);
impl Pieces {
    fn push(&mut self, from: usize, to: usize, token: Option<Token>) {
        if from >= to {
            return;
        }
        if let Some(last) = self.0.last_mut()
            && last.1 == from
            && last.2 == token
        {
            last.1 = to;
            return;
        }
        self.0.push((from, to, token));
    }
}

/// End of a string starting at `from` (just after its opening quotes), or None
/// when it runs past the line.
fn string_end(b: &[u8], from: usize, quote: u8, count: usize) -> Option<usize> {
    let mut i = from;
    while i < b.len() {
        if b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i] == quote && b[i..].iter().take(count).filter(|&&c| c == quote).count() == count {
            return Some(i + count);
        }
        i += 1;
    }
    None
}

fn code(syntax: &Syntax, state: &mut State, b: &[u8], out: &mut Pieces) {
    let mut i = 0;
    // Finish what the previous line left open.
    match *state {
        State::Comment => {
            let close = syntax.block.map_or("*/", |(_, close)| close).as_bytes();
            match find_bytes(b, 0, close) {
                Some(end) => {
                    out.push(0, end + close.len(), Some(Token::Comment));
                    i = end + close.len();
                    *state = State::Normal;
                }
                None => {
                    out.push(0, b.len(), Some(Token::Comment));
                    return;
                }
            }
        }
        State::Text { quote, count } => match string_end(b, 0, quote, count as usize) {
            Some(end) => {
                out.push(0, end, Some(Token::String));
                i = end;
                *state = State::Normal;
            }
            None => {
                out.push(0, b.len(), Some(Token::String));
                return;
            }
        },
        State::Tag | State::Normal => *state = State::Normal,
    }
    let first = b
        .iter()
        .position(|c| !c.is_ascii_whitespace())
        .unwrap_or(b.len());
    // `[section]` lines of INI / TOML.
    if syntax.has(ASSIGN) && b.get(first) == Some(&b'[') {
        out.push(i, first, None);
        let end = b
            .iter()
            .rposition(|&c| c == b']')
            .map_or(b.len(), |e| e + 1);
        out.push(first, end, Some(Token::Keyword));
        out.push(end, b.len(), None);
        return;
    }
    // A word that names what follows: `fn name`, `class Name`.
    let mut defines: Option<Token> = None;
    while i < b.len() {
        let c = b[i];
        let at_word = i == 0 || !ident(b[i - 1]) && b[i - 1] != b'$';
        if let Some((open, close)) = syntax.block
            && b[i..].starts_with(open.as_bytes())
        {
            match find_bytes(b, i + open.len(), close.as_bytes()) {
                Some(end) => {
                    out.push(i, end + close.len(), Some(Token::Comment));
                    i = end + close.len();
                }
                None => {
                    out.push(i, b.len(), Some(Token::Comment));
                    *state = State::Comment;
                    return;
                }
            }
            continue;
        }
        if syntax
            .comments
            .iter()
            .any(|m| b[i..].starts_with(m.as_bytes()))
            && (!syntax.has(WORDS) || i == 0 || b[i - 1].is_ascii_whitespace())
            // `$#` is a shell variable, not a comment.
            && !(syntax.has(DOLLAR) && c == b'#' && i > 0 && b[i - 1] == b'$')
        {
            out.push(i, b.len(), Some(Token::Comment));
            return;
        }
        // Rust: `'a'` / `'\n'` characters and `'a` lifetimes.
        if syntax.has(RUST) && c == b'\'' {
            let next = b.get(i + 1).copied().unwrap_or(0);
            let end = if next == b'\\' {
                b.get(i + 3..)
                    .and_then(|rest| rest.iter().take(10).position(|&c| c == b'\''))
                    .map(|p| i + 4 + p)
            } else if next != 0 && b.get(i + 1 + utf8_len(next)) == Some(&b'\'') {
                Some(i + 2 + utf8_len(next))
            } else {
                None
            };
            if let Some(end) = end {
                out.push(i, end.min(b.len()), Some(Token::String));
                i = end.min(b.len());
            } else if ident_start(next) {
                let end = i + 1 + b[i + 1..].iter().take_while(|&&c| ident(c)).count();
                out.push(i, end, Some(Token::Meta));
                i = end;
            } else {
                out.push(i, i + 1, None);
                i += 1;
            }
            continue;
        }
        if syntax.quotes.contains(&c) {
            let count = if syntax.triple && b[i..].starts_with(&[c, c, c]) {
                3
            } else {
                1
            };
            let end = string_end(b, i + count, c, count);
            let end_at = end.unwrap_or(b.len());
            // JSON / YAML / CSS keys: a string followed by `:`.
            let key = syntax.has(KEYS)
                && b[end_at..]
                    .iter()
                    .find(|c| !c.is_ascii_whitespace())
                    .is_some_and(|&c| c == b':');
            out.push(
                i,
                end_at,
                Some(if key { Token::Type } else { Token::String }),
            );
            if end.is_none() && (count == 3 || syntax.multiline.contains(&c)) {
                *state = State::Text {
                    quote: c,
                    count: count as u8,
                };
            }
            i = end_at;
            continue;
        }
        // Directives at line start: `#include`, `#define`, `#region`.
        if c == b'#' && i == first && syntax.has(DIRECTIVES) {
            let end = i + 1 + b[i + 1..].iter().take_while(|&&c| ident(c)).count();
            out.push(i, end, Some(Token::Meta));
            i = end;
            continue;
        }
        // Rust attributes `#[derive(..)]` and `#![..]`.
        if c == b'#' && syntax.has(RUST) && matches!(b.get(i + 1), Some(b'[') | Some(b'!')) {
            let mut depth = 0;
            let mut end = b.len();
            for (j, &c) in b.iter().enumerate().skip(i + 1) {
                match c {
                    b'[' => depth += 1,
                    b']' => {
                        depth -= 1;
                        if depth == 0 {
                            end = j + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            out.push(i, end, Some(Token::Meta));
            i = end;
            continue;
        }
        if c == b'@' && syntax.has(AT) && b.get(i + 1).is_some_and(|&n| ident_start(n)) {
            let end = i
                + 1
                + b[i + 1..]
                    .iter()
                    .take_while(|&&c| ident(c) || c == b'.' || c == b'-')
                    .count();
            out.push(i, end, Some(Token::Meta));
            i = end;
            continue;
        }
        if c == b'$' && syntax.has(DOLLAR) {
            let next = b.get(i + 1).copied().unwrap_or(0);
            if next == b'{' {
                let end = b[i..]
                    .iter()
                    .position(|&c| c == b'}')
                    .map_or(b.len(), |p| i + p + 1);
                out.push(i, end, Some(Token::Meta));
                i = end;
                continue;
            }
            if ident_start(next) || next.is_ascii_digit() || matches!(next, b'?' | b'@' | b'#') {
                let mut end = i + 2;
                if ident_start(next) {
                    end = i + 1 + b[i + 1..].iter().take_while(|&&c| ident(c)).count();
                }
                let word = std::str::from_utf8(&b[i..end]).unwrap_or("");
                let token = if syntax.literals.iter().any(|l| l.eq_ignore_ascii_case(word)) {
                    Token::Number
                } else {
                    Token::Meta
                };
                out.push(i, end, Some(token));
                i = end;
                continue;
            }
        }
        if at_word
            && (c.is_ascii_digit() || c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            let mut end = i + 1;
            while end < b.len() {
                let d = b[end];
                let exponent = matches!(d, b'+' | b'-') && matches!(b[end - 1], b'e' | b'E');
                if d.is_ascii_alphanumeric()
                    || d == b'_'
                    || exponent
                    || (d == b'.' && b.get(end + 1) != Some(&b'.'))
                {
                    end += 1;
                } else {
                    break;
                }
            }
            // CSS colours and units read as one number.
            out.push(i, end, Some(Token::Number));
            i = end;
            continue;
        }
        if c == b'#'
            && syntax.has(DASHES)
            && syntax.has(KEYS)
            && b.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
        {
            let end = i
                + 1
                + b[i + 1..]
                    .iter()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .count();
            out.push(i, end, Some(Token::Number));
            i = end;
            continue;
        }
        if ident_start(c) {
            let mut end = i + 1;
            while end < b.len()
                && (ident(b[end])
                    || syntax.has(DASHES)
                        && b[end] == b'-'
                        && b.get(end + 1).is_some_and(|&n| ident(n)))
            {
                end += 1;
            }
            let word = std::str::from_utf8(&b[i..end]).unwrap_or("");
            let rest = &b[end..];
            let gap = rest.iter().take_while(|c| c.is_ascii_whitespace()).count();
            let next = rest.get(gap).copied();
            let is = |list: &[&str]| {
                if syntax.has(NOCASE) {
                    list.iter().any(|w| w.eq_ignore_ascii_case(word))
                } else {
                    list.contains(&word)
                }
            };
            // `key:` (JSON, YAML, CSS) or `key =` at line start (INI, TOML).
            let key = (syntax.has(KEYS) && next == Some(b':') && rest.get(gap + 1) != Some(&b':'))
                || (syntax.has(ASSIGN) && next == Some(b'=') && i == first);
            // `name!(` macros in Rust, `name(` calls elsewhere.
            let call =
                (syntax.has(RUST) && rest.first() == Some(&b'!') && rest.get(1) != Some(&b'='))
                    || (syntax.has(CALLS) && next == Some(b'('));
            let token = if key {
                Some(Token::Type)
            } else if is(syntax.keywords) {
                Some(Token::Keyword)
            } else if is(syntax.literals) {
                Some(Token::Number)
            } else if is(syntax.types) {
                Some(Token::Type)
            } else if let Some(token) = defines {
                Some(token)
            } else if call {
                Some(Token::Function)
            } else if syntax.has(CAPITALS) && c.is_ascii_uppercase() {
                Some(Token::Type)
            } else {
                None
            };
            defines = match word {
                "fn" | "def" | "func" | "function" | "fun" | "sub" | "proc" => {
                    Some(Token::Function)
                }
                "class" | "struct" | "enum" | "trait" | "interface" | "type" | "impl" | "union"
                | "record" | "protocol" | "extension" | "object" | "module" | "mod"
                | "namespace" => Some(Token::Type),
                _ => None,
            };
            out.push(i, end, token);
            i = end;
            continue;
        }
        if !c.is_ascii_whitespace() {
            defines = None;
        }
        out.push(i, i + 1, None);
        i += 1;
    }
}

/// HTML / XML: tag names, attribute names, values, comments and entities.
fn markup(syntax: &Syntax, state: &mut State, b: &[u8], out: &mut Pieces) {
    let (open, close) = syntax.block.unwrap_or(("<!--", "-->"));
    let mut i = 0;
    while i < b.len() {
        match *state {
            State::Comment => match find_bytes(b, i, close.as_bytes()) {
                Some(end) => {
                    out.push(i, end + close.len(), Some(Token::Comment));
                    i = end + close.len();
                    *state = State::Normal;
                }
                None => {
                    out.push(i, b.len(), Some(Token::Comment));
                    return;
                }
            },
            State::Tag => {
                let c = b[i];
                if c == b'>' || b[i..].starts_with(b"/>") {
                    let end = if c == b'>' { i + 1 } else { i + 2 };
                    out.push(i, end, None);
                    i = end;
                    *state = State::Normal;
                } else if c == b'"' || c == b'\'' {
                    let end = b[i + 1..]
                        .iter()
                        .position(|&q| q == c)
                        .map_or(b.len(), |p| i + p + 2);
                    out.push(i, end, Some(Token::String));
                    i = end;
                } else if ident_start(c) {
                    let end = i + b[i..]
                        .iter()
                        .take_while(|&&c| ident(c) || matches!(c, b'-' | b':' | b'.'))
                        .count();
                    out.push(i, end, Some(Token::Type));
                    i = end;
                } else {
                    out.push(i, i + 1, None);
                    i += 1;
                }
            }
            State::Text { .. } | State::Normal => {
                *state = State::Normal;
                let c = b[i];
                if b[i..].starts_with(open.as_bytes()) {
                    *state = State::Comment;
                    out.push(i, i + open.len(), Some(Token::Comment));
                    i += open.len();
                } else if c == b'<' && matches!(b.get(i + 1), Some(b'!') | Some(b'?')) {
                    let end = b[i..]
                        .iter()
                        .position(|&c| c == b'>')
                        .map_or(b.len(), |p| i + p + 1);
                    out.push(i, end, Some(Token::Meta));
                    i = end;
                } else if c == b'<' {
                    let name = i + 1 + usize::from(b.get(i + 1) == Some(&b'/'));
                    let end = name
                        + b[name..]
                            .iter()
                            .take_while(|&&c| ident(c) || matches!(c, b'-' | b':' | b'.'))
                            .count();
                    out.push(i, name, None);
                    if end > name {
                        out.push(name, end, Some(Token::Keyword));
                        *state = State::Tag;
                    }
                    i = end.max(i + 1);
                } else if c == b'&' {
                    let end = b[i..]
                        .iter()
                        .take(12)
                        .position(|&c| c == b';')
                        .map_or(i + 1, |p| i + p + 1);
                    out.push(
                        i,
                        end,
                        if end > i + 1 {
                            Some(Token::Number)
                        } else {
                            None
                        },
                    );
                    i = end;
                } else {
                    out.push(i, i + 1, None);
                    i += 1;
                }
            }
        }
    }
}
