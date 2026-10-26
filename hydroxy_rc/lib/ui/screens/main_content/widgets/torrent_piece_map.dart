import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';

class TorrentPieceMap extends StatefulWidget {
  final String infoHash;
  final int totalPieces;
  final int completedPieces;
  final double height;
  final bool showStats;

  const TorrentPieceMap({
    super.key,
    required this.infoHash,
    required this.totalPieces,
    required this.completedPieces,
    this.height = 16.0,
    this.showStats = false,
  });

  @override
  State<TorrentPieceMap> createState() => _TorrentPieceMapState();
}

class _TorrentPieceMapState extends State<TorrentPieceMap> {
  Timer? _pollingTimer;
  Uint8List? _bitfieldBytes;
  int _latestCompleted = 0;
  int _latestTotal = 0;
  double? _hoverX;
  double _containerWidth = 0;

  @override
  void initState() {
    super.initState();
    _latestCompleted = widget.completedPieces;
    _latestTotal = widget.totalPieces;
    _fetchBitfield();
    _pollingTimer = Timer.periodic(const Duration(seconds: 2), (_) {
      if (mounted) {
        _fetchBitfield(isPolling: true);
      }
    });
  }

  @override
  void didUpdateWidget(covariant TorrentPieceMap oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.infoHash != widget.infoHash) {
      setState(() {
        _bitfieldBytes = null;
        _latestCompleted = widget.completedPieces;
        _latestTotal = widget.totalPieces;
        _hoverX = null;
      });
      _fetchBitfield();
    } else {
      _latestCompleted = widget.completedPieces;
      _latestTotal = widget.totalPieces;
    }
  }

  @override
  void dispose() {
    _pollingTimer?.cancel();
    super.dispose();
  }

  Future<void> _fetchBitfield({bool isPolling = false}) async {
    try {
      final detail = await getTorrentDetail(infoHash: widget.infoHash);
      if (mounted) {
        Uint8List? decoded;
        if (detail.pieces.isNotEmpty) {
          try {
            decoded = base64Decode(detail.pieces);
          } catch (_) {}
        }
        setState(() {
          _bitfieldBytes = decoded;
          _latestCompleted = detail.completedPieces;
          _latestTotal = detail.totalPieces;
        });
      }
    } catch (_) {}
  }

  String _getHoverText(int totalPieces) {
    if (_hoverX == null || _containerWidth <= 0 || totalPieces <= 0) return '';
    final pieceIdx = ((_hoverX! / _containerWidth) * totalPieces).floor().clamp(0, totalPieces - 1);
    final isDone = _isPieceComplete(pieceIdx, totalPieces);
    return 'Piece #$pieceIdx / $totalPieces — ${isDone ? "Downloaded" : "Missing"}';
  }

  bool _isPieceComplete(int pieceIdx, int totalPieces) {
    if (_latestTotal > 0 && _latestCompleted >= _latestTotal) return true;
    if (_bitfieldBytes == null) {
      if (totalPieces > 0 && _latestCompleted >= totalPieces) return true;
      return false;
    }
    final byteIdx = pieceIdx ~/ 8;
    final bitIdx = 7 - (pieceIdx % 8);
    if (byteIdx < _bitfieldBytes!.length) {
      return (_bitfieldBytes![byteIdx] & (1 << bitIdx)) != 0;
    }
    return false;
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    final total = _latestTotal > 0 ? _latestTotal : widget.totalPieces;
    final completed = _latestCompleted.clamp(0, total > 0 ? total : 0);
    final percent = total > 0 ? (completed / total * 100.0).clamp(0.0, 100.0) : 0.0;

    return LayoutBuilder(
      builder: (context, constraints) {
        _containerWidth = constraints.maxWidth;
        final hoverText = _getHoverText(total);

        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            MouseRegion(
              cursor: SystemMouseCursors.precise,
              onHover: (event) {
                setState(() {
                  _hoverX = event.localPosition.dx;
                });
              },
              onExit: (_) {
                setState(() {
                  _hoverX = null;
                });
              },
              child: Tooltip(
                message: hoverText.isNotEmpty
                    ? hoverText
                    : '$completed / $total pieces (${percent.toStringAsFixed(1)}%)',
                waitDuration: const Duration(milliseconds: 150),
                child: Container(
                  height: widget.height,
                  decoration: BoxDecoration(
                    color: colors.pieceMapBackgroundColor,
                    borderRadius: BorderRadius.circular(3),
                    border: Border.all(
                      color: colors.pieceMapBorderColor,
                      width: 1,
                    ),
                  ),
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(2),
                    child: CustomPaint(
                      size: Size(constraints.maxWidth, widget.height),
                      painter: _PieceMapPainter(
                        totalPieces: total,
                        completedPieces: completed,
                        bitfield: _bitfieldBytes,
                        completedColor: colors.pieceMapCompletedPieceColor,
                        backgroundColor: colors.pieceMapBackgroundColor,
                      ),
                    ),
                  ),
                ),
              ),
            ),
            if (widget.showStats) ...[
              const SizedBox(height: 4),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(
                    hoverText.isNotEmpty
                        ? hoverText
                        : 'Availability: $completed / $total pieces',
                    style: TextStyle(
                      color: colors.tableSecondaryTextColor,
                      fontSize: 10,
                      fontWeight: FontWeight.w400,
                    ),
                  ),
                  Text(
                    '${percent.toStringAsFixed(1)}%',
                    style: TextStyle(
                      color: percent >= 99.99
                          ? colors.accentGreenColor
                          : colors.accentBlueColor,
                      fontSize: 10,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                ],
              ),
            ],
          ],
        );
      },
    );
  }
}

class _PieceMapPainter extends CustomPainter {
  final int totalPieces;
  final int completedPieces;
  final Uint8List? bitfield;
  final Color completedColor;
  final Color backgroundColor;

  _PieceMapPainter({
    required this.totalPieces,
    required this.completedPieces,
    required this.bitfield,
    required this.completedColor,
    required this.backgroundColor,
  });

  @override
  void paint(Canvas canvas, Size size) {
    if (totalPieces <= 0) {
      canvas.drawRect(
        Rect.fromLTWH(0, 0, size.width, size.height),
        Paint()..color = backgroundColor,
      );
      return;
    }

    final isAllDone = completedPieces >= totalPieces;

    // 100% complete shortcut
    if (isAllDone) {
      canvas.drawRect(
        Rect.fromLTWH(0, 0, size.width, size.height),
        Paint()..color = completedColor,
      );
      return;
    }

    final bgPaint = Paint()..color = backgroundColor;
    canvas.drawRect(Rect.fromLTWH(0, 0, size.width, size.height), bgPaint);

    final greenColor = completedColor;
    final emptyColor = backgroundColor;

    if (totalPieces <= size.width) {
      // Piece by piece rendering
      final pieceWidth = size.width / totalPieces;
      final showGap = pieceWidth >= 4.0;
      final blockWidth = showGap ? (pieceWidth - 1.0).clamp(1.0, pieceWidth) : pieceWidth;

      final paint = Paint()..style = PaintingStyle.fill;

      for (int i = 0; i < totalPieces; i++) {
        final isDone = _checkPiece(i);
        if (isDone) {
          paint.color = greenColor;
          final x = i * pieceWidth;
          canvas.drawRect(
            Rect.fromLTWH(x, 0, blockWidth, size.height),
            paint,
          );
        }
      }
    } else {
      // Density-based sub-pixel rendering
      final pixels = size.width.ceil();
      final paint = Paint()..style = PaintingStyle.fill;

      for (int px = 0; px < pixels; px++) {
        final startPiece = (px * totalPieces) ~/ size.width;
        final endPiece = (((px + 1) * totalPieces) ~/ size.width).clamp(startPiece + 1, totalPieces);
        final pieceRangeCount = endPiece - startPiece;

        if (pieceRangeCount <= 0) continue;

        int doneInBucket = 0;
        for (int p = startPiece; p < endPiece; p++) {
          if (_checkPiece(p)) {
            doneInBucket++;
          }
        }

        if (doneInBucket > 0) {
          final density = doneInBucket / pieceRangeCount;
          paint.color = Color.lerp(emptyColor, greenColor, density)!;
          canvas.drawRect(
            Rect.fromLTWH(px.toDouble(), 0, 1.0, size.height),
            paint,
          );
        }
      }
    }
  }

  bool _checkPiece(int pieceIdx) {
    if (completedPieces >= totalPieces) return true;
    if (bitfield == null) return false;
    final byteIdx = pieceIdx ~/ 8;
    final bitIdx = 7 - (pieceIdx % 8);
    if (byteIdx < bitfield!.length) {
      return (bitfield![byteIdx] & (1 << bitIdx)) != 0;
    }
    return false;
  }

  @override
  bool shouldRepaint(covariant _PieceMapPainter oldDelegate) {
    return oldDelegate.totalPieces != totalPieces ||
        oldDelegate.completedPieces != completedPieces ||
        oldDelegate.bitfield != bitfield;
  }
}
