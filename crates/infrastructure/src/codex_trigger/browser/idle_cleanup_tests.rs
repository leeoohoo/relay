use super::*;

#[test]
fn idle_browser_cleanup_keeps_recent_or_active_profiles() {
    let mut config = BrowserMcpConfig::disabled();
    config.host_browser_idle_timeout = Duration::from_secs(60);
    let now = std::time::Instant::now();
    let active_agent_id = Uuid::new_v4();
    {
        let mut pool = config.pool.lock().expect("browser pool");
        *pool = Some(host_browser(
            now - Duration::from_secs(61),
            [(active_agent_id, now - Duration::from_secs(61))],
        ));
    }

    assert_eq!(
        config
            .prune_idle_host_browsers(&HashSet::from([active_agent_id]))
            .expect("prune"),
        (0, 0)
    );
    assert!(config.pool.lock().expect("browser pool").is_some());
    assert_eq!(
        config
            .prune_idle_host_browsers(&HashSet::new())
            .expect("prune"),
        (1, 0)
    );
    assert!(config.pool.lock().expect("browser pool").is_none());
}

#[test]
fn idle_page_selection_keeps_recent_and_active_agent_pages() {
    let now = std::time::Instant::now();
    let expired_agent = Uuid::new_v4();
    let active_agent = Uuid::new_v4();
    let recent_agent = Uuid::new_v4();
    let process = host_browser(
        now,
        [
            (expired_agent, now - Duration::from_secs(61)),
            (active_agent, now - Duration::from_secs(61)),
            (recent_agent, now - Duration::from_secs(10)),
        ],
    );

    let expired_pages = expired_agent_pages(
        &process,
        &HashSet::from([active_agent]),
        now,
        Duration::from_secs(60),
    );

    assert_eq!(
        expired_pages,
        vec![(expired_agent, expired_agent.simple().to_string())]
    );
    assert!(process.agent_pages.contains_key(&expired_agent));
    assert!(process.agent_pages.contains_key(&active_agent));
    assert!(process.agent_pages.contains_key(&recent_agent));
}

fn host_browser<const N: usize>(
    last_used_at: std::time::Instant,
    pages: [(Uuid, std::time::Instant); N],
) -> HostBrowserProcess {
    HostBrowserProcess {
        endpoint: "http://127.0.0.1:19001".into(),
        child: None,
        last_used_at,
        agent_pages: pages
            .into_iter()
            .map(|(agent_id, last_used_at)| {
                (
                    agent_id,
                    AgentBrowserPage {
                        page_id: agent_id.simple().to_string(),
                        last_used_at,
                    },
                )
            })
            .collect(),
    }
}
