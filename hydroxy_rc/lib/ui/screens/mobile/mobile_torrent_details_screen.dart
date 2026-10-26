import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hydroxy_rc/configs/converter.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/state/torrents_bloc/torrents_bloc.dart';
import 'package:hydroxy_rc/themes/app_theme.dart';
import 'package:hydroxy_rc/ui/screens/main_content/models/torrent_structure_ext.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/tabs/torrent_files_tab.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/tabs/torrent_peers_tab.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/tabs/torrent_placeholder_tab.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/tabs/torrent_trackers_tab.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/torrent_piece_map.dart';

class MobileTorrentDetailsScreen extends StatefulWidget {
  final TorrentStructure initialTorrent;

  const MobileTorrentDetailsScreen({
    super.key,
    required this.initialTorrent,
  });

  @override
  State<MobileTorrentDetailsScreen> createState() =>
      _MobileTorrentDetailsScreenState();
}

class _MobileTorrentDetailsScreenState extends State<MobileTorrentDetailsScreen>
    with SingleTickerProviderStateMixin {
  late TabController _tabController;
  late TorrentStructure _torrent;

  @override
  void initState() {
    super.initState();
    _torrent = widget.initialTorrent;
    _tabController = TabController(length: 5, vsync: this);
  }

  @override
  void dispose() {
    _tabController.dispose();
    super.dispose();
  }

  bool get _isPaused {
    final s = _torrent.status.toLowerCase();
    return s == 'paused' || s == 'stopped';
  }

  Color _getStatusColor(BuildContext context, String status) {
    final colors = context.appColors;
    switch (status.toLowerCase()) {
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

  Future<void> _togglePlayPause() async {
    final bloc = context.read<TorrentsBloc>();
    if (_isPaused) {
      await resumeTorrent(infoHash: _torrent.infoHash);
    } else {
      await pauseTorrent(infoHash: _torrent.infoHash);
    }
    bloc.add(const TorrentsEvent.fetch());
  }

  Future<void> _toggleSequential(bool enable) async {
    final colors = context.appColors;
    await setSequentialDownload(infoHash: _torrent.infoHash, sequential: enable);
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Sequential download ${enable ? "Enabled" : "Disabled"}',
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

  Future<void> _recheckTorrentDialog() async {
    final colors = context.appColors;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: colors.popupMenuBackgroundColor,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(10),
          side: BorderSide(color: colors.popupMenuBorderColor),
        ),
        title: Row(
          children: [
            Icon(Icons.sync_rounded, color: colors.accentBlueColor, size: 22),
            const SizedBox(width: 8),
            const Text('Force Recheck', style: TextStyle(fontSize: 16)),
          ],
        ),
        content: Text(
          'Re-verify downloaded data on disk for "${_torrent.name}"? Active transfer will be paused during verification.',
          style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: Text('Cancel', style: TextStyle(color: colors.tableSecondaryTextColor)),
          ),
          ElevatedButton(
            style: ElevatedButton.styleFrom(
              backgroundColor: colors.accentBlueColor,
              foregroundColor: Colors.white,
            ),
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Recheck'),
          ),
        ],
      ),
    );

    if (confirmed == true && mounted) {
      await recheckTorrent(infoHash: _torrent.infoHash);
      if (mounted) {
        context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
      }
    }
  }

  Future<void> _removeTorrentDialog() async {
    final colors = context.appColors;
    bool deleteFiles = false;

    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => StatefulBuilder(
        builder: (ctx, setDialogState) => AlertDialog(
          backgroundColor: colors.popupMenuBackgroundColor,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(10),
            side: BorderSide(color: colors.popupMenuBorderColor),
          ),
          title: Row(
            children: [
              Icon(Icons.delete_outline_rounded, color: colors.accentRedColor, size: 22),
              const SizedBox(width: 8),
              const Text('Remove Torrent', style: TextStyle(fontSize: 16)),
            ],
          ),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Remove "${_torrent.name}" from transfer list?',
                style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 13),
              ),
              const SizedBox(height: 12),
              InkWell(
                onTap: () {
                  setDialogState(() {
                    deleteFiles = !deleteFiles;
                  });
                },
                child: Row(
                  children: [
                    Checkbox(
                      value: deleteFiles,
                      activeColor: colors.accentRedColor,
                      onChanged: (val) {
                        setDialogState(() {
                          deleteFiles = val ?? false;
                        });
                      },
                    ),
                    Expanded(
                      child: Text(
                        'Also delete files from disk',
                        style: TextStyle(
                          color: deleteFiles ? colors.accentRedColor : colors.tablePrimaryTextColor,
                          fontSize: 13,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: Text('Cancel', style: TextStyle(color: colors.tableSecondaryTextColor)),
            ),
            ElevatedButton(
              style: ElevatedButton.styleFrom(
                backgroundColor: colors.accentRedColor,
                foregroundColor: Colors.white,
              ),
              onPressed: () => Navigator.of(ctx).pop(true),
              child: const Text('Remove'),
            ),
          ],
        ),
      ),
    );

    if (confirmed == true && mounted) {
      await removeTorrent(infoHash: _torrent.infoHash, deleteFiles: deleteFiles);
      if (mounted) {
        context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
        Navigator.of(context).pop();
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;

    return BlocListener<TorrentsBloc, TorrentsState>(
      listener: (context, state) {
        state.maybeWhen(
          loaded: (torrents) {
            final updated = torrents.firstWhere(
              (t) => t.infoHash == _torrent.infoHash,
              orElse: () => _torrent,
            );
            if (mounted) {
              setState(() {
                _torrent = updated;
              });
            }
          },
          orElse: () {},
        );
      },
      child: Scaffold(
        backgroundColor: colors.scaffoldBackgroundColor,
        appBar: AppBar(
          backgroundColor: colors.titleBarBackgroundColor,
          elevation: 0,
          leading: IconButton(
            icon: Icon(Icons.arrow_back_rounded, color: colors.titleBarTextColor),
            onPressed: () => Navigator.of(context).pop(),
          ),
          title: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                _torrent.name,
                style: TextStyle(
                  color: colors.titleBarTextColor,
                  fontSize: 14,
                  fontWeight: FontWeight.bold,
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              Text(
                'Status: ${_torrent.status}',
                style: TextStyle(
                  color: _getStatusColor(context, _torrent.status),
                  fontSize: 11,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ),
          actions: [
            // Play / Pause Action
            IconButton(
              icon: Icon(
                _isPaused ? Icons.play_arrow_rounded : Icons.pause_rounded,
                color: _isPaused ? colors.accentGreenColor : colors.accentAmberColor,
              ),
              tooltip: _isPaused ? 'Resume Torrent' : 'Pause Torrent',
              onPressed: _togglePlayPause,
            ),

            // More Options Menu
            PopupMenuButton<String>(
              icon: Icon(Icons.more_vert_rounded, color: colors.titleBarIconColor),
              color: colors.popupMenuBackgroundColor,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(8),
                side: BorderSide(color: colors.popupMenuBorderColor),
              ),
              onSelected: (val) {
                switch (val) {
                  case 'seq':
                    _toggleSequential(!_torrent.sequential);
                    break;
                  case 'recheck':
                    _recheckTorrentDialog();
                    break;
                  case 'delete':
                    _removeTorrentDialog();
                    break;
                }
              },
              itemBuilder: (context) => [
                PopupMenuItem(
                  value: 'seq',
                  child: Row(
                    children: [
                      Icon(
                        _torrent.sequential ? Icons.check_box_outlined : Icons.check_box_outline_blank,
                        size: 16,
                        color: colors.accentBlueColor,
                      ),
                      const SizedBox(width: 8),
                      Text(
                        _torrent.sequential ? 'Disable Sequential' : 'Enable Sequential',
                        style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13),
                      ),
                    ],
                  ),
                ),
                PopupMenuItem(
                  value: 'recheck',
                  child: Row(
                    children: [
                      Icon(Icons.sync_rounded, size: 16, color: colors.accentBlueColor),
                      const SizedBox(width: 8),
                      Text('Force Recheck', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
                    ],
                  ),
                ),
                const PopupMenuDivider(),
                PopupMenuItem(
                  value: 'delete',
                  child: Row(
                    children: [
                      Icon(Icons.delete_outline_rounded, size: 16, color: colors.accentRedColor),
                      const SizedBox(width: 8),
                      Text('Remove Torrent', style: TextStyle(color: colors.accentRedColor, fontSize: 13)),
                    ],
                  ),
                ),
              ],
            ),
          ],
          bottom: PreferredSize(
            preferredSize: const Size.fromHeight(40),
            child: Container(
              color: colors.detailsPanelBackgroundColor,
              child: TabBar(
                controller: _tabController,
                isScrollable: true,
                indicatorColor: colors.tabBarActiveIndicatorColor,
                indicatorWeight: 2.5,
                labelColor: colors.tabBarActiveTextColor,
                unselectedLabelColor: colors.tabBarInactiveTextColor,
                labelStyle: const TextStyle(fontSize: 12, fontWeight: FontWeight.bold),
                unselectedLabelStyle: const TextStyle(fontSize: 12),
                tabs: const [
                  Tab(icon: Icon(Icons.info_outline, size: 16), text: 'General'),
                  Tab(icon: Icon(Icons.location_on_outlined, size: 16), text: 'Trackers'),
                  Tab(icon: Icon(Icons.people_outline, size: 16), text: 'Peers'),
                  Tab(icon: Icon(Icons.storage_outlined, size: 16), text: 'HTTP Sources'),
                  Tab(icon: Icon(Icons.folder_open_rounded, size: 16), text: 'Content'),
                ],
              ),
            ),
          ),
        ),
        body: TabBarView(
          controller: _tabController,
          children: [
            // Tab 0: General Tab
            _buildMobileGeneralTab(context),

            // Tab 1: Trackers
            TorrentTrackersTab(
              infoHash: _torrent.infoHash,
              torrentName: _torrent.name,
            ),

            // Tab 2: Peers
            TorrentPeersTab(
              infoHash: _torrent.infoHash,
              torrentName: _torrent.name,
            ),

            // Tab 3: HTTP Sources
            TorrentPlaceholderTab(
              title: 'HTTP Sources',
              icon: Icons.storage_outlined,
              torrent: _torrent,
              mockHeaders: const ['URL', 'Status', 'Speed'],
            ),

            // Tab 4: Content / Files
            TorrentFilesTab(
              infoHash: _torrent.infoHash,
              torrentName: _torrent.name,
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildMobileGeneralTab(BuildContext context) {
    final colors = context.appColors;
    final statusColor = _getStatusColor(context, _torrent.status);

    return SingleChildScrollView(
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Card 1: Progress & Piece Map
          Card(
            color: colors.dataCardBackgroundColor,
            elevation: 0,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(10),
              side: BorderSide(color: Color.fromRGBO(colors.dividerColor.red, colors.dividerColor.green, colors.dividerColor.blue, 0.6), width: 0.8),
            ),
            child: Padding(
              padding: const EdgeInsets.all(14),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text(
                        'Progress',
                        style: TextStyle(
                          color: colors.tableSecondaryTextColor,
                          fontSize: 12,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      Text(
                        _torrent.progressPercentage,
                        style: TextStyle(
                          color: colors.tablePrimaryTextColor,
                          fontSize: 18,
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  ClipRRect(
                    borderRadius: BorderRadius.circular(4),
                    child: LinearProgressIndicator(
                      value: _torrent.progressValue,
                      minHeight: 8,
                      backgroundColor: colors.tableProgressBarTrackColor,
                      color: statusColor,
                    ),
                  ),
                  const SizedBox(height: 14),

                  // Pieces Map
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text(
                        'Pieces Map',
                        style: TextStyle(
                          color: colors.tableSecondaryTextColor,
                          fontSize: 11,
                          fontWeight: FontWeight.w500,
                        ),
                      ),
                      Text(
                        '${_torrent.completedPieces} / ${_torrent.totalPieces}',
                        style: TextStyle(
                          color: colors.tablePrimaryTextColor,
                          fontSize: 11,
                          fontFamily: 'monospace',
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 6),
                  TorrentPieceMap(
                    infoHash: _torrent.infoHash,
                    totalPieces: _torrent.totalPiecesCount,
                    completedPieces: _torrent.completedPiecesCount,
                    height: 20,
                    showStats: false,
                  ),
                ],
              ),
            ),
          ),

          const SizedBox(height: 10),

          // Card 2: Transfer Metrics
          Card(
            color: colors.dataCardBackgroundColor,
            elevation: 0,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(10),
              side: BorderSide(color: Color.fromRGBO(colors.dividerColor.red, colors.dividerColor.green, colors.dividerColor.blue, 0.6), width: 0.8),
            ),
            child: Padding(
              padding: const EdgeInsets.all(14),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Transfer Statistics',
                    style: TextStyle(
                      color: colors.tablePrimaryTextColor,
                      fontSize: 13,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  const SizedBox(height: 10),
                  _buildStatRow(
                    colors,
                    label1: 'Down Speed:',
                    value1: formatBytesDynamic(_torrent.downloadSpeed, isSpeed: true),
                    color1: colors.downloadSpeedColor,
                    label2: 'Up Speed:',
                    value2: formatBytesDynamic(_torrent.uploadSpeed, isSpeed: true),
                    color2: colors.uploadSpeedColor,
                  ),
                  _buildStatRow(
                    colors,
                    label1: 'Downloaded:',
                    value1: formatBytesDynamic(_torrent.downloaded),
                    label2: 'Uploaded:',
                    value2: formatBytesDynamic(_torrent.uploaded),
                  ),
                  _buildStatRow(
                    colors,
                    label1: 'Connected Peers:',
                    value1: '${_torrent.peers} peers',
                    label2: 'Share Ratio:',
                    value2: _torrent.ratioString,
                  ),
                  _buildStatRow(
                    colors,
                    label1: 'ETA Remaining:',
                    value1: _torrent.etaString,
                    label2: 'Status:',
                    value2: _torrent.status,
                    color2: statusColor,
                  ),
                ],
              ),
            ),
          ),

          const SizedBox(height: 10),

          // Card 3: Torrent Information
          Card(
            color: colors.dataCardBackgroundColor,
            elevation: 0,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(10),
              side: BorderSide(color: Color.fromRGBO(colors.dividerColor.red, colors.dividerColor.green, colors.dividerColor.blue, 0.6), width: 0.8),
            ),
            child: Padding(
              padding: const EdgeInsets.all(14),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Torrent Information',
                    style: TextStyle(
                      color: colors.tablePrimaryTextColor,
                      fontSize: 13,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  const SizedBox(height: 10),
                  _buildSingleInfoRow(
                    colors,
                    label: 'Total Size:',
                    value: formatBytesDynamic(_torrent.size),
                  ),
                  _buildSingleInfoRow(
                    colors,
                    label: 'Category:',
                    value: _torrent.category.isNotEmpty ? _torrent.category : 'None (Uncategorized)',
                  ),
                  Row(
                    children: [
                      Text(
                        'Sequential Download: ',
                        style: TextStyle(color: colors.dataLabelTextColor, fontSize: 12),
                      ),
                      const Spacer(),
                      Switch(
                        value: _torrent.sequential,
                        activeColor: colors.accentBlueColor,
                        onChanged: (val) => _toggleSequential(val),
                      ),
                    ],
                  ),
                  const SizedBox(height: 6),
                  Text(
                    'Info Hash:',
                    style: TextStyle(color: colors.dataLabelTextColor, fontSize: 12),
                  ),
                  const SizedBox(height: 3),
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
                    decoration: BoxDecoration(
                      color: colors.searchBarBackgroundColor,
                      borderRadius: BorderRadius.circular(6),
                      border: Border.all(color: colors.dividerColor, width: 0.6),
                    ),
                    child: Row(
                      children: [
                        Expanded(
                          child: SelectableText(
                            _torrent.infoHash,
                            style: TextStyle(
                              color: colors.tableAccentTextColor,
                              fontSize: 11,
                              fontFamily: 'monospace',
                            ),
                          ),
                        ),
                        InkWell(
                          onTap: () {
                            Clipboard.setData(ClipboardData(text: _torrent.infoHash));
                            ScaffoldMessenger.of(context).showSnackBar(
                              SnackBar(
                                content: Text(
                                  'Info Hash copied to clipboard',
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
                          },
                          child: Padding(
                            padding: const EdgeInsets.all(4),
                            child: Icon(Icons.copy_rounded, size: 14, color: colors.tableSecondaryTextColor),
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 20),
        ],
      ),
    );
  }

  Widget _buildStatRow(
    AppTheme colors, {
    required String label1,
    required String value1,
    Color? color1,
    required String label2,
    required String value2,
    Color? color2,
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(label1, style: TextStyle(color: colors.dataLabelTextColor, fontSize: 11)),
                const SizedBox(height: 1),
                Text(
                  value1,
                  style: TextStyle(
                    color: color1 ?? colors.tablePrimaryTextColor,
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ],
            ),
          ),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(label2, style: TextStyle(color: colors.dataLabelTextColor, fontSize: 11)),
                const SizedBox(height: 1),
                Text(
                  value2,
                  style: TextStyle(
                    color: color2 ?? colors.tablePrimaryTextColor,
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildSingleInfoRow(
    AppTheme colors, {
    required String label,
    required String value,
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        children: [
          Text(label, style: TextStyle(color: colors.dataLabelTextColor, fontSize: 12)),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              value,
              style: TextStyle(
                color: colors.tablePrimaryTextColor,
                fontSize: 12,
                fontWeight: FontWeight.w500,
              ),
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
    );
  }
}
