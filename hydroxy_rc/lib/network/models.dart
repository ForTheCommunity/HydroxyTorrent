class TorrentStructure {
  final String infoHash;
  final String name;
  final String category;
  final String size;
  final String downloaded;
  final String uploaded;
  final String progress;
  final String status;
  final String downloadSpeed;
  final String uploadSpeed;
  final String peers;
  final String totalPieces;
  final String completedPieces;
  final bool sequential;

  TorrentStructure({
    required this.infoHash,
    required this.name,
    this.category = '',
    required this.size,
    required this.downloaded,
    required this.uploaded,
    required this.progress,
    required this.status,
    required this.downloadSpeed,
    required this.uploadSpeed,
    required this.peers,
    required this.totalPieces,
    required this.completedPieces,
    this.sequential = false,
  });

  factory TorrentStructure.fromJson(Map<String, dynamic> json) {
    return TorrentStructure(
      infoHash: json['info_hash']?.toString() ?? '',
      name: json['name']?.toString() ?? '',
      category: json['category']?.toString() ?? '',
      size: json['size']?.toString() ?? '0',
      downloaded: json['downloaded']?.toString() ?? '0',
      uploaded: json['uploaded']?.toString() ?? '0',
      progress: json['progress']?.toString() ?? '0',
      status: json['status']?.toString() ?? 'Unknown',
      downloadSpeed: json['download_speed']?.toString() ?? '0',
      uploadSpeed: json['upload_speed']?.toString() ?? '0',
      peers: json['peers']?.toString() ?? '0',
      totalPieces: json['total_pieces']?.toString() ?? '0',
      completedPieces: json['completed_pieces']?.toString() ?? '0',
      sequential: json['sequential'] == true,
    );
  }
}

class PeerStatsSnapshot {
  final String ipAddr;
  final int port;
  final String clientId;
  final String state;
  final int downloadedBytes;
  final int uploadedBytes;
  final double downloadSpeed;
  final double uploadSpeed;
  final int piecesReceived;
  final int connectedDurationSecs;

  PeerStatsSnapshot({
    required this.ipAddr,
    required this.port,
    required this.clientId,
    required this.state,
    required this.downloadedBytes,
    required this.uploadedBytes,
    required this.downloadSpeed,
    required this.uploadSpeed,
    required this.piecesReceived,
    required this.connectedDurationSecs,
  });

  factory PeerStatsSnapshot.fromJson(Map<String, dynamic> json) {
    return PeerStatsSnapshot(
      ipAddr: json['ip_addr']?.toString() ?? '',
      port: (json['port'] as num?)?.toInt() ?? 0,
      clientId: json['client_id']?.toString() ?? '',
      state: json['state']?.toString() ?? 'Connected',
      downloadedBytes: (json['downloaded_bytes'] as num?)?.toInt() ?? 0,
      uploadedBytes: (json['uploaded_bytes'] as num?)?.toInt() ?? 0,
      downloadSpeed: (json['download_speed'] as num?)?.toDouble() ?? 0.0,
      uploadSpeed: (json['upload_speed'] as num?)?.toDouble() ?? 0.0,
      piecesReceived: (json['pieces_received'] as num?)?.toInt() ?? 0,
      connectedDurationSecs:
          (json['connected_duration_secs'] as num?)?.toInt() ?? 0,
    );
  }

  String get endpoint => '$ipAddr:$port';
}

class TrackerStatsSnapshot {
  final int tier;
  final String url;
  final String status;
  final int peers;
  final int interval;
  final int nextAnnounce;

  TrackerStatsSnapshot({
    required this.tier,
    required this.url,
    required this.status,
    required this.peers,
    this.interval = 0,
    this.nextAnnounce = 0,
  });

  factory TrackerStatsSnapshot.fromJson(Map<String, dynamic> json) {
    return TrackerStatsSnapshot(
      tier: (json['tier'] as num?)?.toInt() ?? 1,
      url: json['url']?.toString() ?? '',
      status: json['status']?.toString() ?? 'Unknown',
      peers: (json['peers'] as num?)?.toInt() ?? 0,
      interval: (json['interval'] as num?)?.toInt() ?? 0,
      nextAnnounce: (json['next_announce'] as num?)?.toInt() ?? 0,
    );
  }

  String get reannounceInString {
    if (nextAnnounce <= 0) return '—';
    final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    final diff = nextAnnounce - now;
    if (diff <= 0) return 'Now';
    if (diff < 60) return '${diff}s';
    final mins = diff ~/ 60;
    final secs = diff % 60;
    if (mins < 60) return '${mins}m ${secs}s';
    final hours = mins ~/ 60;
    final remMins = mins % 60;
    return '${hours}h ${remMins}m';
  }
}

class TorrentFileItem {
  final int id;
  final String name;
  final String path;
  final int size;
  final double progress;
  final String priority;

  TorrentFileItem({
    required this.id,
    required this.name,
    required this.path,
    required this.size,
    required this.progress,
    required this.priority,
  });

  factory TorrentFileItem.fromJson(Map<String, dynamic> json) {
    return TorrentFileItem(
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: json['name']?.toString() ?? '',
      path: json['path']?.toString() ?? '',
      size: (json['size'] as num?)?.toInt() ?? 0,
      progress: (json['progress'] as num?)?.toDouble() ?? 0.0,
      priority: json['priority']?.toString() ?? 'Normal',
    );
  }
}

class TorrentDetailApiItem {
  final String infoHash;
  final String name;
  final String category;
  final int size;
  final int downloaded;
  final int uploaded;
  final double progress;
  final String status;
  final double downloadSpeed;
  final double uploadSpeed;
  final int peers;
  final int totalPieces;
  final int completedPieces;
  final bool sequential;
  final String pieces;
  final List<PeerStatsSnapshot> peerList;
  final List<TrackerStatsSnapshot> trackers;
  final List<TorrentFileItem> files;

  TorrentDetailApiItem({
    required this.infoHash,
    required this.name,
    this.category = '',
    required this.size,
    required this.downloaded,
    required this.uploaded,
    required this.progress,
    required this.status,
    required this.downloadSpeed,
    required this.uploadSpeed,
    required this.peers,
    required this.totalPieces,
    required this.completedPieces,
    this.sequential = false,
    this.pieces = '',
    required this.peerList,
    this.trackers = const [],
    this.files = const [],
  });

  factory TorrentDetailApiItem.fromJson(Map<String, dynamic> json) {
    final rawPeers = json['peer_list'] as List<dynamic>? ?? [];
    final rawTrackers = json['trackers'] as List<dynamic>? ?? [];
    final rawFiles = json['files'] as List<dynamic>? ?? [];
    return TorrentDetailApiItem(
      infoHash: json['info_hash']?.toString() ?? '',
      name: json['name']?.toString() ?? '',
      category: json['category']?.toString() ?? '',
      size: (json['size'] as num?)?.toInt() ?? 0,
      downloaded: (json['downloaded'] as num?)?.toInt() ?? 0,
      uploaded: (json['uploaded'] as num?)?.toInt() ?? 0,
      progress: (json['progress'] as num?)?.toDouble() ?? 0.0,
      status: json['status']?.toString() ?? 'Unknown',
      downloadSpeed: (json['download_speed'] as num?)?.toDouble() ?? 0.0,
      uploadSpeed: (json['upload_speed'] as num?)?.toDouble() ?? 0.0,
      peers: (json['peers'] as num?)?.toInt() ?? 0,
      totalPieces: (json['total_pieces'] as num?)?.toInt() ?? 0,
      completedPieces: (json['completed_pieces'] as num?)?.toInt() ?? 0,
      sequential: json['sequential'] == true,
      pieces: json['pieces']?.toString() ?? '',
      peerList: rawPeers
          .map((item) =>
              PeerStatsSnapshot.fromJson(item as Map<String, dynamic>))
          .toList(),
      trackers: rawTrackers
          .map((item) =>
              TrackerStatsSnapshot.fromJson(item as Map<String, dynamic>))
          .toList(),
      files: rawFiles
          .map((item) =>
              TorrentFileItem.fromJson(item as Map<String, dynamic>))
          .toList(),
    );
  }
}

class SystemStatsModel {
  final int systemTotalBytes;
  final int systemUsedBytes;
  final int processRssBytes;

  SystemStatsModel({
    required this.systemTotalBytes,
    required this.systemUsedBytes,
    required this.processRssBytes,
  });

  factory SystemStatsModel.fromJson(Map<String, dynamic> json) {
    return SystemStatsModel(
      systemTotalBytes: (json['system_total_bytes'] as num?)?.toInt() ?? 0,
      systemUsedBytes: (json['system_used_bytes'] as num?)?.toInt() ?? 0,
      processRssBytes: (json['process_rss_bytes'] as num?)?.toInt() ?? 0,
    );
  }

  double get systemMemoryUsagePercent {
    if (systemTotalBytes <= 0) return 0.0;
    return (systemUsedBytes / systemTotalBytes).clamp(0.0, 1.0);
  }
}
