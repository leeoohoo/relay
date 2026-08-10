use super::*;

#[test]
fn rpc_stage_timeout_returns_an_actionable_error() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let (_writer, reader) = tokio::io::duplex(64);
    let mut reader = BufReader::new(reader);

    let error = runtime
        .block_on(wait_for_rpc_response_with_timeout(
            &mut reader,
            1,
            "thread/resume",
            Duration::from_millis(5),
        ))
        .expect_err("silent app-server must time out");

    assert!(error
        .to_string()
        .contains("timed out waiting for thread/resume response 1"));
}
