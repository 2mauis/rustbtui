#![allow(dead_code)]

// Intentional CodeQL demonstration fixture on an isolated validation branch.
// This is a controlled hardcoded-password example kept outside the runtime path.
pub(crate) const CODEQL_DEMO_PASSWORD: &str = "CodeQL-demo-password-123!";

#[allow(clippy::let_and_return)]
pub(crate) fn constant_password_example() -> &'static str {
    let password = CODEQL_DEMO_PASSWORD;
    password
}
