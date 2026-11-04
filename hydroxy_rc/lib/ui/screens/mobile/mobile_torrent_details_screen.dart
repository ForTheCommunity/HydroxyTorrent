import 'dart:math' as math;
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
import 'package:hydroxy_rc/ui/screens/main_content/widgets/tabs/torrent_trackers_tab.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/torrent_piece_map.dart';
import 'package:hydroxy_rc/ui/widgets/info_card.dart';

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
          title: Text(
            _torrent.name,
            style: TextStyle(
              color: colors.titleBarTextColor,
              fontSize: 15,
              fontWeight: FontWeight.bold,
            ),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
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
        ),
        body: Column(
          children: [
            // TabBar placed cleanly below AppBar to prevent AppBar squashing/clipping
            Container(
              width: double.infinity,
              color: colors.detailsPanelBackgroundColor,
              child: TabBar(
                controller: _tabController,
                isScrollable: true,
                tabAlignment: TabAlignment.start,
                indicatorColor: colors.tabBarActiveIndicatorColor,
                indicatorWeight: 2.5,
                labelColor: colors.tabBarActiveTextColor,
                unselectedLabelColor: colors.tabBarInactiveTextColor,
                labelStyle: const TextStyle(fontSize: 12, fontWeight: FontWeight.bold),
                unselectedLabelStyle: const TextStyle(fontSize: 12),
                tabs: const [
                  Tab(
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(Icons.info_outline, size: 15),
                        SizedBox(width: 6),
                        Text('General'),
                      ],
                    ),
                  ),
                  Tab(
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(Icons.location_on_outlined, size: 15),
                        SizedBox(width: 6),
                        Text('Trackers'),
                      ],
                    ),
                  ),
                  Tab(
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(Icons.people_outline, size: 15),
                        SizedBox(width: 6),
                        Text('Peers'),
                      ],
                    ),
                  ),
                  Tab(
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(Icons.storage_outlined, size: 15),
                        SizedBox(width: 6),
                        Text('HTTP Sources'),
                      ],
                    ),
                  ),
                  Tab(
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(Icons.folder_open_rounded, size: 15),
                        SizedBox(width: 6),
                        Text('Content'),
                      ],
                    ),
                  ),
                ],
              ),
            ),
            Divider(color: colors.dividerColor, height: 1),

            // Tab Views
            Expanded(
              child: TabBarView(
                controller: _tabController,
                children: [
                  // Tab 0: General Tab
                  _buildMobileGeneralTab(context),

                  // Tab 1: Trackers
                  TorrentTrackersTab(
                    infoHash: _torrent.infoHash,
                    torrentName: _torrent.name,
                    compact: true,
                  ),

                  // Tab 2: Peers
                  TorrentPeersTab(
                    infoHash: _torrent.infoHash,
                    torrentName: _torrent.name,
                    compact: true,
                  ),

                  // Tab 3: HTTP Sources
                  _buildMobileHttpSourcesTab(context),

                  // Tab 4: Content / Files
                  TorrentFilesTab(
                    infoHash: _torrent.infoHash,
                    torrentName: _torrent.name,
                    compact: true,
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildMobileHttpSourcesTab(BuildContext context) {
    final colors = context.appColors;
    return ListView(
      padding: const EdgeInsets.all(12),
      children: [
        InfoCard(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 28),
          child: Column(
            children: [
              Icon(
                Icons.storage_outlined,
                size: 32,
                color: Color.fromRGBO(colors.tableSecondaryTextColor.red, colors.tableSecondaryTextColor.green, colors.tableSecondaryTextColor.blue, 0.5),
              ),
              const SizedBox(height: 10),
              Text(
                'No HTTP Sources',
                style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 14, fontWeight: FontWeight.bold),
              ),
              const SizedBox(height: 4),
              Text(
                'This torrent has no web seeds (HTTP/URL sources).',
                textAlign: TextAlign.center,
                style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 12),
              ),
            ],
          ),
        ),
      ],
    );
  }

  Widget _buildMobileGeneralTab(BuildContext context) {
    final colors = context.appColors;
    final statusColor = _getStatusColor(context, _torrent.status);
    final pieceLength = _torrent.totalPiecesCount > 0
        ? (_torrent.sizeBytes / _torrent.totalPiecesCount).round()
        : 0;
    final remainingBytes = math.max(0, _torrent.sizeBytes - _torrent.downloadedBytes);

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
              side: BorderSide(
                color: Color.fromRGBO(colors.dividerColor.red, colors.dividerColor.green, colors.dividerColor.blue, 0.6),
                width: 0.8,
              ),
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
                      Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Container(
                            padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2.5),
                            decoration: BoxDecoration(
                              color: Color.fromRGBO(statusColor.red, statusColor.green, statusColor.blue, 0.15),
                              borderRadius: BorderRadius.circular(6),
                              border: Border.all(
                                color: Color.fromRGBO(statusColor.red, statusColor.green, statusColor.blue, 0.4),
                                width: 0.8,
                              ),
                            ),
                            child: Text(
                              _torrent.status,
                              style: TextStyle(
                                color: statusColor,
                                fontSize: 11,
                                fontWeight: FontWeight.bold,
                              ),
                            ),
                          ),
                          const SizedBox(width: 8),
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
                    ],
                  ),
                  const SizedBox(height: 10),
                  ClipRRect(
                    borderRadius: BorderRadius.circular(4),
                    child: LinearProgressIndicator(
                      value: _torrent.progressValue,
                      minHeight: 10,
                      backgroundColor: colors.tableProgressBarTrackColor,
                      color: statusColor,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text(
                        '${formatBytesDynamic(_torrent.downloadedBytes.toString())} of ${formatBytesDynamic(_torrent.size)}',
                        style: TextStyle(
                          color: colors.tablePrimaryTextColor,
                          fontSize: 12,
                          fontWeight: FontWeight.w500,
                        ),
                      ),
                      Text(
                        _torrent.progressValue >= 1.0
                            ? 'Completed'
                            : '${formatBytesDynamic(remainingBytes.toString())} left',
                        style: TextStyle(
                          color: colors.tableSecondaryTextColor,
                          fontSize: 11,
                        ),
                      ),
                    ],
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
                        '${_torrent.completedPieces} / ${_torrent.totalPieces} pieces',
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
                    height: 22,
                    showStats: true,
                  ),
                  if (pieceLength > 0) ...[
                    const SizedBox(height: 6),
                    Text(
                      'Piece size: ${formatBytesDynamic(pieceLength.toString())}  •  ${_torrent.totalPieces} total pieces',
                      style: TextStyle(
                        color: colors.tableSecondaryTextColor,
                        fontSize: 10.5,
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ),

          const SizedBox(height: 10),

          // Card 2: Transfer Statistics
          Card(
            color: colors.dataCardBackgroundColor,
            elevation: 0,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(10),
              side: BorderSide(
                color: Color.fromRGBO(colors.dividerColor.red, colors.dividerColor.green, colors.dividerColor.blue, 0.6),
                width: 0.8,
              ),
            ),
            child: Padding(
              padding: const EdgeInsets.all(14),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Icon(Icons.swap_vert_rounded, size: 16, color: colors.accentBlueColor),
                      const SizedBox(width: 6),
                      Text(
                        'Transfer Statistics',
                        style: TextStyle(
                          color: colors.tablePrimaryTextColor,
                          fontSize: 13,
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 12),
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
                    value1: formatBytesDynamic(_torrent.downloadedBytes.toString()),
                    label2: 'Uploaded:',
                    value2: formatBytesDynamic(_torrent.uploaded),
                  ),
                  _buildStatRow(
                    colors,
                    label1: 'ETA Remaining:',
                    value1: _torrent.etaString,
                    label2: 'Share Ratio:',
                    value2: _torrent.ratioString,
                  ),
                  _buildStatRow(
                    colors,
                    label1: 'Connected Peers:',
                    value1: '${_torrent.peers} connected',
                    label2: 'Pieces Progress:',
                    value2: '${_torrent.completedPieces} / ${_torrent.totalPieces}',
                  ),
                  _buildStatRow(
                    colors,
                    label1: 'Status:',
                    value1: _torrent.status,
                    color1: statusColor,
                    label2: 'Data Remaining:',
                    value2: formatBytesDynamic(remainingBytes.toString()),
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
              side: BorderSide(
                color: Color.fromRGBO(colors.dividerColor.red, colors.dividerColor.green, colors.dividerColor.blue, 0.6),
                width: 0.8,
              ),
            ),
            child: Padding(
              padding: const EdgeInsets.all(14),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Icon(Icons.info_outline_rounded, size: 16, color: colors.titleBarLogoColor),
                      const SizedBox(width: 6),
                      Text(
                        'Torrent Information',
                        style: TextStyle(
                          color: colors.tablePrimaryTextColor,
                          fontSize: 13,
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 12),
                  // Name (Full name, selectable)
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Name:',
                        style: TextStyle(color: colors.dataLabelTextColor, fontSize: 11),
                      ),
                      const SizedBox(height: 2),
                      SelectableText(
                        _torrent.name,
                        style: TextStyle(
                          color: colors.tablePrimaryTextColor,
                          fontSize: 12,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 10),
                  _buildStatRow(
                    colors,
                    label1: 'Total Size:',
                    value1: formatBytesDynamic(_torrent.size),
                    label2: 'Storage Target:',
                    value2: _torrent.storage.isNotEmpty ? _torrent.storage : 'default',
                  ),
                  _buildStatRow(
                    colors,
                    label1: 'Category:',
                    value1: _torrent.category.isNotEmpty ? _torrent.category : 'None (Uncategorized)',
                    label2: 'Piece Size:',
                    value2: pieceLength > 0 ? formatBytesDynamic(pieceLength.toString()) : '—',
                  ),
                  _buildStatRow(
                    colors,
                    label1: 'Completed Pieces:',
                    value1: '${_torrent.completedPieces} pieces',
                    label2: 'Total Pieces:',
                    value2: '${_torrent.totalPieces} pieces',
                  ),
                  const SizedBox(height: 4),
                  Row(
                    children: [
                      Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'Sequential Download: ',
                            style: TextStyle(color: colors.dataLabelTextColor, fontSize: 11),
                          ),
                          const SizedBox(height: 2),
                          Text(
                            _torrent.sequential ? 'Enabled (Sequential Order)' : 'Disabled (Rarest-First)',
                            style: TextStyle(
                              color: _torrent.sequential ? colors.accentBlueColor : colors.tableSecondaryTextColor,
                              fontSize: 12,
                              fontWeight: FontWeight.w500,
                            ),
                          ),
                        ],
                      ),
                      const Spacer(),
                      Switch(
                        value: _torrent.sequential,
                        activeColor: colors.accentBlueColor,
                        onChanged: (val) => _toggleSequential(val),
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'Info Hash (SHA-1):',
                    style: TextStyle(color: colors.dataLabelTextColor, fontSize: 11),
                  ),
                  const SizedBox(height: 4),
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
