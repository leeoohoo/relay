use std::thread;
use std::time::Duration;

use fallible_iterator::FallibleIterator;
use postgres::{Client, NoTls};
use tokio::sync::broadcast;

use ai_chat_domain::company::CompanyRealtimeSignal;

const REALTIME_CHANNEL: &str = "ai_chat_realtime_events";

pub fn spawn_postgres_realtime_listener(
    database_url: String,
    sender: broadcast::Sender<CompanyRealtimeSignal>,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("ai-chat-pg-realtime-listener".into())
        .spawn(move || loop {
            let _ = listen_once(&database_url, &sender);
            thread::sleep(Duration::from_secs(1));
        })
        .expect("failed to spawn PostgreSQL realtime listener")
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
