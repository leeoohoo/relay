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
            .prune_idle_host_browsers(&HashSet::from([active_agent_id]), false)
            .expect("prune"),
        (0, 0)
    );
    assert!(config.pool.lock().expect("browser pool").is_some());
    assert_eq!(
        config
            .prune_idle_host_browsers(&HashSet::new(), false)
            .expect("prune"),
        (1, 0)
    );
    assert!(config.pool.lock().expect("browser pool").is_none());
}

#[test]
fn aggressive_cleanup_reclaims_inactive_browser_without_waiting() {
    let config = BrowserMcpConfig::disabled();
    let now = std::time::Instant::now();
    {
        let mut pool = config.pool.lock().expect("browser pool");
        *pool = Some(host_browser(now, []));
    }

    assert_eq!(
        config
            .prune_idle_host_browsers(&HashSet::new(), true)
            .expect("aggressive prune"),
        (1, 0)
    );
    assert!(config.pool.lock().expect("browser pool").is_none());
}

#[test]
fn first_agent_reuses_the_browser_initial_page() {
    let root = std::env::temp_dir().join(format!(
        "relay-browser-initial-page-test-{}",
        Uuid::new_v4().simple()
    ));
    let Some(config) = BrowserMcpConfig::for_test_host(root.clone()) else {
        return;
    };
    let agent_id = Uuid::new_v4();
    let endpoint = config
        .ensure_host_browser(agent_id)
        .expect("host browser should start");
    let page_id = config
        .ensure_agent_browser_page(&endpoint, agent_id)
        .expect("first Agent page");
    let pages = browser_debug_json(&endpoint, "GET", "/json/list")
        .expect("browser page list")
        .as_array()
        .cloned()
        .expect("page array");

    assert_eq!(pages.len(), 1);
    assert_eq!(
        pages[0].get("id").and_then(Value::as_str),
        Some(page_id.as_str())
    );
    assert_eq!(
        pages[0].get("url").and_then(Value::as_str),
        Some(agent_browser_page_url(agent_id).as_str())
    );

    drop(config);
    std::fs::remove_dir_all(root).expect("cleanup browser profile");
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
        proxy: None,
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
