#![allow(dead_code)]

pub(crate) struct DemoCredential {
    pub(crate) username: &'static str,
    pub(crate) password: &'static str,
}

pub(crate) const CODEQL_DEMO_CREDENTIAL: DemoCredential = DemoCredential {
    username: "demo-user",
    password: "CodeQL-demo-password-123!",
};
