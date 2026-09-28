//! The report id of one benchmark entry.
//!
//! Every entry is named `<kind>_<target>_<id>`, and the report writes that id
//! `<kind>/<target>/<id>`, with `-` for the underscores of the target and the
//! id: `parse_nanonbt_serde_short_names` with the bench id `short_names` is
//! `parse/nanonbt-serde/short-names`.
//!
//! The `iai-callgrind` summaries carry the function name and the bench id as
//! two fields, and the portable registry stores the same pair, so this
//! function is what turns either into the id that the report rows and the
//! comparison pair by.

/// The report id of one entry, or `None` for a name that does not end in the
/// bench id its entry carries.
pub fn display_id(function_name: &str, bench_id: &str) -> Option<String> {
    let prefix = function_name.strip_suffix(bench_id)?.strip_suffix('_')?;
    let (kind, target) = prefix.split_once('_')?;
    Some(format!(
        "{kind}/{}/{}",
        target.replace('_', "-"),
        bench_id.replace('_', "-")
    ))
}

#[cfg(test)]
mod tests {
    use super::display_id;

    #[test]
    fn derives_the_display_id() {
        assert_eq!(
            display_id("parse_nanonbt_serde_short_names", "short_names").as_deref(),
            Some("parse/nanonbt-serde/short-names")
        );
        assert_eq!(
            display_id("skip_simdnbt_borrow_compound_list", "compound_list").as_deref(),
            Some("skip/simdnbt-borrow/compound-list")
        );
    }

    #[test]
    fn rejects_a_name_that_does_not_carry_the_id() {
        assert_eq!(display_id("parse_nanonbt_serde_small", "player"), None);
        assert_eq!(display_id("small", "small"), None);
    }
}
