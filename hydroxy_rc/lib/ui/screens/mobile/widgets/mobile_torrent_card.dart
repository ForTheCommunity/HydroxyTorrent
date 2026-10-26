import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hydroxy_rc/configs/converter.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/state/torrents_bloc/torrents_bloc.dart';
import 'package:hydroxy_rc/ui/screens/main_content/models/torrent_structure_ext.dart';

class MobileTorrentCard extends StatelessWidget {
  final TorrentStructure torrent;
  final VoidCallback onTap;
  final VoidCallback? onRecheck;
  final VoidCallback? onRemove;

  const MobileTorrentCard({
    super.key,
    required this.torrent,
    required this.onTap,
    this.onRecheck,
    this.onRemove,
  });

  bool get _isPaused {
    final s = torrent.status.toLowerCase();
    return s == 'paused' || s == 'stopped';
  }

  Color _getStatusColor(BuildContext context) {
    final colors = context.appColors;
    final s = torrent.status.toLowerCase();
    switch (s) {
      case 'downloading':
        return colors.accentGreenColor;
      case 'seeding':
      case 'completed':
        return colors.accentPurpleColor;
      case 'stalled':
        return colors.accentAmberColor;
      case 'queued':
        return colors.tableAccentTextColor;
      case 'checking':
        return colors.accentBlueColor;
      case 'paused':
      case 'stopped':
        return colors.tableSecondaryTextColor;
      case 'error':
        return colors.accentRedColor;
      default:
        return colors.tableSecondaryTextColor;
    }
  }

  IconData _getStatusIcon() {
    final s = torrent.status.toLowerCase();
    switch (s) {
      case 'downloading':
        return Icons.arrow_downward_rounded;
      case 'seeding':
      case 'completed':
        return Icons.arrow_upward_rounded;
      case 'stalled':
        return Icons.hourglass_empty_rounded;
      case 'queued':
        return Icons.schedule_rounded;
      case 'checking':
        return Icons.sync_rounded;
      case 'paused':
      case 'stopped':
        return Icons.pause_rounded;
      case 'error':
        return Icons.error_outline_rounded;
      default:
        return Icons.help_outline_rounded;
    }
  }

  Future<void> _togglePlayPause(BuildContext context) async {
    final bloc = context.read<TorrentsBloc>();
    if (_isPaused) {
      await resumeTorrent(infoHash: torrent.infoHash);
    } else {
      await pauseTorrent(infoHash: torrent.infoHash);
    }
    bloc.add(const TorrentsEvent.fetch());
  }

  Future<void> _toggleSequential(BuildContext context) async {
    final colors = context.appColors;
    final enable = !torrent.sequential;
    await setSequentialDownload(infoHash: torrent.infoHash, sequential: enable);
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Sequential download for "${torrent.name}" ${enable ? "Enabled" : "Disabled"}',
            style: TextStyle(color: colors.snackBarTextColor, fontSize: 13, fontWeight: FontWeight.w500),
          ),
          duration: const Duration(seconds: 2),
          backgroundColor: colors.snackBarBackgroundColor,
          behavior: SnackBarBehavior.floating,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(8),
            side: BorderSide(color: colors.popupMenuBorderColor),
          ),
        ),
      );
      context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
    }
  }

  Future<void> _reannounce(BuildContext context) async {
    final colors = context.appColors;
    await reannounceTorrent(infoHash: torrent.infoHash);
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Reannounced "${torrent.name}"',
            style: TextStyle(color: colors.snackBarTextColor, fontSize: 13, fontWeight: FontWeight.w500),
          ),
          duration: const Duration(seconds: 2),
          backgroundColor: colors.snackBarBackgroundColor,
          behavior: SnackBarBehavior.floating,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(8),
            side: BorderSide(color: colors.popupMenuBorderColor),
          ),
        ),
      );
      context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
    }
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    final statusColor = _getStatusColor(context);

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
      elevation: 0,
      color: colors.dataCardBackgroundColor,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(10),
        side: BorderSide(
          color: Color.fromRGBO(colors.dividerColor.red, colors.dividerColor.green, colors.dividerColor.blue, 0.6),
          width: 0.8,
        ),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Top Row: Title + Status Badge + Quick Play/Pause
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(
                    child: Text(
                      torrent.name,
                      style: TextStyle(
                        color: colors.tablePrimaryTextColor,
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                        height: 1.25,
                      ),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  const SizedBox(width: 8),

                  // Status Badge
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 3),
                    decoration: BoxDecoration(
                      color: Color.fromRGBO(statusColor.red, statusColor.green, statusColor.blue, 0.15),
                      borderRadius: BorderRadius.circular(6),
                      border: Border.all(
                        color: Color.fromRGBO(statusColor.red, statusColor.green, statusColor.blue, 0.4),
                        width: 0.8,
                      ),
                    ),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(_getStatusIcon(), size: 12, color: statusColor),
                        const SizedBox(width: 4),
                        Text(
                          torrent.status,
                          style: TextStyle(
                            color: statusColor,
                            fontSize: 11,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),

              const SizedBox(height: 10),

              // Progress Bar
              ClipRRect(
                borderRadius: BorderRadius.circular(3),
                child: LinearProgressIndicator(
                  value: torrent.progressValue,
                  minHeight: 5,
                  backgroundColor: colors.tableProgressBarTrackColor,
                  color: statusColor,
                ),
              ),

              const SizedBox(height: 8),

              // Progress and Size Info Row
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(
                    '${torrent.progressPercentage}  •  ${formatBytesDynamic(torrent.downloaded)} / ${formatBytesDynamic(torrent.size)}',
                    style: TextStyle(
                      color: colors.tablePrimaryTextColor,
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  Text(
                    'ETA: ${torrent.etaString}',
                    style: TextStyle(
                      color: colors.tableSecondaryTextColor,
                      fontSize: 11,
                    ),
                  ),
                ],
              ),

              const SizedBox(height: 6),

              // Speeds & Peers Row + Quick Actions
              Row(
                children: [
                  // Down Speed
                  Icon(
                    Icons.arrow_downward_rounded,
                    size: 13,
                    color: colors.downloadSpeedColor,
                  ),
                  const SizedBox(width: 2),
                  Text(
                    formatBytesDynamic(torrent.downloadSpeed, isSpeed: true),
                    style: TextStyle(
                      color: colors.downloadSpeedColor,
                      fontSize: 11,
                      fontWeight: FontWeight.bold,
                      fontFamily: 'monospace',
                    ),
                  ),

                  const SizedBox(width: 10),

                  // Up Speed
                  Icon(
                    Icons.arrow_upward_rounded,
                    size: 13,
                    color: colors.uploadSpeedColor,
                  ),
                  const SizedBox(width: 2),
                  Text(
                    formatBytesDynamic(torrent.uploadSpeed, isSpeed: true),
                    style: TextStyle(
                      color: colors.uploadSpeedColor,
                      fontSize: 11,
                      fontWeight: FontWeight.bold,
                      fontFamily: 'monospace',
                    ),
                  ),

                  const SizedBox(width: 10),

                  // Peers
                  Icon(
                    Icons.people_alt_outlined,
                    size: 13,
                    color: colors.tableSecondaryTextColor,
                  ),
                  const SizedBox(width: 3),
                  Text(
                    torrent.peers,
                    style: TextStyle(
                      color: colors.tableSecondaryTextColor,
                      fontSize: 11,
                    ),
                  ),

                  if (torrent.sequential) ...[
                    const SizedBox(width: 8),
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
                      decoration: BoxDecoration(
                        color: Color.fromRGBO(colors.accentBlueColor.red, colors.accentBlueColor.green, colors.accentBlueColor.blue, 0.15),
                        borderRadius: BorderRadius.circular(3),
                        border: Border.all(
                          color: Color.fromRGBO(colors.accentBlueColor.red, colors.accentBlueColor.green, colors.accentBlueColor.blue, 0.4),
                          width: 0.6,
                        ),
                      ),
                      child: Text(
                        'SEQ',
                        style: TextStyle(
                          color: colors.accentBlueColor,
                          fontSize: 9,
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                    ),
                  ],

                  const Spacer(),

                  // Quick Action: Play / Pause
                  InkWell(
                    borderRadius: BorderRadius.circular(16),
                    onTap: () => _togglePlayPause(context),
                    child: Padding(
                      padding: const EdgeInsets.all(4),
                      child: Icon(
                        _isPaused
                            ? Icons.play_arrow_rounded
                            : Icons.pause_rounded,
                        size: 20,
                        color: _isPaused
                            ? colors.accentGreenColor
                            : colors.accentAmberColor,
                      ),
                    ),
                  ),

                  const SizedBox(width: 2),

                  // Quick Action: More Options Popup
                  PopupMenuButton<String>(
                    tooltip: 'Options',
                    icon: Icon(
                      Icons.more_vert_rounded,
                      size: 18,
                      color: colors.tableSecondaryTextColor,
                    ),
                    padding: EdgeInsets.zero,
                    color: colors.popupMenuBackgroundColor,
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(8),
                      side: BorderSide(color: colors.popupMenuBorderColor),
                    ),
                    onSelected: (value) {
                      switch (value) {
                        case 'details':
                          onTap();
                          break;
                        case 'sequential':
                          _toggleSequential(context);
                          break;
                        case 'reannounce':
                          _reannounce(context);
                          break;
                        case 'recheck':
                          onRecheck?.call();
                          break;
                        case 'remove':
                          onRemove?.call();
                          break;
                      }
                    },
                    itemBuilder: (context) => [
                      PopupMenuItem(
                        value: 'details',
                        child: Row(
                          children: [
                            Icon(Icons.info_outline_rounded, size: 16, color: colors.tablePrimaryTextColor),
                            const SizedBox(width: 10),
                            Text('View Details', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
                          ],
                        ),
                      ),
                      PopupMenuItem(
                        value: 'sequential',
                        child: Row(
                          children: [
                            Icon(
                              torrent.sequential ? Icons.check_box_outlined : Icons.check_box_outline_blank,
                              size: 16,
                              color: colors.accentBlueColor,
                            ),
                            const SizedBox(width: 10),
                            Text(
                              torrent.sequential ? 'Disable Sequential' : 'Enable Sequential',
                              style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13),
                            ),
                          ],
                        ),
                      ),
                      PopupMenuItem(
                        value: 'reannounce',
                        child: Row(
                          children: [
                            Icon(Icons.campaign_outlined, size: 16, color: colors.accentAmberColor),
                            const SizedBox(width: 10),
                            Text('Force Reannounce', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
                          ],
                        ),
                      ),
                      PopupMenuItem(
                        value: 'recheck',
                        child: Row(
                          children: [
                            Icon(Icons.sync_rounded, size: 16, color: colors.accentBlueColor),
                            const SizedBox(width: 10),
                            Text('Force Recheck', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
                          ],
                        ),
                      ),
                      const PopupMenuDivider(),
                      PopupMenuItem(
                        value: 'remove',
                        child: Row(
                          children: [
                            Icon(Icons.delete_outline_rounded, size: 16, color: colors.accentRedColor),
                            const SizedBox(width: 10),
                            Text('Remove Torrent', style: TextStyle(color: colors.accentRedColor, fontSize: 13)),
                          ],
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
