import 'package:flutter/material.dart';
import 'package:hydroxy_rc/configs/converter.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/ui/screens/main_content/models/torrent_structure_ext.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/torrent_piece_map.dart';

class TorrentGeneralTab extends StatelessWidget {
  final TorrentStructure torrent;

  const TorrentGeneralTab({
    super.key,
    required this.torrent,
  });

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;

    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Progress Section
          Row(
            children: [
              SizedBox(
                width: 80,
                child: Text(
                  'Progress:',
                  style: TextStyle(
                    color: colors.dataLabelTextColor,
                    fontSize: 12,
                  ),
                ),
              ),
              Expanded(
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(2.0),
                  child: LinearProgressIndicator(
                    value: torrent.progressValue,
                    backgroundColor: colors.tableProgressBarTrackColor,
                    color: colors.tableProgressBarProgressColor,
                    minHeight: 14,
                  ),
                ),
              ),
              const SizedBox(width: 12),
              Text(
                torrent.progressPercentage,
                style: TextStyle(
                  color: colors.tablePrimaryTextColor,
                  fontSize: 12,
                  fontWeight: FontWeight.bold,
                ),
              ),
            ],
          ),
          const SizedBox(height: 10),

          // Horizontal Piece Map Section
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              SizedBox(
                width: 80,
                child: Text(
                  'Pieces Map:',
                  style: TextStyle(
                    color: colors.dataLabelTextColor,
                    fontSize: 12,
                  ),
                ),
              ),
              Expanded(
                child: TorrentPieceMap(
                  infoHash: torrent.infoHash,
                  totalPieces: int.tryParse(torrent.totalPieces) ?? 0,
                  completedPieces: int.tryParse(torrent.completedPieces) ?? 0,
                  height: 16,
                  showStats: true,
                ),
              ),
            ],
          ),
          const SizedBox(height: 16),

          // Transfer Section
          Text(
            'Transfer',
            style: TextStyle(
              color: colors.tablePrimaryTextColor,
              fontWeight: FontWeight.bold,
              fontSize: 12,
            ),
          ),
          Divider(color: colors.dividerColor, height: 12),
          Row(
            children: [
              Expanded(
                child: _DetailRow(
                  label: 'Status:',
                  value: torrent.status,
                ),
              ),
              Expanded(
                child: _DetailRow(
                  label: 'ETA:',
                  value: torrent.etaString,
                ),
              ),
              Expanded(
                child: _DetailRow(
                  label: 'Ratio:',
                  value: torrent.ratioString,
                ),
              ),
            ],
          ),
          Row(
            children: [
              Expanded(
                child: _DetailRow(
                  label: 'Downloaded:',
                  value: formatBytesDynamic(torrent.downloaded),
                ),
              ),
              Expanded(
                child: _DetailRow(
                  label: 'Uploaded:',
                  value: formatBytesDynamic(torrent.uploaded),
                ),
              ),
              Expanded(
                child: _DetailRow(
                  label: 'Pieces:',
                  value: '${torrent.completedPieces} / ${torrent.totalPieces}',
                ),
              ),
            ],
          ),
          Row(
            children: [
              Expanded(
                child: _DetailRow(
                  label: 'Down Speed:',
                  value: formatBytesDynamic(torrent.downloadSpeed, isSpeed: true),
                ),
              ),
              Expanded(
                child: _DetailRow(
                  label: 'Up Speed:',
                  value: formatBytesDynamic(torrent.uploadSpeed, isSpeed: true),
                ),
              ),
              Expanded(
                child: _DetailRow(
                  label: 'Peers:',
                  value: '${torrent.peers} connected',
                ),
              ),
            ],
          ),
          const SizedBox(height: 14),

          // Information Section
          Text(
            'Information',
            style: TextStyle(
              color: colors.tablePrimaryTextColor,
              fontWeight: FontWeight.bold,
              fontSize: 12,
            ),
          ),
          Divider(color: colors.dividerColor, height: 12),
          Row(
            children: [
              Expanded(
                child: _DetailRow(
                  label: 'Total Size:',
                  value: formatBytesDynamic(torrent.size),
                ),
              ),
              Expanded(
                child: _DetailRow(
                  label: 'Completed:',
                  value: torrent.completedPieces,
                ),
              ),
              Expanded(
                child: _DetailRow(
                  label: 'Total Pieces:',
                  value: torrent.totalPieces,
                ),
              ),
            ],
          ),
          Row(
            children: [
              Expanded(
                child: _DetailRow(
                  label: 'Name:',
                  value: torrent.name,
                ),
              ),
            ],
          ),
          Row(
            children: [
              Expanded(
                child: _DetailRow(
                  label: 'Category:',
                  value: torrent.category.isNotEmpty ? torrent.category : 'None (Uncategorized)',
                ),
              ),
              Expanded(
                child: _DetailRow(
                  label: 'Sequential:',
                  value: torrent.sequential ? 'Enabled (Sequential Order)' : 'Disabled (Rarest-First)',
                ),
              ),
            ],
          ),
          Row(
            children: [
              Expanded(
                child: _DetailRow(
                  label: 'Info Hash:',
                  value: torrent.infoHash,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _DetailRow extends StatelessWidget {
  final String label;
  final String value;

  const _DetailRow({
    required this.label,
    required this.value,
  });

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2.0),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            '$label ',
            style: TextStyle(color: colors.dataLabelTextColor, fontSize: 11),
          ),
          Expanded(
            child: SelectableText(
              value,
              maxLines: 1,
              style: TextStyle(
                color: colors.tablePrimaryTextColor,
                fontSize: 11,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
