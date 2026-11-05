//! Build `filter=` expressions for the HL API, so filtering happens in GLPI.
//!
//! WHY. Several pages fetch a collection and then discard most of it here.
//! That is a whole walk of a collection - `Assets/Software` is 1330 rows in
//! three requests on this install - to answer a question GLPI could have
//! answered in one. The operators below have existed since 11.0.12; nothing
//! was blocking this except not having a safe way to build the string.
//!
//! WHY A BUILDER AND NOT `format!`. RSQL's grammar is delimiter-separated:
//! `;` is AND, `,` is OR, and an unquoted value runs until the next unescaped
//! `,`, `;` or `)`. So interpolating a value that happens to contain any of
//! those silently changes the query's shape rather than failing - a computer
//! named `foo;bar` turns one term into two. Values here are always quoted and
//! escaped, which is what `Lexer.php` supports: "Values MAY be quoted with
//! double or single quotes", with backslash escaping inside.
//!
//! VERSIONS. Every operator below exists on 11.0.12 except `=istruthy=` and
//! `=isnottruthy=`, which are new in 12.0.1 - confirmed by diffing
//! `RSQL/Parser.php` between the two releases, where they are the only
//! addition. They are gated behind `Op::requires_12` so a caller can tell.
//!
//! One upstream inconsistency, checked rather than worked around: the truthy
//! pair declare `'value_expected' => false` but are absent from
//! `Parser::isOperatorUnary()`'s list, which looked like it might stop them
//! parsing. Measured against a running 12.0.1 - `is_deleted=istruthy=` and
//! `is_deleted=isnottruthy=` both answer 200 and return the right rows - so
//! the inconsistency is harmless and they are treated as value-less here.

/// An RSQL comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Like,
    ILike,
    NotLike,
    NotILike,
    In,
    Out,
    IsNull,
    NotNull,
    Empty,
    NotEmpty,
    /// New in 12.0.1.
    IsTruthy,
    /// New in 12.0.1.
    IsNotTruthy,
}

impl Op {
    /// The token GLPI's lexer expects.
    pub fn token(self) -> &'static str {
        match self {
            Op::Eq => "==",
            Op::Ne => "!=",
            Op::Lt => "=lt=",
            Op::Le => "=le=",
            Op::Gt => "=gt=",
            Op::Ge => "=ge=",
            Op::Like => "=like=",
            Op::ILike => "=ilike=",
            Op::NotLike => "=notlike=",
            Op::NotILike => "=notilike=",
            Op::In => "=in=",
            Op::Out => "=out=",
            Op::IsNull => "=isnull=",
            Op::NotNull => "=notnull=",
            Op::Empty => "=empty=",
            Op::NotEmpty => "=notempty=",
            Op::IsTruthy => "=istruthy=",
            Op::IsNotTruthy => "=isnottruthy=",
        }
    }

    /// Whether GLPI expects a value after the operator.
    ///
    /// Four are listed by `isOperatorUnary()`; the truthy pair additionally
    /// declare `value_expected => false` in their own definition. Both kinds
    /// are value-less here - see the module note for why that is measured
    /// rather than inferred.
    pub fn takes_value(self) -> bool {
        !matches!(
            self,
            Op::IsNull | Op::NotNull | Op::Empty | Op::NotEmpty | Op::IsTruthy | Op::IsNotTruthy
        )
    }

    /// True for operators GLPI 11.0.12 does not have.
    ///
    /// A caller targeting an install that might be on 11.0.12 should avoid
    /// these; GLPI answers an unknown operator with an RSQL error rather than
    /// ignoring it, so the failure is loud either way.
    pub fn requires_12(self) -> bool {
        matches!(self, Op::IsTruthy | Op::IsNotTruthy)
    }
}

/// Quote and escape a value so no content can change the query's shape.
///
/// Double quotes, with `\` and `"` escaped. Unquoted values would end at the
/// first unescaped `,`, `;` or `)`, which is the whole hazard this exists to
/// remove.
fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        if c == '\\' || c == '"' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// One `field op value` term.
///
/// A value passed with a value-less operator is dropped rather than appended:
/// appending it would produce `is_deleted=istruthy="x"`, which GLPI rejects,
/// and the caller's intent is unambiguous.
pub fn term(field: &str, op: Op, value: Option<&str>) -> String {
    match (op.takes_value(), value) {
        (true, Some(v)) => format!("{field}{}{}", op.token(), quote(v)),
        // A value-taking operator with nothing to compare against would build
        // `name==`, which is not a filter. Degrade to the null check, which is
        // the only honest reading of "match this field against nothing".
        (true, None) => format!("{field}{}", Op::IsNull.token()),
        (false, _) => format!("{field}{}", op.token()),
    }
}

/// Several terms, all of which must hold. `;` is RSQL's AND.
pub fn all(terms: impl IntoIterator<Item = String>) -> String {
    terms.into_iter().collect::<Vec<_>>().join(";")
}

/// Several terms, any of which may hold. `,` is RSQL's OR.
///
/// Parenthesised, because mixing `,` and `;` without grouping takes GLPI's
/// precedence rather than the caller's.
pub fn any(terms: impl IntoIterator<Item = String>) -> String {
    let joined = terms.into_iter().collect::<Vec<_>>().join(",");
    if joined.is_empty() {
        joined
    } else {
        format!("({joined})")
    }
}

/// Append a filter to an HL path, ready for `Client::get`.
///
/// The value is percent-encoded: a filter carries `"`, `;` and `,`, none of
/// which survive a URL unencoded.
pub fn with_filter(path: &str, filter: &str) -> String {
    if filter.is_empty() {
        return path.to_string();
    }
    let sep = if path.contains('?') { '&' } else { '?' };
    format!("{path}{sep}filter={}", urlencode(filter))
}

/// Percent-encode everything outside the unreserved set.
///
/// Hand-rolled rather than adding a dependency for one use: the unreserved
/// set from RFC 3986 is small and anything else is encoded, which is always
/// safe even where it was not strictly required.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_simple_term_quotes_its_value() {
        assert_eq!(term("name", Op::Eq, Some("nixos")), r#"name=="nixos""#);
    }

    /// The reason this module exists. A value carrying RSQL's own delimiters
    /// must not be able to add or split terms.
    #[test]
    fn delimiters_in_a_value_cannot_change_the_query_shape() {
        let t = term("name", Op::Eq, Some("foo;bar,baz)qux"));
        assert_eq!(t, r#"name=="foo;bar,baz)qux""#);
        // One term, not three: every delimiter is inside the quotes.
        assert_eq!(t.matches('"').count(), 2);
    }

    #[test]
    fn quotes_and_backslashes_in_a_value_are_escaped() {
        assert_eq!(term("name", Op::Eq, Some(r#"a"b\c"#)), r#"name=="a\"b\\c""#);
    }

    #[test]
    fn value_less_operators_take_no_value() {
        assert_eq!(
            term("is_deleted", Op::IsTruthy, None),
            "is_deleted=istruthy="
        );
        assert_eq!(term("name", Op::NotEmpty, None), "name=notempty=");
        // A value handed to one is dropped rather than appended.
        assert_eq!(
            term("is_deleted", Op::IsTruthy, Some("1")),
            "is_deleted=istruthy="
        );
    }

    /// `name==` is not a filter; the null check is the only honest reading.
    #[test]
    fn a_value_taking_operator_with_no_value_degrades_to_the_null_check() {
        assert_eq!(term("name", Op::Eq, None), "name=isnull=");
    }

    #[test]
    fn all_joins_with_and_and_any_groups_with_or() {
        let a = term("is_deleted", Op::IsNotTruthy, None);
        let b = term("name", Op::ILike, Some("nix*"));
        assert_eq!(
            all([a.clone(), b.clone()]),
            "is_deleted=isnottruthy=;name=ilike=\"nix*\""
        );
        assert_eq!(any([a, b]), "(is_deleted=isnottruthy=,name=ilike=\"nix*\")");
    }

    /// Grouping matters: an un-parenthesised OR inside an AND would take
    /// GLPI's precedence rather than the caller's.
    #[test]
    fn an_or_nested_in_an_and_stays_grouped() {
        let or = any([
            term("itemtype", Op::Eq, Some("Computer")),
            term("itemtype", Op::Eq, Some("Printer")),
        ]);
        let f = all([term("is_deleted", Op::IsNotTruthy, None), or]);
        assert_eq!(
            f,
            r#"is_deleted=isnottruthy=;(itemtype=="Computer",itemtype=="Printer")"#
        );
    }

    #[test]
    fn empty_groups_do_not_produce_stray_parentheses() {
        assert_eq!(any(Vec::<String>::new()), "");
        assert_eq!(all(Vec::<String>::new()), "");
    }

    #[test]
    fn with_filter_encodes_and_picks_the_right_separator() {
        let f = term("name", Op::Eq, Some("a;b"));
        let u = with_filter("Assets/Computer", &f);
        assert_eq!(u, "Assets/Computer?filter=name%3D%3D%22a%3Bb%22");
        assert!(with_filter("Assets/Computer?x=1", &f).contains("&filter="));
        // Nothing to add, nothing added - not a trailing `?`.
        assert_eq!(with_filter("Assets/Computer", ""), "Assets/Computer");
    }

    /// The filter the Computers and Agents pages send, pinned.
    ///
    /// Written down because the choice of operator here was got wrong once in
    /// the other direction. `=isnottruthy=` compiles to `IS NOT TRUE` and so
    /// also matches NULL, which looked like the safer option - but
    /// `is_deleted` is `tinyint NOT NULL DEFAULT '0'` in every definition in
    /// GLPI's install/mysql, so no NULL can exist and the two are equivalent
    /// on this column. `==0` is kept because it also works on 11.0.12, where
    /// `=isnottruthy=` does not exist.
    ///
    /// If this ever moves to a nullable column, the operator must change -
    /// `==0` would silently drop NULL rows from a fleet view, which is worse
    /// than the deleted-machine bug it was introduced to fix.
    #[test]
    fn the_live_machines_filter_is_what_the_pages_expect() {
        let f = term("is_deleted", Op::Eq, Some("0"));
        assert_eq!(f, r#"is_deleted=="0""#);
        assert!(
            !Op::Eq.requires_12(),
            "the pages must keep working against 11.0.12"
        );
        assert_eq!(
            with_filter("Assets/Computer", &f),
            "Assets/Computer?filter=is_deleted%3D%3D%220%22"
        );
    }

    /// A filter is NOT a guarantee, and this is the note that says so.
    ///
    /// GLPI silently drops a term naming a field it does not know and runs
    /// the query without it. `Search.php:443` lifts `Error::UNKNOWN_PROPERTY`
    /// out of the set it throws on, so an unknown OPERATOR or a mapped
    /// property raise RSQLException while an unknown FIELD does not -
    /// upstream's own comment on that branch, "Fail the request if any of the
    /// filters are invalid", overstates what the code beneath it does.
    ///
    /// Measured against a running 12.0.1, counting rows on Assets/Computer:
    ///
    /// ```text
    ///   (no filter)                 1 row
    ///   name=="nixos"               1 row    filter applied
    ///   name=="no-such-machine"     0 rows   filter applied
    ///   no_such_field=="x"          1 row    filter IGNORED
    ///   is_deletedX==0              1 row    filter IGNORED
    /// ```
    ///
    /// So every caller that relies on a filter for CORRECTNESS rather than
    /// for efficiency has to re-check the result locally - `api::computers`
    /// does, and says why. This test exists to keep that reasoning attached
    /// to the builder rather than only to the one caller that currently
    /// remembers it.
    #[test]
    fn a_filter_is_an_optimisation_not_a_guarantee() {
        // Nothing here can detect the server ignoring a term; the builder
        // only promises a well-formed string. The assertion is that the
        // field name is carried verbatim, because that is the single thing a
        // caller can get wrong and never be told about.
        assert_eq!(term("is_deleted", Op::Eq, Some("0")), r#"is_deleted=="0""#);
        assert_eq!(
            term("is_deletedX", Op::Eq, Some("0")),
            r#"is_deletedX=="0""#,
            "a typo builds just as happily, and GLPI will accept and ignore it"
        );
    }

    /// The inverse, used to count what was excluded. `!=` rather than
    /// `=istruthy=` for the same portability reason.
    #[test]
    fn the_excluded_count_filter_is_the_exact_complement() {
        let live = term("is_deleted", Op::Eq, Some("0"));
        let gone = term("is_deleted", Op::Ne, Some("0"));
        assert_eq!(gone, r#"is_deleted!="0""#);
        assert_ne!(live, gone);
        assert!(!Op::Ne.requires_12());
    }

    #[test]
    fn the_two_new_operators_are_flagged_as_needing_12() {
        assert!(Op::IsTruthy.requires_12());
        assert!(Op::IsNotTruthy.requires_12());
        for op in [Op::Eq, Op::Gt, Op::ILike, Op::IsNull, Op::NotEmpty, Op::In] {
            assert!(!op.requires_12(), "{op:?} exists on 11.0.12");
        }
    }

    /// Every token is what GLPI's parser declares, spelled out so a typo in
    /// one cannot pass unnoticed.
    #[test]
    fn tokens_match_upstreams_spelling() {
        assert_eq!(Op::Ne.token(), "!=");
        assert_eq!(Op::Le.token(), "=le=");
        assert_eq!(Op::NotILike.token(), "=notilike=");
        assert_eq!(Op::Out.token(), "=out=");
        assert_eq!(Op::IsNotTruthy.token(), "=isnottruthy=");
    }
}
