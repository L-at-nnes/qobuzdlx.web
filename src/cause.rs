//! Telling what really went wrong.

use std::error::Error;

/// An error and what caused it, from the outside in, separated by `: `.
///
/// `reqwest` reports "error decoding response body" for a connection that was cut halfway, and the
/// reason (a reset, a timeout, a certificate) is one level down. A cause that the message above it
/// already repeats is left out.
#[must_use]
pub fn describe(error: &(dyn Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let message = cause.to_string();
        if !message.is_empty() && !text.contains(&message) {
            text.push_str(": ");
            text.push_str(&message);
        }
        source = cause.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

    #[derive(Debug)]
    struct Layer(&'static str, Option<Box<Layer>>);

    impl fmt::Display for Layer {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.0)
        }
    }

    impl Error for Layer {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            self.1
                .as_deref()
                .map(|layer| layer as &(dyn Error + 'static))
        }
    }

    #[test]
    fn an_error_without_a_cause_is_its_message() {
        assert_eq!(describe(&Layer("it broke", None)), "it broke");
    }

    #[test]
    fn the_causes_follow_from_the_outside_in() {
        let error = Layer(
            "error decoding response body",
            Some(Box::new(Layer(
                "request or response body error",
                Some(Box::new(Layer("connection reset by peer", None))),
            ))),
        );

        assert_eq!(
            describe(&error),
            "error decoding response body: request or response body error: connection reset by peer"
        );
    }

    #[test]
    fn a_cause_the_message_already_says_is_not_repeated() {
        let error = Layer(
            "failed: timed out",
            Some(Box::new(Layer("timed out", None))),
        );

        assert_eq!(describe(&error), "failed: timed out");
    }
}
