//! Native Functions Module
//! This module contains all built-in native functions for the Taurine interpreter.
//! These functions provide access to system resources and common operations.

use crate::value::Value;
use crate::string_intern::InternedString;
use indexmap::IndexMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::io::Read;
use smallvec::SmallVec;
use std::sync::OnceLock;

/// Sanitize user-provided path to prevent path traversal attacks
fn sanitize_path(user_path: &str) -> Result<PathBuf, String> {
    sanitize_path_with_absolute(user_path, true)
}

fn sanitize_path_with_absolute(user_path: &str, allow_absolute: bool) -> Result<PathBuf, String> {
    let full_path = Path::new(user_path);

    if full_path.is_absolute() {
        if !allow_absolute {
            return Err("Absolute paths are not allowed in sandbox mode".to_string());
        }
        let canonical = full_path.canonicalize()
            .map_err(|e| format!("Invalid path: {e}"))?;
        return Ok(canonical);
    }

    for component in full_path.components() {
        if let std::path::Component::ParentDir = component {
            return Err("Path traversal detected".to_string());
        }
    }

    let current_dir = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {e}"))?;

    let resolved = current_dir.join(full_path);
    let canonical = resolved.canonicalize()
        .map_err(|e| format!("Invalid path: {e}"))?;

    if !canonical.starts_with(&current_dir) {
        return Err("Path traversal detected: access outside current directory is not allowed".to_string());
    }

    Ok(canonical)
}

/// Helper to create an interned string for built-in function names
const fn intern_builtin(id: u32) -> InternedString {
    InternedString::new(id as usize)
}

macro_rules! reg {
    ($global:expr, $id:expr, $func:expr) => {
        $global.borrow_mut().define(
            intern_builtin($id),
            Value::NativeFunction(Rc::new($func))
        );
    };
}

/// Register all built-in functions in the given environment
pub fn register_builtins(global: &Rc<RefCell<crate::environment::Environment>>) {
    register_core_functions(global);
    register_io_functions(global);
    register_string_functions(global);
    register_array_functions(global);
    register_json_functions(global);
    register_http_functions(global);
    register_crypto_functions(global);
    register_date_functions(global);
    register_regex_functions(global);
    register_async_functions(global);
}


// Core Functions
fn register_core_functions(global: &Rc<RefCell<crate::environment::Environment>>) {
    reg!(global, 1, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        let output = args.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
        println!("{output}");
        Ok(Value::Nil)
    });

    reg!(global, 2, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() {
            return Err("assert() requires at least 1 argument".to_string());
        }
        if !args[0].is_truthy() {
            let msg = if args.len() > 1 {
                args[1].to_string()
            } else {
                "Assertion failed".to_string()
            };
            return Err(format!("ASSERTION FAILED: {msg}"));
        }
        Ok(Value::Nil)
    });

    reg!(global, 3, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 {
            return Err("assert_eq() requires 2 arguments".to_string());
        }
        if args[0] != args[1] {
            let msg = if args.len() > 2 {
                args[2].to_string()
            } else {
                format!("Expected {:?} == {:?}", args[0], args[1])
            };
            return Err(format!("ASSERTION FAILED: {msg}"));
        }
        Ok(Value::Nil)
    });

    reg!(global, 4, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() {
            return Err("type() requires 1 argument".to_string());
        }
        let type_name = match &args[0] {
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Bool(_) => "boolean",
            Value::Nil => "nil",
            Value::Table(_) => "table",
            Value::Array(_) => "array",
            Value::Range { .. } => "range",
            Value::Function { .. } => "function",
            Value::AsyncFunction { .. } => "async_function",
            Value::Generator { .. } => "generator",
            Value::Future(_) => "future",
            Value::NativeFunction(_) => "function",
            Value::Error(_) => "error",
        };
        Ok(Value::String(type_name.to_string()))
    });

    reg!(global, 5, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() {
            return Err("tonumber() requires 1 argument".to_string());
        }
        match &args[0] {
            Value::Number(n) => Ok(Value::Number(*n)),
            Value::String(s) => s.parse::<f64>()
                .map(Value::Number)
                .map_err(|_| format!("Cannot convert '{s}' to number")),
            _ => Err("tonumber() requires number or string".to_string()),
        }
    });

    reg!(global, 6, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() {
            return Err("tostring() requires 1 argument".to_string());
        }
        Ok(Value::String(args[0].to_string()))
    });
}


// I/O Functions
fn register_io_functions(global: &Rc<RefCell<crate::environment::Environment>>) {
    reg!(global, 10, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_read() requires 1 argument".to_string()); }
        if let Value::String(path) = &args[0] {
            let sanitized = sanitize_path(path)?;
            std::fs::read_to_string(&sanitized)
                .map(Value::String)
                .map_err(|e| format!("Cannot read file: {e}"))
        } else { Err("io_read() requires string path".to_string()) }
    });

    reg!(global, 11, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("io_write() requires 2 arguments".to_string()); }
        if let (Value::String(path), Value::String(content)) = (&args[0], &args[1]) {
            let sanitized = sanitize_path(path)?;
            std::fs::write(&sanitized, content)
                .map(|_| Value::Bool(true))
                .map_err(|e| format!("Cannot write file: {e}"))
        } else { Err("io_write() requires string path and content".to_string()) }
    });

    reg!(global, 12, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("io_append() requires 2 arguments".to_string()); }
        if let (Value::String(path), Value::String(content)) = (&args[0], &args[1]) {
            use std::io::Write;
            let sanitized = sanitize_path(path)?;
            let mut file = std::fs::OpenOptions::new()
                .append(true).create(true).open(&sanitized)
                .map_err(|e| format!("Cannot open file: {e}"))?;
            file.write_all(content.as_bytes())
                .map(|_| Value::Bool(true))
                .map_err(|e| format!("Cannot append: {e}"))
        } else { Err("io_append() requires string path and content".to_string()) }
    });

    reg!(global, 13, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_exists() requires 1 argument".to_string()); }
        if let Value::String(path) = &args[0] {
            match sanitize_path(path) {
                Ok(sanitized) => Ok(Value::Bool(std::path::Path::new(&sanitized).exists())),
                Err(_) => Ok(Value::Bool(false)),
            }
        } else { Err("io_exists() requires string path".to_string()) }
    });

    reg!(global, 14, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_remove() requires 1 argument".to_string()); }
        if let Value::String(path) = &args[0] {
            let sanitized = sanitize_path(path)?;
            std::fs::remove_file(&sanitized)
                .or_else(|_| std::fs::remove_dir_all(&sanitized))
                .map(|_| Value::Bool(true))
                .map_err(|e| format!("Cannot remove: {e}"))
        } else { Err("io_remove() requires string path".to_string()) }
    });

    reg!(global, 15, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_mkdir() requires 1 argument".to_string()); }
        if let Value::String(path) = &args[0] {
            let sanitized = sanitize_path(path)?;
            std::fs::create_dir_all(&sanitized)
                .map(|_| Value::Bool(true))
                .map_err(|e| format!("Cannot create directory: {e}"))
        } else { Err("io_mkdir() requires string path".to_string()) }
    });

    reg!(global, 16, |_args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        Ok(Value::String(std::env::consts::OS.to_string()))
    });

    reg!(global, 17, |_args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        Ok(Value::String(std::env::consts::ARCH.to_string()))
    });

    reg!(global, 18, |_args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        std::env::current_dir()
            .map(|p| Value::String(p.to_string_lossy().to_string()))
            .map_err(|e| format!("Cannot get current directory: {e}"))
    });

    reg!(global, 19, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        let code = if args.is_empty() { 0 } else {
            match &args[0] {
                Value::Number(n) => *n as i32,
                Value::Bool(b) => if *b { 0 } else { 1 },
                _ => 0
            }
        };
        Err(format!("__EXIT__:{code}"))
    });

    reg!(global, 20, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_sleep() requires 1 argument".to_string()); }
        match &args[0] {
            Value::Number(n) => {
                std::thread::sleep(std::time::Duration::from_secs_f64(*n));
                Ok(Value::Nil)
            }
            _ => Err("io_sleep() requires number of seconds".to_string())
        }
    });

    reg!(global, 21, |_args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|e| format!("Time error: {e}"))?;
        Ok(Value::Number(duration.as_secs_f64()))
    });
}


// String Functions
fn register_string_functions(global: &Rc<RefCell<crate::environment::Environment>>) {
    reg!(global, 30, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_strupper() requires 1 argument".to_string()); }
        if let Value::String(s) = &args[0] {
            Ok(Value::String(s.to_uppercase()))
        } else { Err("io_strupper() requires string".to_string()) }
    });

    reg!(global, 31, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_strlower() requires 1 argument".to_string()); }
        if let Value::String(s) = &args[0] {
            Ok(Value::String(s.to_lowercase()))
        } else { Err("io_strlower() requires string".to_string()) }
    });

    reg!(global, 32, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_strtrim() requires 1 argument".to_string()); }
        if let Value::String(s) = &args[0] {
            Ok(Value::String(s.trim().to_string()))
        } else { Err("io_strtrim() requires string".to_string()) }
    });

    reg!(global, 33, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("io_strsubstr() requires 2 arguments".to_string()); }
        if let (Value::String(s), Value::Number(start)) = (&args[0], &args[1]) {
            let len = if args.len() > 2 {
                match &args[2] { Value::Number(l) => *l as usize, _ => s.len() }
            } else { s.len() };
            let start = *start as usize;
            let end = (start + len).min(s.len());
            if start >= s.len() {
                Ok(Value::String("".to_string()))
            } else {
                Ok(Value::String(s[start..end].to_string()))
            }
        } else { Err("io_strsubstr() requires string and number".to_string()) }
    });

    reg!(global, 34, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("io_strfind() requires 2 arguments".to_string()); }
        if let (Value::String(s), Value::String(pattern)) = (&args[0], &args[1]) {
            match s.find(pattern.as_str()) {
                Some(i) => Ok(Value::Number(i as f64)),
                None => Ok(Value::Number(-1.0)),
            }
        } else { Err("io_strfind() requires two strings".to_string()) }
    });

    reg!(global, 35, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 3 { return Err("io_strreplace() requires 3 arguments".to_string()); }
        if let (Value::String(s), Value::String(from), Value::String(to)) = (&args[0], &args[1], &args[2]) {
            Ok(Value::String(s.replacen(from.as_str(), to.as_str(), 1)))
        } else { Err("io_strreplace() requires three strings".to_string()) }
    });

    reg!(global, 36, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 3 { return Err("io_strreplaceall() requires 3 arguments".to_string()); }
        if let (Value::String(s), Value::String(from), Value::String(to)) = (&args[0], &args[1], &args[2]) {
            Ok(Value::String(s.replace(from.as_str(), to.as_str())))
        } else { Err("io_strreplaceall() requires three strings".to_string()) }
    });

    reg!(global, 37, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("io_strsplit() requires 2 arguments".to_string()); }
        if let (Value::String(s), Value::String(delimiter)) = (&args[0], &args[1]) {
            let arr = Rc::new(RefCell::new(
                s.split(delimiter.as_str()).map(|x| Value::String(x.to_string())).collect()
            ));
            Ok(Value::Array(arr))
        } else { Err("io_strsplit() requires string and delimiter".to_string()) }
    });

    reg!(global, 38, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_char() requires 1 argument".to_string()); }
        match &args[0] {
            Value::Number(n) => {
                let c = *n as u8 as char;
                Ok(Value::String(c.to_string()))
            }
            _ => Err("io_char() requires number".to_string())
        }
    });

    reg!(global, 39, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_byte() requires 1 argument".to_string()); }
        if let Value::String(s) = &args[0] {
            if let Some(c) = s.chars().next() {
                Ok(Value::Number(c as u32 as f64))
            } else {
                Ok(Value::Number(0.0))
            }
        } else { Err("io_byte() requires string".to_string()) }
    });
}


// Array Functions
fn register_array_functions(global: &Rc<RefCell<crate::environment::Environment>>) {
    reg!(global, 40, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("io_arraypush() requires 2 arguments".to_string()); }
        if let Value::Array(arr) = &args[0] {
            arr.borrow_mut().push(args[1].clone());
            Ok(Value::Nil)
        } else { Err("io_arraypush() requires array".to_string()) }
    });

    reg!(global, 41, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_arraypop() requires 1 argument".to_string()); }
        if let Value::Array(arr) = &args[0] {
            let mut borrowed = arr.borrow_mut();
            Ok(borrowed.pop().unwrap_or(Value::Nil))
        } else { Err("io_arraypop() requires array".to_string()) }
    });

    reg!(global, 42, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_arraylen() requires 1 argument".to_string()); }
        if let Value::Array(arr) = &args[0] {
            Ok(Value::Number(arr.borrow().len() as f64))
        } else { Err("io_arraylen() requires array".to_string()) }
    });

    reg!(global, 43, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("io_arrayget() requires 2 arguments".to_string()); }
        if let Value::Array(arr) = &args[0] {
            let idx = match &args[1] {
                Value::Number(n) => *n as usize,
                _ => return Err("io_arrayget() requires number index".to_string())
            };
            if idx < arr.borrow().len() {
                Ok(arr.borrow()[idx].clone())
            } else {
                Ok(Value::Nil)
            }
        } else { Err("io_arrayget() requires array".to_string()) }
    });

    reg!(global, 44, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 3 { return Err("io_arrayset() requires 3 arguments".to_string()); }
        if let Value::Array(arr) = &args[0] {
            let idx = match &args[1] {
                Value::Number(n) => *n as usize,
                _ => return Err("io_arrayset() requires number index".to_string())
            };
            if idx < arr.borrow().len() {
                arr.borrow_mut()[idx] = args[2].clone();
                Ok(Value::Nil)
            } else {
                Err(format!("Index {} out of bounds", idx))
            }
        } else { Err("io_arrayset() requires array".to_string()) }
    });

    reg!(global, 45, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("io_arrayconcat() requires 2 arguments".to_string()); }
        if let (Value::Array(arr1), Value::Array(arr2)) = (&args[0], &args[1]) {
            let mut result = arr1.borrow().clone();
            result.extend(arr2.borrow().iter().cloned());
            Ok(Value::Array(Rc::new(RefCell::new(result))))
        } else { Err("io_arrayconcat() requires two arrays".to_string()) }
    });

    reg!(global, 46, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_arrayreverse() requires 1 argument".to_string()); }
        if let Value::Array(arr) = &args[0] {
            let mut vec = arr.borrow().clone();
            vec.reverse();
            *arr.borrow_mut() = vec;
            Ok(Value::Nil)
        } else { Err("io_arrayreverse() requires array".to_string()) }
    });

    reg!(global, 47, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("io_arrayclear() requires 1 argument".to_string()); }
        if let Value::Array(arr) = &args[0] {
            arr.borrow_mut().clear();
            Ok(Value::Nil)
        } else { Err("io_arrayclear() requires array".to_string()) }
    });
}


// JSON Functions
fn register_json_functions(global: &Rc<RefCell<crate::environment::Environment>>) {
    reg!(global, 50, |args: &[Value], interner: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("json_parse() requires 1 argument".to_string()); }
        if let Value::String(s) = &args[0] {
            json_str_to_value(s, interner)
        } else { Err("json_parse() requires string".to_string()) }
    });

    reg!(global, 51, |args: &[Value], interner: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("json_stringify() requires 1 argument".to_string()); }
        let json = value_to_json_str(&args[0], interner)?;
        Ok(Value::String(json))
    });
}

const MAX_JSON_DEPTH: usize = 100;

/// Parse JSON string to Value, using interner for object keys
fn json_str_to_value(s: &str, interner: &mut crate::string_intern::StringInterner) -> Result<Value, String> {
    let parsed = serde_json::from_str::<serde_json::Value>(s)
        .map_err(|e| format!("JSON parse error: {e}"))?;

    fn convert_json(
        v: &serde_json::Value,
        depth: usize,
        interner: &mut crate::string_intern::StringInterner,
    ) -> Result<Value, String> {
        if depth > MAX_JSON_DEPTH {
            return Err(format!("JSON depth limit exceeded (max: {})", MAX_JSON_DEPTH));
        }
        match v {
            serde_json::Value::Null => Ok(Value::Nil),
            serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    Ok(Value::Number(f))
                } else {
                    Err(format!("JSON Error: Number '{}' is out of f64 bounds", n))
                }
            }
            serde_json::Value::String(s) => Ok(Value::String(s.clone())),
            serde_json::Value::Array(arr) => {
                let vec: Result<Vec<Value>, String> = arr.iter()
                    .map(|item| convert_json(item, depth + 1, interner))
                    .collect();
                let mut smallvec = SmallVec::new();
                for v in vec? {
                    smallvec.push(v);
                }
                Ok(Value::Array(Rc::new(RefCell::new(smallvec))))
            }
            serde_json::Value::Object(obj) => {
                let mut map = IndexMap::new();
                for (k, v) in obj {
                    let key_id = interner.intern(k);
                    map.insert(key_id, convert_json(v, depth + 1, interner)?);
                }
                Ok(Value::Table(Rc::new(RefCell::new(map))))
            }
        }
    }

    convert_json(&parsed, 0, interner)
}

/// Convert Value to JSON string, using interner to resolve object keys
fn value_to_json_str(v: &Value, interner: &crate::string_intern::StringInterner) -> Result<String, String> {
    fn convert_value(
        v: &Value,
        interner: &crate::string_intern::StringInterner,
    ) -> Result<serde_json::Value, String> {
        Ok(match v {
            Value::Number(n) => serde_json::Value::Number(
                serde_json::Number::from_f64(*n).unwrap_or(serde_json::Number::from(0))
            ),
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Nil => serde_json::Value::Null,
            Value::Array(arr) => {
                let vec: Result<Vec<_>, _> = arr.borrow().iter()
                    .map(|x| convert_value(x, interner))
                    .collect();
                serde_json::Value::Array(vec?)
            }
            Value::Table(t) => {
                let mut map = serde_json::Map::new();
                for (k, v) in t.borrow().iter() {
                    if let Some(key_str) = interner.get(*k) {
                        map.insert(key_str.to_string(), convert_value(v, interner)?);
                    } else {
                        map.insert(k.to_string(), convert_value(v, interner)?);
                    }
                }
                serde_json::Value::Object(map)
            }
            _ => return Err(format!("Cannot convert {:?} to JSON", v))
        })
    }

    let json = convert_value(v, interner)?;
    Ok(serde_json::to_string(&json).map_err(|e| format!("JSON stringify error: {e}"))?)
}


// HTTP Functions
const MAX_HTTP_RESPONSE_SIZE: usize = 10 * 1024 * 1024;

static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn get_http_client() -> &'static reqwest::blocking::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new())
    })
}

fn validate_http_url(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| format!("Invalid URL: {e}"))?;

    match parsed.scheme() {
        "http" | "https" => {},
        _ => return Err(format!("Scheme '{}' is not allowed", parsed.scheme())),
    }

    if let Some(host) = parsed.host_str() {
        if host == "localhost" || host == "127.0.0.1" || host == "::1" {
            return Err("Access to localhost is not allowed".to_string());
        }
        if host.starts_with("10.") ||
           host.starts_with("192.168.") ||
           host.starts_with("172.16.") ||
           host.starts_with("169.254.") {
            return Err("Access to private networks is not allowed".to_string());
        }
    }

    Ok(())
}

fn register_math_functions(global: &Rc<RefCell<crate::environment::Environment>>) {
    reg!(global, 110, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("math_floor() requires 1 argument".to_string()); }
        match &args[0] {
            Value::Number(n) => Ok(Value::Number(n.floor())),
            _ => Err("math_floor() requires number".to_string())
        }
    });
    reg!(global, 111, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("math_ceil() requires 1 argument".to_string()); }
        match &args[0] {
            Value::Number(n) => Ok(Value::Number(n.ceil())),
            _ => Err("math_ceil() requires number".to_string())
        }
    });
    reg!(global, 112, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("math_sqrt() requires 1 argument".to_string()); }
        match &args[0] {
            Value::Number(n) => {
                if *n < 0.0 { return Err("math_sqrt() requires non-negative number".to_string()); }
                Ok(Value::Number(n.sqrt()))
            }
            _ => Err("math_sqrt() requires number".to_string())
        }
    });
    reg!(global, 113, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("math_abs() requires 1 argument".to_string()); }
        match &args[0] {
            Value::Number(n) => Ok(Value::Number(n.abs())),
            _ => Err("math_abs() requires number".to_string())
        }
    });
}

fn register_http_functions(global: &Rc<RefCell<crate::environment::Environment>>) {
    reg!(global, 60, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("http_get() requires 1 argument".to_string()); }
        if let Value::String(url) = &args[0] {
            validate_http_url(url)?;
            let client = get_http_client();
            let response = client.get(url.as_str())
                .send()
                .map_err(|e| format!("HTTP GET error: {e}"))?;
            if let Some(content_length) = response.content_length() {
                if content_length > MAX_HTTP_RESPONSE_SIZE as u64 {
                    return Err(format!("Response too large: {} bytes", content_length));
                }
            }
            let mut reader = response.take((MAX_HTTP_RESPONSE_SIZE + 1) as u64);
            let mut text = String::new();
            reader.read_to_string(&mut text).map_err(|e| format!("Read response error: {e}"))?;
            if text.len() > MAX_HTTP_RESPONSE_SIZE {
                return Err(format!("HTTP Error: Response truncated. Exceeded limit of {} bytes", MAX_HTTP_RESPONSE_SIZE));
            }
            Ok(Value::String(text))
        } else { Err("http_get() requires string URL".to_string()) }
    });

    reg!(global, 61, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("http_post() requires 2 arguments".to_string()); }
        if let (Value::String(url), Value::String(data)) = (&args[0], &args[1]) {
            validate_http_url(url)?;
            let client = get_http_client();
            let response = client.post(url.as_str())
                .body(data.clone())
                .send()
                .map_err(|e| format!("HTTP POST error: {e}"))?;
            let mut reader = response.take((MAX_HTTP_RESPONSE_SIZE + 1) as u64);
            let mut text = String::new();
            reader.read_to_string(&mut text).map_err(|e| format!("Read response error: {e}"))?;
            if text.len() > MAX_HTTP_RESPONSE_SIZE {
                return Err(format!("HTTP Error: Response truncated. Exceeded limit of {} bytes", MAX_HTTP_RESPONSE_SIZE));
            }
            Ok(Value::String(text))
        } else { Err("http_post() requires string URL and data".to_string()) }
    });

    reg!(global, 62, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("http_put() requires 2 arguments".to_string()); }
        if let (Value::String(url), Value::String(data)) = (&args[0], &args[1]) {
            validate_http_url(url)?;
            let client = get_http_client();
            let response = client.put(url.as_str())
                .body(data.clone())
                .send()
                .map_err(|e| format!("HTTP PUT error: {e}"))?;
            let mut reader = response.take((MAX_HTTP_RESPONSE_SIZE + 1) as u64);
            let mut text = String::new();
            reader.read_to_string(&mut text).map_err(|e| format!("Read response error: {e}"))?;
            if text.len() > MAX_HTTP_RESPONSE_SIZE {
                return Err(format!("HTTP Error: Response truncated. Exceeded limit of {} bytes", MAX_HTTP_RESPONSE_SIZE));
            }
            Ok(Value::String(text))
        } else { Err("http_put() requires string URL and data".to_string()) }
    });

    reg!(global, 63, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("http_delete() requires 1 argument".to_string()); }
        if let Value::String(url) = &args[0] {
            validate_http_url(url)?;
            let client = get_http_client();
            let response = client.delete(url.as_str())
                .send()
                .map_err(|e| format!("HTTP DELETE error: {e}"))?;
            let mut reader = response.take((MAX_HTTP_RESPONSE_SIZE + 1) as u64);
            let mut text = String::new();
            reader.read_to_string(&mut text).map_err(|e| format!("Read response error: {e}"))?;
            if text.len() > MAX_HTTP_RESPONSE_SIZE {
                return Err(format!("HTTP Error: Response truncated. Exceeded limit of {} bytes", MAX_HTTP_RESPONSE_SIZE));
            }
            Ok(Value::String(text))
        } else { Err("http_delete() requires string URL".to_string()) }
    });
}


// Crypto Functions
fn register_crypto_functions(global: &Rc<RefCell<crate::environment::Environment>>) {
    reg!(global, 70, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("crypto_md5() requires 1 argument".to_string()); }
        let data = match &args[0] {
            Value::String(s) => s.as_bytes().to_vec(),
            Value::Number(n) => n.to_le_bytes().to_vec(),
            _ => return Err("crypto_md5() requires string or number".to_string())
        };
        let hash = md5::compute(&data);
        Ok(Value::String(format!("{hash:x}")))
    });

    reg!(global, 71, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("crypto_sha256() requires 1 argument".to_string()); }
        use sha2::{Sha256, Digest};
        let data = match &args[0] {
            Value::String(s) => s.as_bytes().to_vec(),
            Value::Number(n) => n.to_le_bytes().to_vec(),
            _ => return Err("crypto_sha256() requires string or number".to_string())
        };
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = hasher.finalize();
        Ok(Value::String(format!("{hash:x}")))
    });

    reg!(global, 72, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("crypto_base64_encode() requires 1 argument".to_string()); }
        if let Value::String(s) = &args[0] {
            use base64::{Engine, engine::general_purpose};
            let encoded = general_purpose::STANDARD.encode(s.as_bytes());
            Ok(Value::String(encoded))
        } else { Err("crypto_base64_encode() requires string".to_string()) }
    });

    reg!(global, 73, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("crypto_base64_decode() requires 1 argument".to_string()); }
        if let Value::String(s) = &args[0] {
            use base64::{Engine, engine::general_purpose};
            let decoded = general_purpose::STANDARD.decode(s.as_str())
                .map_err(|e| format!("Base64 decode error: {e}"))?;
            String::from_utf8(decoded)
                .map(Value::String)
                .map_err(|e| format!("UTF-8 decode error: {e}"))
        } else { Err("crypto_base64_decode() requires string".to_string()) }
    });

    reg!(global, 74, |_args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        use uuid::Uuid;
        Ok(Value::String(Uuid::new_v4().to_string()))
    });

    reg!(global, 75, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        let count = if args.is_empty() { 16 } else {
            match &args[0] {
                Value::Number(n) => *n as usize,
                _ => return Err("crypto_random_bytes() requires number".to_string())
            }
        };
        let mut bytes = vec![0u8; count];
        getrandom::getrandom(&mut bytes)
            .map_err(|e| format!("Random generation error: {e}"))?;
        Ok(Value::Array(Rc::new(RefCell::new(
            bytes.into_iter().map(|b| Value::Number(b as f64)).collect()
        ))))
    });
}


// Date Functions
fn register_date_functions(global: &Rc<RefCell<crate::environment::Environment>>) {
    reg!(global, 80, |_args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|e| format!("Time error: {e}"))?;
        Ok(Value::Number(duration.as_secs_f64()))
    });

    reg!(global, 81, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.is_empty() { return Err("date_format() requires 1 argument".to_string()); }
        let timestamp = match &args[0] {
            Value::Number(n) => *n,
            _ => return Err("date_format() requires number timestamp".to_string())
        };
        let format = if args.len() > 1 {
            match &args[1] {
                Value::String(s) => s.clone(),
                _ => "%Y-%m-%d %H:%M:%S".to_string()
            }
        } else {
            "%Y-%m-%d %H:%M:%S".to_string()
        };
        let datetime = chrono::DateTime::from_timestamp(timestamp as i64, 0)
            .ok_or("Invalid timestamp")?;
        let formatted = datetime.format(&format).to_string();
        Ok(Value::String(formatted))
    });
}

fn register_async_functions(global: &Rc<RefCell<crate::environment::Environment>>) {
    let async_sleep_fn = Rc::new(|args: &[Value], _int: &mut crate::string_intern::StringInterner| -> Result<Value, String> {
        if args.is_empty() { return Err("async_sleep() requires 1 argument".to_string()); }
        match &args[0] {
            Value::Number(ms) => {
                std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
                Ok(Value::Nil)
            }
            _ => Err("async_sleep() requires number of milliseconds".to_string())
        }
    });
    global.borrow_mut().define(intern_builtin(100), Value::NativeFunction(async_sleep_fn));

    let async_spawn_fn = Rc::new(|args: &[Value], _int: &mut crate::string_intern::StringInterner| -> Result<Value, String> {
        if args.is_empty() { return Err("async_spawn() requires 1 argument".to_string()); }
        Ok(Value::Nil)
    });
    global.borrow_mut().define(intern_builtin(101), Value::NativeFunction(async_spawn_fn));
}

// Regex Functions
fn register_regex_functions(global: &Rc<RefCell<crate::environment::Environment>>) {
    reg!(global, 90, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("regex_match() requires 2 arguments".to_string()); }
        if let (Value::String(pattern), Value::String(text)) = (&args[0], &args[1]) {
            let re = regex::Regex::new(pattern.as_str())
                .map_err(|e| format!("Invalid regex: {e}"))?;
            Ok(Value::Bool(re.is_match(text.as_str())))
        } else { Err("regex_match() requires two strings".to_string()) }
    });

    reg!(global, 91, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("regex_find() requires 2 arguments".to_string()); }
        if let (Value::String(pattern), Value::String(text)) = (&args[0], &args[1]) {
            let re = regex::Regex::new(pattern.as_str())
                .map_err(|e| format!("Invalid regex: {e}"))?;
            if let Some(m) = re.find(text.as_str()) {
                let result = Value::new_table();
                if let Value::Table(t) = &result {
                    let mut map = t.borrow_mut();
                    map.insert(1, Value::String(m.as_str().to_string()));
                    map.insert(2, Value::Number(m.start() as f64 + 1.0));
                    map.insert(3, Value::Number(m.end() as f64));
                }
                Ok(result)
            } else {
                Ok(Value::Nil)
            }
        } else { Err("regex_find() requires two strings".to_string()) }
    });

    reg!(global, 92, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 3 { return Err("regex_replace() requires 3 arguments".to_string()); }
        if let (Value::String(pattern), Value::String(text), Value::String(replacement)) = (&args[0], &args[1], &args[2]) {
            let re = regex::Regex::new(pattern.as_str())
                .map_err(|e| format!("Invalid regex: {e}"))?;
            let result = re.replacen(text.as_str(), 1, replacement.as_str());
            Ok(Value::String(result.to_string()))
        } else { Err("regex_replace() requires pattern, text, and replacement".to_string()) }
    });

    reg!(global, 93, |args: &[Value], _int: &mut crate::string_intern::StringInterner| {
        if args.len() < 2 { return Err("regex_find_all() requires 2 arguments".to_string()); }
        if let (Value::String(pattern), Value::String(text)) = (&args[0], &args[1]) {
            let re = regex::Regex::new(pattern.as_str())
                .map_err(|e| format!("Invalid regex: {e}"))?;
            let matches: Vec<Value> = re.find_iter(text.as_str())
                .map(|m| {
                    let result = Value::new_table();
                    if let Value::Table(t) = &result {
                        let mut map = t.borrow_mut();
                        map.insert(1, Value::String(m.as_str().to_string()));
                        map.insert(2, Value::Number(m.start() as f64 + 1.0));
                        map.insert(3, Value::Number(m.end() as f64));
                    }
                    result
                })
                .collect();
            let mut smallvec = SmallVec::new();
            for m in matches {
                smallvec.push(m);
            }
            Ok(Value::Array(Rc::new(RefCell::new(smallvec))))
        } else { Err("regex_find_all() requires two strings".to_string()) }
    });
}