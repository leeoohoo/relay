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
    let page = config
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
        Some(page.page_id.as_str())
    );
    assert_eq!(
        pages[0].get("url").and_then(Value::as_str),
        Some(agent_browser_page_url(agent_id).as_str())
    );

    drop(config);
    std::fs::remove_dir_all(root).expect("cleanup browser profile");
}

#[test]
fn host_browser_reclaims_lru_tabs_and_recreates_them_on_demand() {
    let root = std::env::temp_dir().join(format!(
        "relay-browser-lru-page-test-{}",
        Uuid::new_v4().simple()
    ));
    let Some(mut config) = BrowserMcpConfig::for_test_host(root.clone()) else {
        return;
    };
    config.host_max_idle_pages = 2;
    let agents = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let endpoint = config
        .ensure_host_browser(agents[0])
        .expect("host browser should start");
    for agent_id in agents {
        config
            .ensure_agent_browser_page(&endpoint, agent_id)
            .expect("Agent page");
        std::thread::sleep(Duration::from_millis(2));
    }

    assert_eq!(
        config
            .prune_idle_host_browsers(&HashSet::new(), false)
            .expect("LRU prune"),
        (0, 1)
    );
    assert_eq!(
        config.host_page_urls_for_test().expect("page URLs").len(),
        2
    );

    let restored = config
        .ensure_agent_browser_page(&endpoint, agents[0])
        .expect("recreated Agent page");
    assert!(!restored.page_id.is_empty());
    assert_eq!(
        config.host_page_urls_for_test().expect("page URLs").len(),
        3
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

    let expired_pages = reclaimable_agent_pages(
        &process,
        &HashSet::from([active_agent]),
        now,
        Duration::from_secs(60),
        10,
    );

    assert_eq!(
        expired_pages,
        vec![(expired_agent, expired_agent.simple().to_string())]
    );
    assert!(process.agent_pages.contains_key(&expired_agent));
    assert!(process.agent_pages.contains_key(&active_agent));
    assert!(process.agent_pages.contains_key(&recent_agent));
}

#[test]
fn idle_page_selection_keeps_only_the_most_recent_inactive_pages() {
    let now = std::time::Instant::now();
    let oldest_agent = Uuid::new_v4();
    let middle_agent = Uuid::new_v4();
    let recent_agent = Uuid::new_v4();
    let process = host_browser(
        now,
        [
            (oldest_agent, now - Duration::from_secs(30)),
            (middle_agent, now - Duration::from_secs(20)),
            (recent_agent, now - Duration::from_secs(10)),
        ],
    );

    assert_eq!(
        reclaimable_agent_pages(&process, &HashSet::new(), now, Duration::from_secs(60), 2,),
        vec![(oldest_agent, oldest_agent.simple().to_string())]
    );
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
                        last_used_at: Arc::new(Mutex::new(last_used_at)),
                    },
                )
            })
            .collect(),
    }
}
