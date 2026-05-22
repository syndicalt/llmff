pub mod backend;
pub mod engine;
pub mod error;
pub mod graph;
pub mod manifest;
pub mod stage;
pub mod value;

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
