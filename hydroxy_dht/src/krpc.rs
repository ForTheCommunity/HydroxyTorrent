use std::{collections::BTreeMap, net::SocketAddrV4};

use hydroxy_bencode::{decode::BencodedValue, encode::encode};

use crate::id::{NodeId, NodeInfo, compact_nodes_encode, compact_peer_encode};

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
    },
    GetPeers {
        id: NodeId,
        info_hash: NodeId,
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
    },
    GetPeers {
        id: NodeId,
        token: Vec<u8>,
        peers: Vec<SocketAddrV4>,
        nodes: Vec<NodeInfo>,
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
                    QueryPayload::FindNode { id, target } => {
                        map.insert(b"q".to_vec(), BencodedValue::String(b"find_node".to_vec()));
                        args.insert(b"id".to_vec(), BencodedValue::String(id.to_vec()));
                        args.insert(b"target".to_vec(), BencodedValue::String(target.to_vec()));
                    }
                    QueryPayload::GetPeers { id, info_hash } => {
                        map.insert(b"q".to_vec(), BencodedValue::String(b"get_peers".to_vec()));
                        args.insert(b"id".to_vec(), BencodedValue::String(id.to_vec()));
                        args.insert(
                            b"info_hash".to_vec(),
                            BencodedValue::String(info_hash.to_vec()),
                        );
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
                    ResponsePayload::FindNode { id, nodes } => {
                        res.insert(b"id".to_vec(), BencodedValue::String(id.to_vec()));
                        res.insert(
                            b"nodes".to_vec(),
                            BencodedValue::String(compact_nodes_encode(nodes)),
                        );
                    }
                    ResponsePayload::GetPeers {
                        id,
                        token,
                        peers,
                        nodes,
                    } => {
                        res.insert(b"id".to_vec(), BencodedValue::String(id.to_vec()));
                        res.insert(b"token".to_vec(), BencodedValue::String(token.clone()));
                        if !peers.is_empty() {
                            let val_list = peers
                                .iter()
                                .map(|p| BencodedValue::String(compact_peer_encode(p).to_vec()))
                                .collect();
                            res.insert(b"values".to_vec(), BencodedValue::List(val_list));
                        }
                        if !nodes.is_empty() {
                            res.insert(
                                b"nodes".to_vec(),
                                BencodedValue::String(compact_nodes_encode(nodes)),
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
}
