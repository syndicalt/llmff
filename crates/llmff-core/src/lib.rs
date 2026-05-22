pub mod error;
pub mod manifest;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_exposes_version() {
        assert_eq!(crate::version(), env!("CARGO_PKG_VERSION"));
    }
}
