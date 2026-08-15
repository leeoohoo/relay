use std::thread;
use std::time::Duration;

use fallible_iterator::FallibleIterator;
use postgres::{Client, NoTls};
use rand::Rng;
use tokio::sync::broadcast;

use ai_chat_domain::company::CompanyRealtimeSignal;

const REALTIME_CHANNEL: &str = "ai_chat_realtime_events";

pub fn spawn_postgres_realtime_listener(
    database_url: String,
    sender: broadcast::Sender<CompanyRealtimeSignal>,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("ai-chat-pg-realtime-listener".into())
        .spawn(move || {
            let mut consecutive_failures = 0u32;
            loop {
                match listen_once(&database_url, &sender) {
                    Ok(()) => {
                        consecutive_failures = 0;
                        thread::sleep(Duration::from_millis(250));
                    }
                    Err(error) => {
                        consecutive_failures = consecutive_failures.saturating_add(1);
                        let delay = reconnect_delay(consecutive_failures);
                        tracing::warn!(
                            %error,
                            consecutive_failures,
                            reconnect_delay_ms = delay.as_millis(),
                            "PostgreSQL realtime listener disconnected; reconnecting"
                        );
                        thread::sleep(delay);
                    }
                }
            }
        })
        .expect("failed to spawn PostgreSQL realtime listener")
}

fn reconnect_delay(consecutive_failures: u32) -> Duration {
    let exponent = consecutive_failures.saturating_sub(1).min(5);
    let base_ms = 1_000u64.saturating_mul(1u64 << exponent).min(30_000);
    let jitter_percent = rand::thread_rng().gen_range(90..=110);
    Duration::from_millis(base_ms.saturating_mul(jitter_percent) / 100)
}

fn listen_once(
    database_url: &str,
    sender: &broadcast::Sender<CompanyRealtimeSignal>,
) -> anyhow::Result<()> {
    let mut client = Client::connect(database_url, NoTls)?;
    client.batch_execute(&format!("LISTEN {REALTIME_CHANNEL}"))?;
    let mut notifications = client.notifications();
    let mut iterator = notifications.blocking_iter();
    while let Some(notification) = iterator.next()? {
        let signal = serde_json::from_str::<CompanyRealtimeSignal>(notification.payload())?;
        let _ = sender.send(signal);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_delay_is_bounded_and_increases() {
        assert!(reconnect_delay(2) > reconnect_delay(1));
        assert!(reconnect_delay(100) <= Duration::from_secs(33));
    }
}
