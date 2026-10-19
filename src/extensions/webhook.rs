use std::time::Duration;

use futures_util::future::BoxFuture;

use super::manifest::Webhook;
use super::registry::now;
use super::{Event, Extension, Failure, Info, Source};

/// An extension that is an HTTP address: each event is sent to it as a `POST` with the JSON as the
/// body, `X-QobuzDLX-Event` naming the event and the headers of the manifest. Any `2xx` answer is
/// success; anything else, or no answer within the timeout, is reported as failed.
///
/// Unlike a program, it works in the container image (which has no shell), and the receiving end
/// can be written in any language and run anywhere.
#[derive(Debug, Clone)]
pub struct WebhookExtension {
    info: Info,
    webhook: Webhook,
    http: reqwest::Client,
}

impl WebhookExtension {
    pub fn new(info: Info, webhook: Webhook, timeout: Duration) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(concat!("QobuzDLX/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            info,
            webhook,
            http,
        })
    }
}

impl Extension for WebhookExtension {
    fn info(&self) -> Info {
        Info {
            source: Source::Webhook,
            ..self.info.clone()
        }
    }

    fn handle<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, Result<(), Failure>> {
        Box::pin(async move {
            let mut request = self
                .http
                .post(&self.webhook.url)
                .header("content-type", "application/json")
                .header("x-qobuzdlx-event", event.kind().name())
                .body(event.to_json(now()));
            for (name, value) in &self.webhook.headers {
                request = request.header(name, value);
            }

            let response = request.send().await.map_err(|error| {
                Failure(format!("{}: {}", self.webhook.url, error.without_url()))
            })?;
            let status = response.status();
            if status.is_success() {
                Ok(())
            } else {
                Err(Failure(format!("{} answered {status}", self.webhook.url)))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::{Router, routing::post};
    use std::sync::{Arc, Mutex};
    use tokio::net::TcpListener;

    type Received = Arc<Mutex<Vec<(String, String, String)>>>;

    async fn serve(status: StatusCode) -> (String, Received) {
        let received = Received::default();
        let app =
            Router::new()
                .route(
                    "/hook",
                    post(
                        move |State(received): State<Received>,
                              headers: HeaderMap,
                              body: String| async move {
                            let header = |name: &str| {
                                headers
                                    .get(name)
                                    .and_then(|value| value.to_str().ok())
                                    .unwrap_or_default()
                                    .to_owned()
                            };
                            received.lock().unwrap().push((
                                header("x-qobuzdlx-event"),
                                header("authorization"),
                                body,
                            ));
                            status
                        },
                    ),
                )
                .with_state(Arc::clone(&received));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/hook", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (url, received)
    }

    fn extension(url: String) -> WebhookExtension {
        let info = Info {
            name: "hook".into(),
            version: None,
            description: None,
            source: Source::Webhook,
            events: vec![],
        };
        let webhook = Webhook {
            url,
            headers: [("Authorization".to_owned(), "Bearer secret".to_owned())].into(),
        };
        WebhookExtension::new(info, webhook, Duration::from_secs(5)).unwrap()
    }

    #[tokio::test]
    async fn the_event_is_posted_with_its_name_and_the_configured_headers() {
        let (url, received) = serve(StatusCode::NO_CONTENT).await;

        extension(url).handle(&Event::QueueDrained).await.unwrap();

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].0, "queue_drained");
        assert_eq!(received[0].1, "Bearer secret");
        assert!(received[0].2.contains(r#""event":"queue_drained""#));
    }

    #[tokio::test]
    async fn an_error_answer_is_a_failure() {
        let (url, _) = serve(StatusCode::INTERNAL_SERVER_ERROR).await;

        let failure = extension(url)
            .handle(&Event::QueueDrained)
            .await
            .unwrap_err();

        assert!(failure.0.contains("500"), "{failure}");
    }

    #[tokio::test]
    async fn an_unreachable_address_is_a_failure() {
        let failure = extension("http://127.0.0.1:1/hook".into())
            .handle(&Event::QueueDrained)
            .await
            .unwrap_err();

        assert!(failure.0.contains("127.0.0.1"), "{failure}");
    }
}
