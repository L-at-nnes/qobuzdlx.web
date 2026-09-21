use std::sync::LazyLock;

use regex::Regex;

static LINE_BREAK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>|</p>").expect("valid pattern"));

static HTML_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").expect("valid pattern"));

/// Qobuz sends the description of an album as HTML: paragraphs, emphasis, and accents or © as
/// entities such as `&copy;`. A tag reader shows all of it verbatim, so it is turned into plain text.
#[must_use]
pub fn html_to_text(html: &str) -> String {
    let with_breaks = LINE_BREAK.replace_all(html, "\n");
    let without_tags = HTML_TAG.replace_all(&with_breaks, "");
    html_escape::decode_html_entities(&without_tags)
        .trim()
        .to_owned()
}

/// The comment tag: the user's template, in which `%Description%` stands for the album description
/// and `<br/>` for a line break.
#[must_use]
pub fn build_comment(template: &str, description: Option<&str>) -> String {
    let description = html_to_text(description.unwrap_or_default());

    // Case-insensitive, like the variables of the naming templates
    let mut comment = String::with_capacity(template.len() + description.len());
    let lowered = template.to_lowercase();
    let mut from = 0;
    while let Some(found) = lowered[from..].find("%description%") {
        comment.push_str(&template[from..from + found]);
        comment.push_str(&description);
        from += found + "%description%".len();
    }
    comment.push_str(&template[from..]);

    comment.replace("<br/>", "\n").replace("<br />", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entities_are_decoded() {
        assert_eq!(
            html_to_text("Caf&eacute; &copy; 2020 &amp; co"),
            "Café © 2020 & co"
        );
    }

    #[test]
    fn paragraphs_become_line_breaks_and_other_markup_disappears() {
        assert_eq!(
            html_to_text("<p>A <em>big</em> album</p><p>Second &amp; last</p>"),
            "A big album\nSecond & last"
        );
    }

    #[test]
    fn line_break_tags_become_line_breaks_in_any_spelling() {
        assert_eq!(html_to_text("a<br>b<br/>c<BR />d"), "a\nb\nc\nd");
    }

    #[test]
    fn nothing_gives_nothing() {
        assert_eq!(html_to_text(""), "");
        assert_eq!(html_to_text("<p>  </p>"), "");
    }

    #[test]
    fn the_template_receives_the_description_and_its_own_line_breaks() {
        assert_eq!(
            build_comment("Ripped<br/>%Description%", Some("<p>Calm</p>")),
            "Ripped\nCalm"
        );
        assert_eq!(
            build_comment("%DESCRIPTION% / %description%", Some("x")),
            "x / x"
        );
    }

    #[test]
    fn a_missing_description_leaves_the_rest_of_the_template() {
        assert_eq!(build_comment("Ripped: %Description%", None), "Ripped: ");
    }
}
