 import 'dart:math';

String formatBytesDynamic(String byteString, {bool isSpeed = false}) {
  final bytes = double.tryParse(byteString.trim()) ?? 0.0;
  if (bytes <= 0) return isSpeed ? '0 B/s' : '0 B';

  const suffixes = ['B', 'KB', 'MB', 'GB', 'TB'];
  var i = (log(bytes) / log(1024)).floor();
  if (i >= suffixes.length) i = suffixes.length - 1;

  final num = bytes / pow(1024, i);
  final unit = isSpeed ? '${suffixes[i]}/s' : suffixes[i];

  return '${num.toStringAsFixed(2)} $unit';
}