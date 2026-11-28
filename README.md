# HydroxyTorrent

**HydroxyTorrent** is a high-performance, modular BitTorrent client and background daemon written in **Rust** (Tokio async runtime) paired with a reactive remote controller and Web UI built with **Flutter** (Mobile, Web & Desktop).


---

## Table of Contents

- [Disclaimer](#disclaimer)
- [BEP (BitTorrent Enhancement Proposal) Support](#bep-bittorrent-enhancement-proposal-support)
  - [Implemented BEPs](#implemented-beps)
  - [Not Implemented / Planned (TODO) BEPs](#not-implemented--planned-todo-beps)
- [License](#license)

---

## Disclaimer

> **⚠️ Warning: This software is under active development.**
>
> HydroxyTorrent is created for educational and experimental purposes. Components are actively being refined. Do not rely on this client as your sole daily driver for mission-critical transfers.

---

## BEP (BitTorrent Enhancement Proposal) Support

### Implemented BEPs

| BEP | Title | Crate / Component | Description |
|:---|:---|:---|:---|
| **[BEP 0003](https://www.bittorrent.org/beps/bep_0003.html)** | The BitTorrent Protocol Specification | `hydroxy_bencode`<br>`hydroxy_pwp`<br>`hydroxy_storage_io`<br>`hydroxy_tracker` | **Core Specification**: Bencoding encoding/decoding, metainfo `.torrent` file parsing, peer wire protocol framing (Handshake, Choke, Unchoke, Interested, Not Interested, Have, Bitfield, Request, Piece, Cancel), single/multi-file disk layout, and HTTP tracker announces. |
| **[BEP 0005](https://www.bittorrent.org/beps/bep_0005.html)** | DHT Protocol (Distributed Hash Table) | `hydroxy_dht`<br>`hydroxy_pwp`<br>`hydroxy_engine` | Trackerless peer discovery using Kademlia routing tables, KRPC (`ping`, `find_node`, `get_peers`, `announce_peer`), iterative lookups, token management, and PWP `Port` (`0x09`) negotiation. |
| **[BEP 0006](https://www.bittorrent.org/beps/bep_0006.html)** | Fast Extension | `hydroxy_pwp`<br>`hydroxy_engine` | Optimized peer negotiation: handshake reserved bit (`0x04`), `Have All`, `Have None`, `Suggest Piece`, `Reject Request`, and canonical `/24`-masked (IPv4) / `/48`-masked (IPv6) SHA-1 `Allowed Fast` piece set generation and choked downloading. |
| **[BEP 0007](https://www.bittorrent.org/beps/bep_0007.html)** | IPv6 Tracker Extension | `hydroxy_tracker`<br>`hydroxy_engine` | Dual-stack IPv6 swarm support: compact 18-byte `peers6` parsing for HTTP/UDP/WebSocket trackers, `&ipv6=` global unicast announce parameter, dual-stack inbound peer listener, and IPv6 peer workers. |
| **[BEP 0010](https://www.bittorrent.org/beps/bep_0010.html)** | Extension Protocol | `hydroxy_pwp`<br>`hydroxy_engine` | Extended message framing (`extended`, ID 20), extended handshake (`id = 0`), client version (`v`) exchange, and dynamic extension ID mapping. |
| **[BEP 0011](https://www.bittorrent.org/beps/bep_0011.html)** | Peer Exchange (PEX) | `hydroxy_engine` | Direct swarm peer discovery between connected peers (`ut_pex`) advertising added and dropped compact IPv4 (`added`/`dropped`) and IPv6 (`added6`/`dropped6`) endpoints. |
| **[BEP 0012](https://www.bittorrent.org/beps/bep_0012.html)** | Multitracker Metadata Extension | `hydroxy_tracker`<br>`hydroxy_utils` | Tier-based tracker configuration (`announce-list`), tier shuffling, fallback rotation, and multitracker announcement. |
| **[BEP 0015](https://www.bittorrent.org/beps/bep_0015.html)** | UDP Tracker Protocol | `hydroxy_tracker` | Low-overhead binary tracker communication over UDP: connection ID negotiation, transaction ID tracking, IPv4 announce requests, and error responses. |
| **[BEP 0020](https://www.bittorrent.org/beps/bep_0020.html)** | Peer ID Conventions | `hydroxy_utils` | Standardized Azureus-style client Peer ID formatting (`-HY0001-...`) and parsing logic for popular client identification. |
| **[BEP 0023](https://www.bittorrent.org/beps/bep_0023.html)** | Compact Peer List Format | `hydroxy_tracker` | Compact 6-byte IPv4 peer list parsing (4-byte IP + 2-byte port in network byte order) for HTTP and UDP trackers. |
| **[BEP 0027](https://www.bittorrent.org/beps/bep_0027.html)** | Private Torrents | `hydroxy_utils`<br>`hydroxy_engine` | Enforcing `info.private == 1` by disabling decentralized peer discovery (DHT announces, `Port` exchange, and PEX). |
| **[BEP 0032](https://www.bittorrent.org/beps/bep_0032.html)** | IPv6 Extension for DHT | `hydroxy_dht`<br>`hydroxy_engine` | Dual-stack DHT with separate IPv4/IPv6 Kademlia routing tables, `"want"` (`"n4"`/`"n6"`) negotiation, 38-byte `nodes6` encoding/decoding, and 18-byte IPv6 peer `values`. |
| **[BEP 0048](https://www.bittorrent.org/beps/bep_0048.html)** | Tracker Protocol Extension: Scrape | `hydroxy_tracker` | Scrape requests over HTTP (`/scrape`) and UDP (action `2`) to fetch seeder, leecher, and downloaded statistics. |
| **WebSocket Trackers** | WebTorrent Tracker Extension | `hydroxy_tracker` | `ws://` and `wss://` JSON-RPC tracker exchange for browser and WebTorrent swarms. |

---

### Not Implemented / Planned (TODO) BEPs

The following BEPs are planned or currently in research for future releases:

| BEP | Title | Target Component | Description / Notes |
|:---|:---|:---|:---|
| **[BEP 0009](https://www.bittorrent.org/beps/bep_0009.html)** | Extension for Peers to Send Metadata Files | `hydroxy_engine` | Magnet link resolution (`magnet:?xt=urn:btih:...`) and peer-to-peer `.torrent` metainfo transfer (`ut_metadata`). |
| **[BEP 0014](https://www.bittorrent.org/beps/bep_0014.html)** | Local Peer Discovery (LPD / LSD) | `hydroxy_engine` | Multicast peer discovery over local subnets (`239.192.152.143:6771`). |
| **[BEP 0017](https://www.bittorrent.org/beps/bep_0017.html)** | HTTP Seeding (Hoffman-style) | `hydroxy_storage_io` | Web-seeding fallback using HTTP servers for metainfo file segments. |
| **[BEP 0019](https://www.bittorrent.org/beps/bep_0019.html)** | WebSeed (GetRight-style) | `hydroxy_engine` | Direct web-seeding using standard HTTP/1.1 `Range` requests. |
| **[BEP 0029](https://www.bittorrent.org/beps/bep_0029.html)** | Micro Transport Protocol (uTP) | `hydroxy_utp` | UDP-based transport layer with LEDBAT congestion control. *(Crate stub created)*. |
| **[BEP 0052](https://www.bittorrent.org/beps/bep_0052.html)** | BitTorrent v2 | `hydroxy_bencode`<br>`hydroxy_engine` | Next-generation BitTorrent protocol using SHA-256 info-hashes, per-file Merkle hash trees, and hybrid v1/v2 torrents. |
| **[BEP 0055](https://www.bittorrent.org/beps/bep_0055.html)** | Holepunching Extension | `hydroxy_engine` | NAT traversal coordination via intermediate peers (`ut_holepunch`). |

---

## License

This project is released into the public domain under the **[Unlicense](https://unlicense.org)**. See the [UNLICENSE](./UNLICENSE) file for details.
