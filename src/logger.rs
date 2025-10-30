use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn init() {
    let tracing_subscriber = tracing_subscriber::registry();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt = tracing_subscriber::fmt::layer()
        .with_thread_ids(true)
        .with_target(false);
    tracing_subscriber.with(filter).with(fmt).init();
}
