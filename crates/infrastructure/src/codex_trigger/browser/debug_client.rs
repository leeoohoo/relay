use super::*;
use std::{
    io::{Read, Write},
    net::TcpStream,
};

pub(super) fn browser_endpoint_ready(endpoint: &str) -> bool {
    let Some(port) = endpoint
        .rsplit(':')
        .next()
        .and_then(|value| value.parse().ok())
    else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(
        &(std::net::Ipv4Addr::LOCALHOST, port).into(),
        Duration::from_millis(250),
    ) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
    if stream
        .write_all(b"GET /json/version HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut response = [0_u8; 256];
    stream
        .read(&mut response)
        .ok()
        .is_some_and(|read| read > 0 && response[..read].starts_with(b"HTTP/1.1 200"))
}

pub(super) fn agent_browser_page_url(agent_id: Uuid) -> String {
    format!("about:blank#relay-agent-{agent_id}")
}

pub(super) fn browser_debug_json(endpoint: &str, method: &str, path: &str) -> AppResult<Value> {
    let body = browser_debug_request(endpoint, method, path)?;
    serde_json::from_slice(&body).map_err(|error| {
        AppError::Internal(format!("managed browser returned invalid JSON: {error}"))
    })
}

pub(super) fn close_browser_page(endpoint: &str, page_id: &str) -> AppResult<()> {
    validate_browser_page_id(page_id)?;
    browser_debug_request(endpoint, "GET", &format!("/json/close/{page_id}"))?;
    Ok(())
}

pub(super) fn validate_browser_page_id(page_id: &str) -> AppResult<()> {
    validate_safe_value(page_id, "managed browser page ID", 256)?;
    if page_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Ok(());
    }
    Err(AppError::Internal(
        "managed browser returned an invalid page ID".into(),
    ))
}

fn browser_debug_request(endpoint: &str, method: &str, path: &str) -> AppResult<Vec<u8>> {
    let port = endpoint
        .rsplit(':')
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| AppError::Internal("managed browser endpoint is invalid".into()))?;
    let mut stream = TcpStream::connect_timeout(
        &(std::net::Ipv4Addr::LOCALHOST, port).into(),
        Duration::from_secs(2),
    )
    .map_err(|error| AppError::Internal(format!("cannot connect to managed browser: {error}")))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| AppError::Internal(format!("cannot configure browser read: {error}")))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| AppError::Internal(format!("cannot configure browser write: {error}")))?;
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| AppError::Internal(format!("cannot request managed browser: {error}")))?;

    let mut response = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                response.extend_from_slice(&chunk[..read]);
                if response.len() as u64 > MAX_BROWSER_DEBUG_RESPONSE_BYTES {
                    return Err(AppError::Internal(
                        "managed browser response exceeded the safe limit".into(),
                    ));
                }
                if browser_http_response_is_complete(&response) {
                    break;
                }
            }
            Err(error)
                if !response.is_empty()
                    && matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
            {
                break;
            }
            Err(error) => {
                return Err(AppError::Internal(format!(
                    "cannot read managed browser: {error}"
                )))
            }
        }
    }
    let separator = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| AppError::Internal("managed browser returned invalid HTTP".into()))?;
    let headers = String::from_utf8_lossy(&response[..separator]);
    if !headers
        .lines()
        .next()
        .is_some_and(|line| line.contains(" 200 "))
    {
        return Err(AppError::Internal(format!(
            "managed browser request failed: {}",
            headers.lines().next().unwrap_or("unknown response")
        )));
    }
    Ok(response[separator + 4..].to_vec())
}

fn browser_http_response_is_complete(response: &[u8]) -> bool {
    let Some(separator) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let headers = String::from_utf8_lossy(&response[..separator]);
    let Some(content_length) = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    }) else {
        return false;
    };
    response.len().saturating_sub(separator + 4) >= content_length
}
