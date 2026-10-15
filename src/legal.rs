//! Release license and corresponding-source information, embedded at build time.
//! Saving these notices allocates a document; normal UI rendering reads only
//! the static strings and does not load files or format the combined document.

pub const NOTICES: &str =
    include_str!(concat!(env!("OUT_DIR"), "/cogcalc-third-party-notices.txt"));
pub const SOURCE: &str = include_str!(concat!(env!("OUT_DIR"), "/cogcalc-source.txt"));

const PROJECT_LICENSE: &str = include_str!("../LICENSE");

pub fn combined_notices() -> String {
    let mut document =
        String::with_capacity(PROJECT_LICENSE.len() + SOURCE.len() + NOTICES.len() + 160);
    document.push_str("CogCalc — license and source information\n\n");
    if !SOURCE.is_empty() {
        document.push_str(SOURCE);
        document.push_str("\n\n");
    }
    document.push_str("CogCalc is licensed under GPL-3.0-only.\n\n");
    document.push_str(PROJECT_LICENSE);
    if !NOTICES.is_empty() {
        document.push_str("\n\nThird-party notices\n\n");
        document.push_str(NOTICES);
    }
    document.push('\n');
    document
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_notices_preserve_the_complete_license_and_embedded_release_text() {
        let document = combined_notices();
        assert!(document.contains(PROJECT_LICENSE));
        assert!(document.contains("GPL-3.0-only"));
        if !NOTICES.is_empty() {
            assert!(document.contains(NOTICES));
        }
        if !SOURCE.is_empty() {
            assert!(document.contains(SOURCE));
        }
    }
}
