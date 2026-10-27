import 'package:hydroxy_rc/network/models.dart';

extension TorrentStructureX on TorrentStructure {
  /// Progress as a double between 0.0 and 1.0
  double get progressValue {
    final value = double.tryParse(progress.trim()) ?? 0.0;
    return value.clamp(0.0, 1.0);
  }

  /// Formatted progress percentage e.g. "75.4%"
  String get progressPercentage {
    return '${(progressValue * 100).toStringAsFixed(1)}%';
  }

  /// Size in integer bytes
  int get sizeBytes => (double.tryParse(size.trim()) ?? 0.0).round();

  /// Downloaded amount in integer bytes
  int get downloadedBytes {
    final d = (double.tryParse(downloaded.trim()) ?? 0.0).round();
    final s = sizeBytes;
    if (s > 0 && (d > s || progressValue >= 1.0)) {
      return s;
    }
    return d;
  }

  /// Uploaded amount in integer bytes
  int get uploadedBytes => (double.tryParse(uploaded.trim()) ?? 0.0).round();

  /// Download speed in double bytes per second (supports decimal speeds from backend)
  double get downloadSpeedValue => double.tryParse(downloadSpeed.trim()) ?? 0.0;

  /// Download speed in integer bytes per second
  int get downloadSpeedBytes => downloadSpeedValue.round();

  /// Upload speed in double bytes per second (supports decimal speeds from backend)
  double get uploadSpeedValue => double.tryParse(uploadSpeed.trim()) ?? 0.0;

  /// Upload speed in integer bytes per second
  int get uploadSpeedBytes => uploadSpeedValue.round();

  /// Total pieces count
  int get totalPiecesCount => (double.tryParse(totalPieces.trim()) ?? 0.0).round();

  /// Completed pieces count
  int get completedPiecesCount => (double.tryParse(completedPieces.trim()) ?? 0.0).round();

  /// Active connected peers count
  int get peersCount => (double.tryParse(peers.trim()) ?? 0.0).round();

  /// Ratio: uploaded / downloaded
  double get ratio {
    final down = downloadedBytes;
    final up = uploadedBytes;
    if (down > 0) {
      return up / down;
    }
    return up > 0 ? 999.0 : 0.0;
  }

  /// Formatted ratio string e.g. "1.25"
  String get ratioString {
    final r = ratio;
    if (r >= 999.0) return '∞';
    return r.toStringAsFixed(2);
  }

  /// Estimated time remaining in seconds
  int get etaSeconds {
    final speed = downloadSpeedBytes;
    if (speed <= 0) return 999999999;
    final remaining = sizeBytes - downloadedBytes;
    if (remaining <= 0) return 0;
    return (remaining / speed).ceil();
  }

  /// Formatted ETA string e.g. "5m 20s", "Done", "∞"
  String get etaString {
    if (progressValue >= 1.0 || (sizeBytes > 0 && downloadedBytes >= sizeBytes)) {
      return 'Done';
    }
    final speed = downloadSpeedBytes;
    if (speed <= 0) {
      return '∞';
    }
    final secs = etaSeconds;
    if (secs > 86400) {
      final days = secs ~/ 86400;
      final hours = (secs % 86400) ~/ 3600;
      return '${days}d ${hours}h';
    }
    if (secs > 3600) {
      final hours = secs ~/ 3600;
      final mins = (secs % 3600) ~/ 60;
      return '${hours}h ${mins}m';
    }
    if (secs > 60) {
      final mins = secs ~/ 60;
      final remainingSecs = secs % 60;
      return '${mins}m ${remainingSecs}s';
    }
    return '${secs}s';
  }
}
