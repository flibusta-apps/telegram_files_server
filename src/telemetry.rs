use tracing_subscriber::{filter::LevelFilter, Layer};

/// Keep ordinary logs local: only ERROR events are reported to Sentry.
pub fn error_layer<S>() -> impl Layer<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    sentry_tracing::ErrorLayer::new().with_filter(LevelFilter::ERROR)
}

/// The legacy combined layer captured INFO/WARN/ERROR spans, independently of events.
pub fn span_layer<S>() -> impl Layer<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    sentry_tracing::SpanLayer::new().with_filter(tracing_subscriber::filter::LevelFilter::INFO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn preserves_info_spans_and_error_trace_context_but_not_debug_spans() {
        let options = sentry::ClientOptions::default().traces_sample_rate(1.0);
        let envelopes = sentry::test::with_captured_envelopes_options(
            || {
                let subscriber = tracing_subscriber::registry()
                    .with(span_layer())
                    .with(error_layer());
                tracing::subscriber::with_default(subscriber, || {
                    let root = tracing::info_span!("request");
                    let _root = root.enter();
                    {
                        let info = tracing::info_span!("retained_info");
                        let _info = info.enter();
                        let debug = tracing::debug_span!("excluded_debug");
                        let _debug = debug.enter();
                        tracing::error!("linked error");
                    }
                });
            },
            options,
        );
        let items: Vec<_> = envelopes.into_iter().flat_map(|e| e.into_items()).collect();
        let transaction = items
            .iter()
            .find_map(|item| match item {
                sentry::protocol::EnvelopeItem::Transaction(t) => Some(t),
                _ => None,
            })
            .expect("INFO root transaction captured");
        assert!(transaction
            .spans
            .iter()
            .any(|s| s.description.as_deref() == Some("retained_info")));
        assert!(!transaction
            .spans
            .iter()
            .any(|s| s.description.as_deref() == Some("excluded_debug")));
        let event = items
            .iter()
            .find_map(|item| match item {
                sentry::protocol::EnvelopeItem::Event(e) => Some(e),
                _ => None,
            })
            .expect("error event captured");
        let sentry::protocol::Context::Trace(event_trace) = &event.contexts["trace"] else {
            panic!("error trace missing")
        };
        let sentry::protocol::Context::Trace(transaction_trace) = &transaction.contexts["trace"]
        else {
            panic!("transaction trace missing")
        };
        assert_eq!(event_trace.trace_id, transaction_trace.trace_id);
        assert!(event.breadcrumbs.is_empty());
    }

    #[test]
    fn captures_only_errors_on_the_active_sentry_hub() {
        let events = sentry::test::with_captured_events(|| {
            let subscriber = tracing_subscriber::registry()
                .with(span_layer())
                .with(error_layer());
            tracing::subscriber::with_default(subscriber, || {
                tracing::trace!("not reported");
                tracing::debug!("not reported");
                tracing::info!("not reported");
                tracing::warn!("not reported");
                tracing::error!(answer = 42, "dependency migration capture");
            });
        });
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].level, sentry::Level::Error);
        assert_eq!(
            events[0].message.as_deref(),
            Some("dependency migration capture")
        );
        assert!(events[0].breadcrumbs.is_empty());
    }
}
