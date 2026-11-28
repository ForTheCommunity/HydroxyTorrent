use std::{collections::BTreeMap, net::SocketAddr};

use hydroxy_bencode::{
    decode::{BencodedValue, decode},
    encode::encode,
};

use crate::{
    errors::DHTError,
    id::{
        NodeId, NodeInfo, compact_nodes_decode, compact_nodes_encode, compact_nodes6_decode,
        compact_nodes6_encode, compact_peer_decode, compact_peer_encode,
    },
};

/// BEP 0032 `"want"` query flags indicating requested address families (`"n4"` and/or `"n6"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WantFlags {
    pub n4: bool,
    pub n6: bool,
}

impl WantFlags {
    pub const BOTH: Self = Self { n4: true, n6: true };

    /// Per BEP 0032: when `"want"` is omitted, default to the address family of the UDP packet source.
    pub fn effective_for_source(&self, src: &SocketAddr) -> (bool, bool) {
        if !self.n4 && !self.n6 {
            (src.is_ipv4(), src.is_ipv6())
        } else {
            (self.n4, self.n6)
        }
    }

    fn to_bencoded(&self) -> Option<BencodedValue> {
        if !self.n4 && !self.n6 {
            return None;
        }
        let mut list = Vec::with_capacity(2);
        if self.n4 {
            list.push(BencodedValue::String(b"n4".to_vec()));
        }
        if self.n6 {
            list.push(BencodedValue::String(b"n6".to_vec()));
        }
        Some(BencodedValue::List(list))
    }

    fn from_args(args: &BTreeMap<Vec<u8>, BencodedValue>) -> Self {
        let mut flags = Self::default();
        if let Some(BencodedValue::List(items)) = args.get(b"want".as_slice()) {
            for item in items {
                if let BencodedValue::String(s) = item {
                    if s.as_slice() == b"n4" {
                        flags.n4 = true;
                    } else if s.as_slice() == b"n6" {
                        flags.n6 = true;
                    }
                }
            }
        }
        flags
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KRPCMessage {
    Query {
        transaction_id: Vec<u8>,
        query: QueryPayload,
    },
    Response {
        transaction_id: Vec<u8>,
        response: ResponsePayload,
    },
    Error {
        transaction_id: Vec<u8>,
        code: i64,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryPayload {
    Ping {
        id: NodeId,
    },
    FindNode {
        id: NodeId,
        target: NodeId,
        want: WantFlags,
    },
    GetPeers {
        id: NodeId,
        info_hash: NodeId,
        want: WantFlags,
    },
    AnnouncePeer {
        id: NodeId,
        info_hash: NodeId,
        port: u16,
        token: Vec<u8>,
        implied_port: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponsePayload {
    Ping {
        id: NodeId,
    },
    FindNode {
        id: NodeId,
        nodes: Vec<NodeInfo>,
        nodes6: Vec<NodeInfo>,
    },
    GetPeers {
        id: NodeId,
        token: Vec<u8>,
        peers: Vec<SocketAddr>,
        nodes: Vec<NodeInfo>,
        nodes6: Vec<NodeInfo>,
    },
    AnnouncePeer {
        id: NodeId,
    },
}

impl KRPCMessage {
    pub fn transaction_id(&self) -> &[u8] {
        match self {
            KRPCMessage::Query { transaction_id, .. } => transaction_id,
            KRPCMessage::Response { transaction_id, .. } => transaction_id,
            KRPCMessage::Error { transaction_id, .. } => transaction_id,
        }
    }

    /// KRPC Message Encode
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut map = BTreeMap::new();
        map.insert(
            b"t".to_vec(),
            BencodedValue::String(self.transaction_id().to_vec()),
        );

        match self {
            KRPCMessage::Query { query, .. } => {
                map.insert(b"y".to_vec(), BencodedValue::String(b"q".to_vec()));

                let mut args = BTreeMap::new();

                match query {
                    QueryPayload::Ping { id } => {
                        map.insert(b"q".to_vec(), BencodedValue::String(b"ping".to_vec()));
                        args.insert(b"id".to_vec(), BencodedValue::String(id.to_vec()));
                    }
                    QueryPayload::FindNode { id, target, want } => {
                        map.insert(b"q".to_vec(), BencodedValue::String(b"find_node".to_vec()));
                        args.insert(b"id".to_vec(), BencodedValue::String(id.to_vec()));
                        args.insert(b"target".to_vec(), BencodedValue::String(target.to_vec()));
                        if let Some(w) = want.to_bencoded() {
                            args.insert(b"want".to_vec(), w);
                        }
                    }
                    QueryPayload::GetPeers {
                        id,
                        info_hash,
                        want,
                    } => {
                        map.insert(b"q".to_vec(), BencodedValue::String(b"get_peers".to_vec()));
                        args.insert(b"id".to_vec(), BencodedValue::String(id.to_vec()));
                        args.insert(
                            b"info_hash".to_vec(),
                            BencodedValue::String(info_hash.to_vec()),
                        );
                        if let Some(w) = want.to_bencoded() {
                            args.insert(b"want".to_vec(), w);
                        }
                    }
                    QueryPayload::AnnouncePeer {
                        id,
                        info_hash,
                        port,
                        token,
                        implied_port,
                    } => {
                        map.insert(
                            b"q".to_vec(),
                            BencodedValue::String(b"announce_peer".to_vec()),
                        );
                        args.insert(b"id".to_vec(), BencodedValue::String(id.to_vec()));
                        args.insert(
                            b"info_hash".to_vec(),
                            BencodedValue::String(info_hash.to_vec()),
                        );
                        args.insert(b"port".to_vec(), BencodedValue::Integer(*port as i64));
                        args.insert(b"token".to_vec(), BencodedValue::String(token.clone()));
                        if *implied_port {
                            args.insert(b"implied_port".to_vec(), BencodedValue::Integer(1));
                        }
                    }
                }

                map.insert(b"a".to_vec(), BencodedValue::Dictionary(args));
            }

            KRPCMessage::Response { response, .. } => {
                map.insert(b"y".to_vec(), BencodedValue::String(b"r".to_vec()));
                let mut res = BTreeMap::new();

                match response {
                    ResponsePayload::Ping { id } => {
                        res.insert(b"id".to_vec(), BencodedValue::String(id.to_vec()));
                    }
                    ResponsePayload::FindNode { id, nodes, nodes6 } => {
                        res.insert(b"id".to_vec(), BencodedValue::String(id.to_vec()));
                        if !nodes.is_empty() || nodes6.is_empty() {
                            res.insert(
                                b"nodes".to_vec(),
                                BencodedValue::String(compact_nodes_encode(nodes)),
                            );
                        }
                        if !nodes6.is_empty() {
                            res.insert(
                                b"nodes6".to_vec(),
                                BencodedValue::String(compact_nodes6_encode(nodes6)),
                            );
                        }
                    }
                    ResponsePayload::GetPeers {
                        id,
                        token,
                        peers,
                        nodes,
                        nodes6,
                    } => {
                        res.insert(b"id".to_vec(), BencodedValue::String(id.to_vec()));
                        res.insert(b"token".to_vec(), BencodedValue::String(token.clone()));
                        if !peers.is_empty() {
                            let val_list = peers
                                .iter()
                                .map(|p| BencodedValue::String(compact_peer_encode(p)))
                                .collect();
                            res.insert(b"values".to_vec(), BencodedValue::List(val_list));
                        }
                        if !nodes.is_empty() {
                            res.insert(
                                b"nodes".to_vec(),
                                BencodedValue::String(compact_nodes_encode(nodes)),
                            );
                        }
                        if !nodes6.is_empty() {
                            res.insert(
                                b"nodes6".to_vec(),
                                BencodedValue::String(compact_nodes6_encode(nodes6)),
                            );
                        }
                    }
                    ResponsePayload::AnnouncePeer { id } => {
                        res.insert(b"id".to_vec(), BencodedValue::String(id.to_vec()));
                    }
                }
                map.insert(b"r".to_vec(), BencodedValue::Dictionary(res));
            }

            KRPCMessage::Error { code, message, .. } => {
                map.insert(b"y".to_vec(), BencodedValue::String(b"e".to_vec()));
                let err_list = vec![
                    BencodedValue::Integer(*code),
                    BencodedValue::String(message.as_bytes().to_vec()),
                ];
                map.insert(b"e".to_vec(), BencodedValue::List(err_list));
            }
        }

        encode(&BencodedValue::Dictionary(map))
    }

    /// KRPC Message Decode
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DHTError> {
        let bencoded_value = decode(bytes)?;
        let dict = match bencoded_value {
            BencodedValue::Dictionary(d) => d,
            _ => {
                return Err(DHTError::Protocol(
                    "Top-Level Value is not a dictionary".into(),
                ));
            }
        };

        let transaction_id = dict
            .get(b"t".as_slice())
            .and_then(|v| match v {
                BencodedValue::String(s) => Some(s.clone()),
                _ => None,
            })
            .ok_or_else(|| DHTError::Protocol("Missing or invalid 't' field".into()))?;

        let message_type = dict
            .get(b"y".as_slice())
            .and_then(|v| match v {
                BencodedValue::String(s) => Some(s.as_slice()),
                _ => None,
            })
            .ok_or_else(|| DHTError::Protocol("Missing or invalid 'y' field".into()))?;

        match message_type {
            b"q" => {
                let query_name = dict
                    .get(b"q".as_slice())
                    .and_then(|v| match v {
                        BencodedValue::String(s) => Some(s.as_slice()),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        DHTError::Protocol("Missing or invalid 'q' field in query".into())
                    })?;

                let args = dict
                    .get(b"a".as_slice())
                    .and_then(|v| match v {
                        BencodedValue::Dictionary(d) => Some(d),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        DHTError::Protocol("Missing or invalid 'a' dict in query".into())
                    })?;

                let id = get_node_id(args, b"id").ok_or_else(|| {
                    DHTError::Protocol("Missing or invalid sender 'id' in query".into())
                })?;

                let query = match query_name {
                    b"ping" => QueryPayload::Ping { id },
                    b"find_node" => {
                        let target = get_node_id(args, b"target").ok_or_else(|| {
                            DHTError::Protocol("Missing 'target' in find_node".into())
                        })?;
                        let want = WantFlags::from_args(args);
                        QueryPayload::FindNode { id, target, want }
                    }
                    b"get_peers" => {
                        let info_hash = get_node_id(args, b"info_hash").ok_or_else(|| {
                            DHTError::Protocol("Missing 'info_hash' in get_peers".into())
                        })?;
                        let want = WantFlags::from_args(args);
                        QueryPayload::GetPeers {
                            id,
                            info_hash,
                            want,
                        }
                    }
                    b"announce_peer" => {
                        let info_hash = get_node_id(args, b"info_hash").ok_or_else(|| {
                            DHTError::Protocol("Missing 'info_hash' in announce_peer".into())
                        })?;
                        let port = args
                            .get(b"port".as_slice())
                            .and_then(|v| match v {
                                BencodedValue::Integer(i) => Some(*i as u16),
                                _ => None,
                            })
                            .unwrap_or(0);
                        let token = args
                            .get(b"token".as_slice())
                            .and_then(|v| match v {
                                BencodedValue::String(s) => Some(s.clone()),
                                _ => None,
                            })
                            .ok_or_else(|| {
                                DHTError::Protocol("Missing 'token' in announce_peer".into())
                            })?;
                        let implied_port = args
                            .get(b"implied_port".as_slice())
                            .and_then(|v| match v {
                                BencodedValue::Integer(i) => Some(*i != 0),
                                _ => None,
                            })
                            .unwrap_or(false);

                        QueryPayload::AnnouncePeer {
                            id,
                            info_hash,
                            port,
                            token,
                            implied_port,
                        }
                    }
                    other => {
                        return Err(DHTError::Protocol(format!(
                            "Unknown query method: {:?}",
                            String::from_utf8_lossy(other)
                        )));
                    }
                };

                Ok(KRPCMessage::Query {
                    transaction_id,
                    query,
                })
            }

            b"r" => {
                let res = dict
                    .get(b"r".as_slice())
                    .and_then(|v| match v {
                        BencodedValue::Dictionary(d) => Some(d),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        DHTError::Protocol("Missing or invalid 'r' dict in response".into())
                    })?;

                let id = get_node_id(res, b"id").ok_or_else(|| {
                    DHTError::Protocol("Missing or invalid 'id' in response".into())
                })?;

                let token = res.get(b"token".as_slice()).and_then(|v| match v {
                    BencodedValue::String(s) => Some(s.clone()),
                    _ => None,
                });

                let peers = res
                    .get(b"values".as_slice())
                    .and_then(|v| match v {
                        BencodedValue::List(l) => {
                            let mut peers = Vec::new();
                            for item in l {
                                if let BencodedValue::String(s) = item {
                                    if let Some(addr) = compact_peer_decode(s) {
                                        peers.push(addr);
                                    }
                                }
                            }
                            Some(peers)
                        }
                        _ => None,
                    })
                    .unwrap_or_default();

                let has_nodes_key =
                    res.contains_key(b"nodes".as_slice()) || res.contains_key(b"nodes6".as_slice());

                let nodes = res
                    .get(b"nodes".as_slice())
                    .and_then(|v| match v {
                        BencodedValue::String(s) => compact_nodes_decode(s).ok(),
                        _ => None,
                    })
                    .unwrap_or_default();

                let nodes6 = res
                    .get(b"nodes6".as_slice())
                    .and_then(|v| match v {
                        BencodedValue::String(s) => compact_nodes6_decode(s).ok(),
                        _ => None,
                    })
                    .unwrap_or_default();

                let response = if let Some(token) = token {
                    ResponsePayload::GetPeers {
                        id,
                        token,
                        peers,
                        nodes,
                        nodes6,
                    }
                } else if has_nodes_key || !nodes.is_empty() || !nodes6.is_empty() {
                    ResponsePayload::FindNode { id, nodes, nodes6 }
                } else {
                    ResponsePayload::Ping { id }
                };

                Ok(KRPCMessage::Response {
                    transaction_id,
                    response,
                })
            }

            b"e" => {
                let err_list = dict
                    .get(b"e".as_slice())
                    .and_then(|v| match v {
                        BencodedValue::List(l) => Some(l),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        DHTError::Protocol("Missing or invalid 'e' list in error".into())
                    })?;

                if err_list.len() < 2 {
                    return Err(DHTError::Protocol(
                        "Error list must contain at least 2 elements".into(),
                    ));
                }

                let code = match &err_list[0] {
                    BencodedValue::Integer(i) => *i,
                    _ => 201,
                };

                let message = match &err_list[1] {
                    BencodedValue::String(s) => String::from_utf8_lossy(s).to_string(),
                    _ => "Unknown error".to_string(),
                };

                Ok(KRPCMessage::Error {
                    transaction_id,
                    code,
                    message,
                })
            }

            other => Err(DHTError::Protocol(format!(
                "Unknown message type 'y': {:?}",
                String::from_utf8_lossy(other)
            ))),
        }
    }
}

fn get_node_id(dict: &BTreeMap<Vec<u8>, BencodedValue>, key: &[u8]) -> Option<NodeId> {
    dict.get(key).and_then(|v| match v {
        BencodedValue::String(s) if s.len() == 20 => {
            let mut id = [0u8; 20];
            id.copy_from_slice(s);
            Some(id)
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};

    #[test]
    fn test_ping_query_and_response_roundtrip() {
        let msg = KRPCMessage::Query {
            transaction_id: b"aa".to_vec(),
            query: QueryPayload::Ping { id: [1u8; 20] },
        };
        let encoded = msg.to_bytes();
        let decoded = KRPCMessage::from_bytes(&encoded).unwrap();
        assert_eq!(msg, decoded);

        let resp = KRPCMessage::Response {
            transaction_id: b"aa".to_vec(),
            response: ResponsePayload::Ping { id: [2u8; 20] },
        };
        let encoded_resp = resp.to_bytes();
        let decoded_resp = KRPCMessage::from_bytes(&encoded_resp).unwrap();
        assert_eq!(resp, decoded_resp);
    }

    #[test]
    fn test_bep0032_find_node_and_get_peers_dual_stack_roundtrip() {
        let msg = KRPCMessage::Query {
            transaction_id: b"t1".to_vec(),
            query: QueryPayload::FindNode {
                id: [1u8; 20],
                target: [2u8; 20],
                want: WantFlags::BOTH,
            },
        };
        let encoded = msg.to_bytes();
        let decoded = KRPCMessage::from_bytes(&encoded).unwrap();
        assert_eq!(msg, decoded);

        let v6_ip: Ipv6Addr = "2406:9cc0:200:1521::77".parse().unwrap();
        let resp = KRPCMessage::Response {
            transaction_id: b"t1".to_vec(),
            response: ResponsePayload::FindNode {
                id: [3u8; 20],
                nodes: vec![NodeInfo::new_v4(
                    [4u8; 20],
                    SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 6881),
                )],
                nodes6: vec![NodeInfo::new_v6(
                    [5u8; 20],
                    SocketAddrV6::new(v6_ip, 6881, 0, 0),
                )],
            },
        };
        let encoded_resp = resp.to_bytes();
        let decoded_resp = KRPCMessage::from_bytes(&encoded_resp).unwrap();
        assert_eq!(resp, decoded_resp);

        let gp_resp = KRPCMessage::Response {
            transaction_id: b"gp".to_vec(),
            response: ResponsePayload::GetPeers {
                id: [2u8; 20],
                token: b"secret_tok".to_vec(),
                peers: vec![
                    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 50), 51413)),
                    SocketAddr::V6(SocketAddrV6::new(v6_ip, 51413, 0, 0)),
                ],
                nodes: vec![],
                nodes6: vec![NodeInfo::new_v6(
                    [6u8; 20],
                    SocketAddrV6::new(v6_ip, 9999, 0, 0),
                )],
            },
        };
        let gp_bytes = gp_resp.to_bytes();
        let gp_decoded = KRPCMessage::from_bytes(&gp_bytes).unwrap();
        assert_eq!(gp_resp, gp_decoded);
    }

    #[test]
    fn test_announce_peer_and_error_roundtrip() {
        let msg = KRPCMessage::Query {
            transaction_id: b"ap".to_vec(),
            query: QueryPayload::AnnouncePeer {
                id: [1u8; 20],
                info_hash: [9u8; 20],
                port: 6881,
                token: b"secret_tok".to_vec(),
                implied_port: true,
            },
        };
        let encoded = msg.to_bytes();
        let decoded = KRPCMessage::from_bytes(&encoded).unwrap();
        assert_eq!(msg, decoded);

        let err = KRPCMessage::Error {
            transaction_id: b"err".to_vec(),
            code: 203,
            message: "Protocol Error".to_string(),
        };
        let encoded_err = err.to_bytes();
        assert_eq!(KRPCMessage::from_bytes(&encoded_err).unwrap(), err);
    }
}
