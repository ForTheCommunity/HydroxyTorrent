use hydroxy_bencode::decode::{BencodedValue, decode};

use crate::{Peer, Tracker, TrackerEvents, TrackerResponse, errors::TrackerError};

pub async fn announce(tracker: &Tracker) -> Result<TrackerResponse, TrackerError> {
    let mut url = format!(
        "{}?info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact=1",
        tracker.announce_url,
        urlencoding::encode_binary(&tracker.info_hash).to_string(),
        urlencoding::encode_binary(&tracker.peer_id).to_string(),
        tracker.port,
        tracker.uploaded,
        tracker.downloaded,
        tracker.remaining_size,
    );

    if let Some(ref event) = tracker.event {
        let event_str = match event {
            TrackerEvents::Started => "started",
            TrackerEvents::Completed => "completed",
            TrackerEvents::Stopped => "stopped",
        };
        url.push_str("&event=");
        url.push_str(event_str);
    }

    println!("GENERATED TRACKER ANNOUNCE URL -> {}", url);

    let response = reqwest::get(url)
        .await
        .map_err(|e| TrackerError::NetworkError(e.to_string()))?;

    if !response.status().is_success() && response.status() != reqwest::StatusCode::OK {
        return Err(TrackerError::NetworkError(format!(
            "HTTP Error: {}",
            response.status()
        )));
    }

    let response_bytes = response
        .bytes()
        .await
        .map_err(|e| TrackerError::TrackerResponseParseError(e.to_string()))?;

    let decoded_response = decode(&response_bytes).map_err(|e| TrackerError::DecodeError(e))?;

    let tracker_response = convert(&decoded_response)
        .map_err(|e| TrackerError::TrackerResponseParseError(e.to_string()))?;

    Ok(tracker_response)
}

pub fn convert(decoded_tracker_response: &BencodedValue) -> Result<TrackerResponse, TrackerError> {
    let tracker_response = if let BencodedValue::Dictionary(dict) = decoded_tracker_response {
        // completed peers count.
        let complete = match dict.get(&b"complete"[..]) {
            Some(BencodedValue::Integer(completed_peers_count)) => *completed_peers_count as usize,
            _ => 0,
        };

        // downloaded peers count.
        let downloaded = match dict.get(&b"downloaded"[..]) {
            Some(BencodedValue::Integer(downloaded_peers_count)) => {
                *downloaded_peers_count as usize
            }
            _ => 0,
        };

        // incomplete peers count.
        let incomplete = match dict.get(&b"incomplete"[..]) {
            Some(BencodedValue::Integer(incomplete_peers_count)) => {
                *incomplete_peers_count as usize
            }
            _ => 0,
        };

        // interval
        let interval = match dict.get(&b"interval"[..]) {
            Some(BencodedValue::Integer(interval)) => *interval as usize,
            _ => 0,
        };

        // min interval
        let min_interval = match dict.get(&b"min interval"[..]) {
            Some(BencodedValue::Integer(min_interval)) => *min_interval as usize,
            _ => 0,
        };

        // IPV4 Peers
        let ipv4_peers = match dict.get(&b"peers"[..]) {
            Some(BencodedValue::String(bytes)) => Peer::parse_peers(&bytes),
            _ => Vec::new(),
        };

        Ok(TrackerResponse {
            complete,
            downloaded,
            incomplete,
            interval,
            min_interval,
            ipv4_peers,
            // not implemented for IPV4 peers.
            ipv6_peers: Vec::new(),
        })
    } else {
        Err(TrackerError::TrackerResponseParseError(
            "Error Invalid Tracker Response. !!!!".into(),
        ))
    };
    tracker_response
}
