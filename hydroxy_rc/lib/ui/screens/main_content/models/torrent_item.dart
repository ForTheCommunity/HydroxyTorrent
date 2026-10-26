class TorrentItem {
  final String id;
  final String name;
  final int sizeInBytes;
  final double progress;
  final String status;
  final int seeds;
  final int peers;
  final int downSpeedKbps;
  final int upSpeedKbps;
  final String eta;
  final double ratio;
  final int uploadedBytes;
  final int downloadedBytes;
  final DateTime addedDate;

  TorrentItem({
    required this.id,
    required this.name,
    required this.sizeInBytes,
    required this.progress,
    required this.status,
    required this.seeds,
    required this.peers,
    required this.downSpeedKbps,
    required this.upSpeedKbps,
    required this.eta,
    required this.ratio,
    required this.uploadedBytes,
    required this.downloadedBytes,
    required this.addedDate,
  });
}