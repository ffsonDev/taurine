use taurine::safety::{
    SecurityContext, SecurityLevel, Permissions,
    InputValidator, ResourceTracker, SecurityError,
};
use std::time::Duration;

#[test]
fn test_security_levels_ordering() {
    let full = SecurityLevel::Full;
    let standard = SecurityLevel::Standard;
    let restricted = SecurityLevel::Restricted;
    let sandbox = SecurityLevel::Sandbox;

    assert_ne!(full, standard);
    assert_ne!(standard, restricted);
    assert_ne!(restricted, sandbox);
}

#[test]
fn test_sandbox_blocks_all_io() {
    let mut ctx = SecurityContext::new();
    ctx.apply_level(&SecurityLevel::Sandbox);

    assert!(!ctx.is_function_allowed("io_read"));
    assert!(!ctx.is_function_allowed("io_write"));
    assert!(!ctx.is_function_allowed("io_append"));
    assert!(!ctx.is_function_allowed("io_remove"));
    assert!(!ctx.is_function_allowed("http_get"));
    assert!(!ctx.is_function_allowed("http_post"));
}

#[test]
fn test_sandbox_blocks_filesystem() {
    let mut ctx = SecurityContext::new();
    ctx.apply_level(&SecurityLevel::Sandbox);

    let result = ctx.validate_path("/etc/passwd");
    assert!(result.is_err());
}

#[test]
fn test_sandbox_blocks_network() {
    let mut ctx = SecurityContext::new();
    ctx.apply_level(&SecurityLevel::Sandbox);

    let result = ctx.validate_host("evil.com");
    assert!(result.is_err());
}

#[test]
fn test_full_level_allows_everything() {
    let mut ctx = SecurityContext::new();
    ctx.apply_level(&SecurityLevel::Full);

    assert!(ctx.is_function_allowed("io_read"));
    assert!(ctx.is_function_allowed("io_write"));
    assert!(ctx.is_function_allowed("http_get"));
    assert!(ctx.validate_path("/any/path").is_ok());
    assert!(ctx.validate_host("any.host.com").is_ok());
}

#[test]
fn test_restricted_blocks_network() {
    let mut ctx = SecurityContext::new();
    ctx.apply_level(&SecurityLevel::Restricted);

    let result = ctx.validate_host("example.com");
    assert!(result.is_err());
}

#[test]
fn test_restricted_allows_filesystem() {
    let mut ctx = SecurityContext::new();
    ctx.apply_level(&SecurityLevel::Restricted);

    let result = ctx.validate_path("/safe/path");
    assert!(result.is_ok());
}

#[test]
fn test_custom_blocked_function() {
    let mut ctx = SecurityContext::new();
    ctx.block_function("dangerous_func");

    assert!(!ctx.is_function_allowed("dangerous_func"));
    assert!(ctx.is_function_allowed("safe_func"));

    ctx.unblock_function("dangerous_func");
    assert!(ctx.is_function_allowed("dangerous_func"));
}

#[test]
fn test_permissions_full() {
    let perms = Permissions::full();
    assert!(perms.allow_fs);
    assert!(perms.allow_network);
    assert!(perms.allow_env);
    assert!(perms.allow_process);
    assert!(perms.allow_native);
}

#[test]
fn test_permissions_sandbox() {
    let perms = Permissions::sandbox();
    assert!(!perms.allow_fs);
    assert!(!perms.allow_network);
    assert!(!perms.allow_env);
    assert!(!perms.allow_process);
    assert!(!perms.allow_native);
}

#[test]
fn test_permissions_restricted() {
    let perms = Permissions::restricted();
    assert!(perms.allow_fs);
    assert!(!perms.allow_network);
    assert!(!perms.allow_env);
    assert!(!perms.allow_process);
    assert!(perms.allow_native);
}

#[test]
fn test_permissions_allowed_paths() {
    let mut perms = Permissions::default();
    perms.allow_fs = true;
    perms.allowed_paths.insert("/safe".to_string());
    perms.allowed_paths.insert("/tmp".to_string());

    assert!(perms.is_path_allowed("/safe/file.txt"));
    assert!(perms.is_path_allowed("/tmp/data.txt"));
    assert!(!perms.is_path_allowed("/etc/passwd"));
}

#[test]
fn test_permissions_allowed_hosts() {
    let mut perms = Permissions::default();
    perms.allow_network = true;
    perms.allowed_hosts.insert("api.".to_string());
    perms.allowed_hosts.insert("trusted.com".to_string());

    assert!(perms.is_host_allowed("api.example.com"));
    assert!(perms.is_host_allowed("trusted.com"));
    assert!(!perms.is_host_allowed("malicious.com"));
}

#[test]
fn test_permissions_file_size_limit() {
    let mut perms = Permissions::restricted();
    perms.max_file_size = 1024;

    assert!(perms.max_file_size == 1024);
}

#[test]
fn test_input_validator_safe_string() {
    assert!(InputValidator::validate_string("hello world", 100).is_ok());
}

#[test]
fn test_input_validator_too_long() {
    let long_string = "x".repeat(200);
    assert!(InputValidator::validate_string(&long_string, 100).is_err());
}

#[test]
fn test_input_validator_path_traversal() {
    assert!(InputValidator::validate_string("../etc/passwd", 100).is_err());
    assert!(InputValidator::validate_string("..\\windows\\system32", 100).is_err());
}

#[test]
fn test_input_validator_script_injection() {
    assert!(InputValidator::validate_string("<script>alert(1)</script>", 100).is_err());
    assert!(InputValidator::validate_string("javascript:void(0)", 100).is_err());
}

#[test]
fn test_input_validator_valid_identifier() {
    assert!(InputValidator::validate_identifier("valid_name").is_ok());
    assert!(InputValidator::validate_identifier("_private").is_ok());
    assert!(InputValidator::validate_identifier("camelCase").is_ok());
}

#[test]
fn test_input_validator_invalid_identifier() {
    assert!(InputValidator::validate_identifier("").is_err());
    assert!(InputValidator::validate_identifier("123invalid").is_err());
    assert!(InputValidator::validate_identifier("-dash").is_err());
}

#[test]
fn test_input_validator_valid_number() {
    assert!(InputValidator::validate_number(42.0).is_ok());
    assert!(InputValidator::validate_number(-3.14).is_ok());
    assert!(InputValidator::validate_number(0.0).is_ok());
}

#[test]
fn test_input_validator_invalid_number() {
    assert!(InputValidator::validate_number(f64::NAN).is_err());
    assert!(InputValidator::validate_number(f64::INFINITY).is_err());
    assert!(InputValidator::validate_number(f64::NEG_INFINITY).is_err());
}

#[test]
fn test_resource_tracker_operations() {
    let mut tracker = ResourceTracker::with_max_operations(10);

    for _ in 0..10 {
        assert!(tracker.record_operation().is_ok());
    }
    assert!(tracker.record_operation().is_err());
}

#[test]
fn test_resource_tracker_memory() {
    let mut tracker = ResourceTracker::new();
    tracker.record_memory(1024);
    tracker.record_memory(2048);
    assert_eq!(tracker.memory_used(), 3072);
}

#[test]
fn test_resource_tracker_timeout() {
    let tracker = ResourceTracker::new();
    assert!(tracker.check_timeout(Duration::from_secs(10)).is_ok());

    std::thread::sleep(Duration::from_millis(50));
    assert!(tracker.check_timeout(Duration::from_millis(10)).is_err());
}

#[test]
fn test_resource_tracker_reset() {
    let mut tracker = ResourceTracker::new();
    tracker.record_memory(1024);
    tracker.record_operation().unwrap();
    tracker.reset();

    assert_eq!(tracker.memory_used(), 0);
    assert_eq!(tracker.operations(), 0);
}

#[test]
fn test_security_error_display() {
    let err = SecurityError::PathNotAllowed("/forbidden".to_string());
    assert!(err.to_string().contains("/forbidden"));

    let err = SecurityError::FileSizeExceeded { size: 2000, max: 1000 };
    assert!(err.to_string().contains("2000"));
    assert!(err.to_string().contains("1000"));

    let err = SecurityError::FunctionNotAllowed("evil_func".to_string());
    assert!(err.to_string().contains("evil_func"));
}

#[test]
fn test_security_context_timeout() {
    let mut ctx = SecurityContext::new();
    ctx.set_timeout(Duration::from_secs(60));
    assert_eq!(ctx.timeout(), Some(Duration::from_secs(60)));
}

#[test]
fn test_security_context_max_memory() {
    let mut ctx = SecurityContext::new();
    ctx.set_max_memory(512 * 1024 * 1024);
    assert_eq!(ctx.max_memory(), 512 * 1024 * 1024);
}

#[test]
fn test_security_context_max_recursion() {
    let mut ctx = SecurityContext::new();
    ctx.set_max_recursion(2000);
    assert_eq!(ctx.max_recursion(), 2000);
}

#[test]
fn test_file_size_validation() {
    let mut ctx = SecurityContext::new();
    ctx.apply_level(&SecurityLevel::Restricted);

    let small_size = 1024;
    assert!(ctx.validate_file_size(small_size).is_ok());

    let large_size = 100 * 1024 * 1024;
    assert!(ctx.validate_file_size(large_size).is_err());
}

#[test]
fn test_network_size_validation() {
    let mut ctx = SecurityContext::new();
    ctx.apply_level(&SecurityLevel::Restricted);

    let small_size = 1024;
    assert!(ctx.validate_network_size(small_size).is_ok());

    let large_size = 10 * 1024 * 1024;
    assert!(ctx.validate_network_size(large_size).is_err());
}