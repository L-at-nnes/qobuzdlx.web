use std::future::Future;

use super::{Page, Result};

/// Qobuz answers at most this many items per call, whatever the limit asked for.
pub(super) const PAGE_SIZE: u32 = 500;

/// Never gather more than this many items of one listing: a bound against an endpoint that keeps
/// answering with pages while its total never gets reached.
const MAX_ITEMS: usize = 1_000_000;

/// Appends to `first` the pages that follow it, until it holds `total` items.
///
/// `fetch` is given the offset of the page to read. A page that comes back empty ends the loop even
/// if the total was not reached: the listing shrank, or the total was wrong.
pub(super) async fn complete<T, F, Fut>(first: &mut Page<T>, mut fetch: F) -> Result<()>
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = Result<Page<T>>>,
{
    while first.items.len() < first.total as usize && first.items.len() < MAX_ITEMS {
        let offset = u32::try_from(first.items.len()).unwrap_or(u32::MAX);
        let next = fetch(offset).await?;
        if next.items.is_empty() {
            break;
        }
        first.items.extend(next.items);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn page(offset: u32, count: u32, total: u32) -> Page<u32> {
        Page {
            offset,
            limit: PAGE_SIZE,
            total,
            items: (offset..offset + count).collect(),
        }
    }

    #[tokio::test]
    async fn the_following_pages_are_appended_in_order() {
        let calls = RefCell::new(Vec::new());
        let mut first = page(0, 500, 1200);

        complete(&mut first, |offset| {
            calls.borrow_mut().push(offset);
            let count = (1200 - offset).min(500);
            async move { Ok(page(offset, count, 1200)) }
        })
        .await
        .unwrap();

        assert_eq!(*calls.borrow(), [500, 1000]);
        assert_eq!(first.items, (0..1200).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn a_complete_first_page_needs_no_further_call() {
        let mut first = page(0, 3, 3);

        complete(&mut first, |_| async { panic!("no call expected") })
            .await
            .unwrap();

        assert_eq!(first.items.len(), 3);
    }

    #[tokio::test]
    async fn an_empty_page_stops_the_loop() {
        let mut first = page(0, 500, 2000);

        complete(
            &mut first,
            |offset| async move { Ok(page(offset, 0, 2000)) },
        )
        .await
        .unwrap();

        assert_eq!(first.items.len(), 500);
    }

    #[tokio::test]
    async fn an_error_is_passed_on() {
        let mut first = page(0, 500, 2000);

        let result = complete(&mut first, |_| async {
            Err(super::super::Error::NotSignedIn)
        })
        .await;

        assert!(matches!(result, Err(super::super::Error::NotSignedIn)));
    }
}
