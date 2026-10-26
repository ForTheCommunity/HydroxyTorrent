import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/state/torrents_bloc/torrents_bloc.dart';
import 'package:hydroxy_rc/ui/screens/main_content/models/torrent_structure_ext.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/torrent_data_table.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/torrent_details_panel.dart';

class TorrentDashboardScreen extends StatefulWidget {
  final String? filter;
  final String? searchQuery;
  final bool isRegex;

  const TorrentDashboardScreen({
    super.key,
    this.filter,
    this.searchQuery,
    this.isRegex = false,
  });

  @override
  State<TorrentDashboardScreen> createState() => _TorrentDashboardScreenState();
}

class _TorrentDashboardScreenState extends State<TorrentDashboardScreen> {
  final ScrollController _horizontalScrollController = ScrollController();
  final ScrollController _verticalScrollController = ScrollController();

  // Multi-selection state
  final Set<String> _selectedInfoHashes = <String>{};
  String? _primarySelectedIH;
  String? _anchorInfoHash;

  // Bottom Details Panel State
  bool _isPanelExpanded = false;
  int _activeTabIndex = 0;
  double _panelContentHeight = 220.0;
  late final ValueNotifier<double> _panelContentHeightNotifier =
      ValueNotifier<double>(_panelContentHeight);
  static const double _minPanelHeight = 100.0;
  static const double _maxPanelHeight = 500.0;
  static const double _tabBarHeight = 32.0;
  static const double _borderThickness = 1.0;

  // Sorting State
  String? _sortColumnKey;
  bool _isAscending = true;

  // Column Ordering & Widths
  final List<String> _columnKeys = [
    'Name',
    'Category',
    'Size',
    'Progress',
    'Status',
    'Peers',
    'Down Speed',
    'Up Speed',
    'ETA',
    'Ratio',
    'Downloaded',
    'Uploaded',
    'Total Pieces',
    'Completed Pieces',
    'Info Hash',
  ];

  final Map<String, double> _columnWidths = {
    'Name': 240.0,
    'Category': 120.0,
    'Size': 95.0,
    'Progress': 140.0,
    'Status': 110.0,
    'Peers': 80.0,
    'Down Speed': 110.0,
    'Up Speed': 110.0,
    'ETA': 90.0,
    'Ratio': 80.0,
    'Downloaded': 110.0,
    'Uploaded': 110.0,
    'Total Pieces': 100.0,
    'Completed Pieces': 120.0,
    'Info Hash': 320.0,
  };

  @override
  void initState() {
    super.initState();
    context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
  }

  @override
  void dispose() {
    _horizontalScrollController.dispose();
    _verticalScrollController.dispose();
    _panelContentHeightNotifier.dispose();
    super.dispose();
  }

  void _onSort(String columnKey) {
    setState(() {
      if (_sortColumnKey == columnKey) {
        _isAscending = !_isAscending;
      } else {
        _sortColumnKey = columnKey;
        // Speeds, Size, Progress, Peers, Downloaded, Uploaded should default to descending
        // so the active / fastest / largest torrents appear at the top first.
        switch (columnKey) {
          case 'Down Speed':
          case 'Up Speed':
          case 'Size':
          case 'Progress':
          case 'Downloaded':
          case 'Uploaded':
          case 'Peers':
          case 'Ratio':
            _isAscending = false;
            break;
          default:
            _isAscending = true;
            break;
        }
      }
    });
  }

  void _onColumnReordered(int oldIndex, int newIndex) {
    setState(() {
      if (newIndex > oldIndex) newIndex -= 1;
      final item = _columnKeys.removeAt(oldIndex);
      _columnKeys.insert(newIndex, item);
    });
  }

  void _onColumnResized(String columnKey, double newWidth) {
    setState(() {
      _columnWidths[columnKey] = newWidth;
    });
  }

  void _onSelectTorrent(
    TorrentStructure torrent, {
    bool isToggle = false,
    bool isRange = false,
    required List<TorrentStructure> currentList,
  }) {
    setState(() {
      if (isToggle) {
        if (_selectedInfoHashes.contains(torrent.infoHash)) {
          _selectedInfoHashes.remove(torrent.infoHash);
          if (_primarySelectedIH == torrent.infoHash) {
            _primarySelectedIH = _selectedInfoHashes.isNotEmpty
                ? _selectedInfoHashes.last
                : null;
          }
        } else {
          _selectedInfoHashes.add(torrent.infoHash);
          _primarySelectedIH = torrent.infoHash;
          _isPanelExpanded = true;
        }
        _anchorInfoHash = torrent.infoHash;
      } else if (isRange && (_anchorInfoHash != null || _primarySelectedIH != null)) {
        final anchor = _anchorInfoHash ?? _primarySelectedIH!;
        final anchorIndex = currentList.indexWhere(
          (t) => t.infoHash == anchor,
        );
        final currIndex = currentList.indexWhere(
          (t) => t.infoHash == torrent.infoHash,
        );

        if (anchorIndex != -1 && currIndex != -1) {
          final start = anchorIndex < currIndex ? anchorIndex : currIndex;
          final end = anchorIndex < currIndex ? currIndex : anchorIndex;
          _selectedInfoHashes.clear();
          for (int i = start; i <= end; i++) {
            _selectedInfoHashes.add(currentList[i].infoHash);
          }
          _primarySelectedIH = torrent.infoHash;
          _isPanelExpanded = true;
        } else {
          _selectedInfoHashes.clear();
          _selectedInfoHashes.add(torrent.infoHash);
          _primarySelectedIH = torrent.infoHash;
          _anchorInfoHash = torrent.infoHash;
        }
      } else {
        if (_selectedInfoHashes.length == 1 &&
            _selectedInfoHashes.contains(torrent.infoHash)) {
          _isPanelExpanded = !_isPanelExpanded;
        } else {
          _selectedInfoHashes.clear();
          _selectedInfoHashes.add(torrent.infoHash);
          _primarySelectedIH = torrent.infoHash;
          _anchorInfoHash = torrent.infoHash;
          _isPanelExpanded = true;
        }
      }
    });
  }

  void _onSelectAll(bool? selectAll, List<TorrentStructure> currentList) {
    setState(() {
      if (selectAll == true) {
        _selectedInfoHashes.addAll(currentList.map((t) => t.infoHash));
        if (_primarySelectedIH == null && currentList.isNotEmpty) {
          _primarySelectedIH = currentList.first.infoHash;
          _anchorInfoHash = currentList.first.infoHash;
        }
      } else {
        _selectedInfoHashes.clear();
        _primarySelectedIH = null;
        _anchorInfoHash = null;
      }
    });
  }

  Future<void> _resumeTorrents(List<TorrentStructure> torrents) async {
    if (torrents.isEmpty) return;
    final results = await Future.wait(
      torrents.map((t) => resumeTorrent(infoHash: t.infoHash)),
    );
    final count = results.where((r) => r).length;
    if (mounted) {
      context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
      if (torrents.length > 1) {
        final colors = context.appColors;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Resumed $count of ${torrents.length} torrents'),
            duration: const Duration(seconds: 2),
            backgroundColor: colors.snackBarBackgroundColor,
          ),
        );
      }
    }
  }

  Future<void> _pauseTorrents(List<TorrentStructure> torrents) async {
    if (torrents.isEmpty) return;
    final results = await Future.wait(
      torrents.map((t) => pauseTorrent(infoHash: t.infoHash)),
    );
    final count = results.where((r) => r).length;
    if (mounted) {
      context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
      if (torrents.length > 1) {
        final colors = context.appColors;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Stopped $count of ${torrents.length} torrents'),
            duration: const Duration(seconds: 2),
            backgroundColor: colors.snackBarBackgroundColor,
          ),
        );
      }
    }
  }

  Future<void> _reannounceTorrents(List<TorrentStructure> torrents) async {
    if (torrents.isEmpty) return;
    await Future.wait(
      torrents.map((t) => reannounceTorrent(infoHash: t.infoHash)),
    );
    if (mounted) {
      context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
    }
  }

  Future<void> _recheckTorrents(List<TorrentStructure> torrents) async {
    if (torrents.isEmpty) return;
    final colors = context.appColors;
    final isMultiple = torrents.length > 1;

    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogCtx) => AlertDialog(
        backgroundColor: colors.popupMenuBackgroundColor,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(8),
          side: BorderSide(color: colors.popupMenuBorderColor),
        ),
        title: Row(
          children: [
            Icon(Icons.sync_rounded, color: colors.accentBlueColor, size: 22),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                isMultiple ? 'Force Recheck (${torrents.length} Torrents)' : 'Force Recheck',
                style: TextStyle(
                  color: colors.tablePrimaryTextColor,
                  fontSize: 16,
                  fontWeight: FontWeight.bold,
                ),
              ),
            ),
          ],
        ),
        content: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 440),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              RichText(
                text: TextSpan(
                  style: TextStyle(
                    color: colors.tableSecondaryTextColor,
                    fontSize: 13,
                    height: 1.4,
                  ),
                  children: [
                    const TextSpan(text: 'Are you sure you want to force recheck '),
                    if (isMultiple)
                      TextSpan(
                        text: '${torrents.length} selected torrents',
                        style: TextStyle(
                          color: colors.tablePrimaryTextColor,
                          fontWeight: FontWeight.bold,
                        ),
                      )
                    else
                      TextSpan(
                        text: '"${torrents.first.name}"',
                        style: TextStyle(
                          color: colors.tablePrimaryTextColor,
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                    const TextSpan(
                      text: '?\n\nThis will pause active transfers and re-verify all downloaded data on disk against piece hashes. This may take some time depending on torrent size.',
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogCtx).pop(false),
            child: Text(
              'Cancel',
              style: TextStyle(color: colors.tableSecondaryTextColor),
            ),
          ),
          ElevatedButton(
            style: ElevatedButton.styleFrom(
              backgroundColor: colors.accentBlueColor,
              foregroundColor: Colors.white,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(4),
              ),
            ),
            onPressed: () => Navigator.of(dialogCtx).pop(true),
            child: const Text('Force Recheck'),
          ),
        ],
      ),
    );

    if (confirmed != true || !mounted) return;

    final results = await Future.wait(
      torrents.map((t) => recheckTorrent(infoHash: t.infoHash)),
    );
    final successCount = results.where((r) => r).length;

    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            isMultiple
                ? 'Force rechecking $successCount of ${torrents.length} torrents...'
                : 'Force rechecking "${torrents.first.name}"...',
          ),
          duration: const Duration(seconds: 2),
          backgroundColor: colors.snackBarBackgroundColor,
        ),
      );
      context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
    }
  }

  Future<void> _toggleSequentialTorrents(
    List<TorrentStructure> torrents,
    bool enable,
  ) async {
    if (torrents.isEmpty) return;
    final colors = context.appColors;
    await Future.wait(
      torrents.map(
        (t) => setSequentialDownload(infoHash: t.infoHash, sequential: enable),
      ),
    );
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            torrents.length == 1
                ? 'Sequential download for "${torrents.first.name}" ${enable ? "Enabled" : "Disabled"}'
                : 'Sequential download for ${torrents.length} torrents ${enable ? "Enabled" : "Disabled"}',
          ),
          duration: const Duration(seconds: 2),
          backgroundColor: colors.snackBarBackgroundColor,
        ),
      );
      context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
    }
  }

  Future<void> _removeTorrents(List<TorrentStructure> torrents) async {
    if (torrents.isEmpty) return;
    final colors = context.appColors;
    final isMultiple = torrents.length > 1;
    bool deleteDownloadedFiles = false;

    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogCtx) {
        return StatefulBuilder(
          builder: (context, setDialogState) {
            return AlertDialog(
              backgroundColor: colors.popupMenuBackgroundColor,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(8),
                side: BorderSide(color: colors.popupMenuBorderColor),
              ),
              title: Row(
                children: [
                  Icon(
                    Icons.delete_forever_rounded,
                    color: colors.accentRedColor,
                    size: 22,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      isMultiple
                          ? 'Remove ${torrents.length} Torrents'
                          : 'Remove Torrent',
                      style: TextStyle(
                        color: colors.tablePrimaryTextColor,
                        fontSize: 16,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                  ),
                ],
              ),
              content: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 440),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    RichText(
                      text: TextSpan(
                        style: TextStyle(
                          color: colors.tableSecondaryTextColor,
                          fontSize: 13,
                          height: 1.4,
                        ),
                        children: [
                          const TextSpan(text: 'Are you sure you want to remove '),
                          if (isMultiple)
                            TextSpan(
                              text: '${torrents.length} selected torrents',
                              style: TextStyle(
                                color: colors.tablePrimaryTextColor,
                                fontWeight: FontWeight.bold,
                              ),
                            )
                          else
                            TextSpan(
                              text: '"${torrents.first.name}"',
                              style: TextStyle(
                                color: colors.tablePrimaryTextColor,
                                fontWeight: FontWeight.bold,
                              ),
                            ),
                          const TextSpan(
                            text: ' from the transfer list?',
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(height: 16),
                    InkWell(
                      borderRadius: BorderRadius.circular(4),
                      onTap: () {
                        setDialogState(() {
                          deleteDownloadedFiles = !deleteDownloadedFiles;
                        });
                      },
                      child: Padding(
                        padding: const EdgeInsets.symmetric(vertical: 4),
                        child: Row(
                          children: [
                            Checkbox(
                              value: deleteDownloadedFiles,
                              activeColor: colors.accentRedColor,
                              checkColor: Colors.white,
                              side: BorderSide(
                                color: colors.tableSecondaryTextColor,
                                width: 1.5,
                              ),
                              onChanged: (val) {
                                setDialogState(() {
                                  deleteDownloadedFiles = val ?? false;
                                });
                              },
                            ),
                            const SizedBox(width: 6),
                            Expanded(
                              child: Text(
                                'Delete download files too',
                                style: TextStyle(
                                  color: deleteDownloadedFiles
                                      ? colors.accentRedColor
                                      : colors.tablePrimaryTextColor,
                                  fontSize: 13,
                                  fontWeight: deleteDownloadedFiles
                                      ? FontWeight.w600
                                      : FontWeight.normal,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.of(dialogCtx).pop(false),
                  child: Text(
                    'Cancel',
                    style: TextStyle(color: colors.tableSecondaryTextColor),
                  ),
                ),
                ElevatedButton(
                  style: ElevatedButton.styleFrom(
                    backgroundColor: colors.accentRedColor,
                    foregroundColor: Colors.white,
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(4),
                    ),
                  ),
                  onPressed: () => Navigator.of(dialogCtx).pop(true),
                  child: const Text('Remove'),
                ),
              ],
            );
          },
        );
      },
    );

    if (confirmed == true && mounted) {
      await Future.wait(
        torrents.map(
          (t) => removeTorrent(
            infoHash: t.infoHash,
            deleteFiles: deleteDownloadedFiles,
          ),
        ),
      );

      if (mounted) {
        setState(() {
          final removedHashes = torrents.map((t) => t.infoHash).toSet();
          _selectedInfoHashes.removeWhere((ih) => removedHashes.contains(ih));
          if (_primarySelectedIH != null && removedHashes.contains(_primarySelectedIH)) {
            _primarySelectedIH = _selectedInfoHashes.isNotEmpty
                ? _selectedInfoHashes.last
                : null;
          }
          if (_anchorInfoHash != null && removedHashes.contains(_anchorInfoHash)) {
            _anchorInfoHash = null;
          }
        });

        context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
      }
    }
  }

  Widget _buildBatchActionBar(
    BuildContext context,
    List<TorrentStructure> selectedTorrents,
  ) {
    final colors = context.appColors;
    final count = selectedTorrents.length;

    return Container(
      height: 38,
      padding: const EdgeInsets.symmetric(horizontal: 10),
      decoration: BoxDecoration(
        color: colors.tableHeaderBackgroundColor,
        border: Border(
          bottom: BorderSide(color: colors.tableDividerColor),
        ),
      ),
      child: Row(
        children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
            decoration: BoxDecoration(
              color: colors.accentGreenColor.withOpacity(0.15),
              borderRadius: BorderRadius.circular(4),
              border: Border.all(
                color: colors.accentGreenColor.withOpacity(0.4),
              ),
            ),
            child: Text(
              '$count selected',
              style: TextStyle(
                color: colors.accentGreenColor,
                fontWeight: FontWeight.bold,
                fontSize: 12,
              ),
            ),
          ),
          const SizedBox(width: 12),
          _batchActionButton(
            context: context,
            icon: Icons.play_arrow_rounded,
            iconColor: colors.accentGreenColor,
            label: 'Resume',
            tooltip: 'Resume selected torrents',
            onPressed: () => _resumeTorrents(selectedTorrents),
          ),
          const SizedBox(width: 6),
          _batchActionButton(
            context: context,
            icon: Icons.pause_rounded,
            iconColor: colors.accentAmberColor,
            label: 'Stop',
            tooltip: 'Stop selected torrents',
            onPressed: () => _pauseTorrents(selectedTorrents),
          ),
          const SizedBox(width: 6),
          _batchActionButton(
            context: context,
            icon: Icons.campaign_outlined,
            iconColor: colors.accentAmberColor,
            label: 'Reannounce',
            tooltip: 'Force reannounce selected torrents',
            onPressed: () => _reannounceTorrents(selectedTorrents),
          ),
          const SizedBox(width: 6),
          _batchActionButton(
            context: context,
            icon: Icons.sync_rounded,
            iconColor: colors.accentBlueColor,
            label: 'Recheck',
            tooltip: 'Force recheck selected torrents',
            onPressed: () => _recheckTorrents(selectedTorrents),
          ),
          const SizedBox(width: 6),
          PopupMenuButton<bool>(
            tooltip: 'Sequential download options',
            color: colors.popupMenuBackgroundColor,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(6),
              side: BorderSide(color: colors.popupMenuBorderColor),
            ),
            onSelected: (enable) => _toggleSequentialTorrents(selectedTorrents, enable),
            itemBuilder: (context) => [
              PopupMenuItem(
                value: true,
                child: Row(
                  children: [
                    Icon(Icons.check_box_outlined, color: colors.accentGreenColor, size: 16),
                    const SizedBox(width: 8),
                    Text('Enable Sequential', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 12)),
                  ],
                ),
              ),
              PopupMenuItem(
                value: false,
                child: Row(
                  children: [
                    Icon(Icons.check_box_outline_blank, color: colors.tableSecondaryTextColor, size: 16),
                    const SizedBox(width: 8),
                    Text('Disable Sequential', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 12)),
                  ],
                ),
              ),
            ],
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(4),
                border: Border.all(color: colors.tableDividerColor),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(Icons.format_list_numbered_rounded, size: 16, color: colors.accentBlueColor),
                  const SizedBox(width: 5),
                  Text(
                    'Sequential',
                    style: TextStyle(
                      color: colors.tablePrimaryTextColor,
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  const SizedBox(width: 2),
                  Icon(Icons.arrow_drop_down, size: 14, color: colors.tableSecondaryTextColor),
                ],
              ),
            ),
          ),
          const SizedBox(width: 6),
          _batchActionButton(
            context: context,
            icon: Icons.delete_outline_rounded,
            iconColor: colors.accentRedColor,
            label: 'Remove',
            tooltip: 'Remove selected torrents (Delete)',
            onPressed: () => _removeTorrents(selectedTorrents),
          ),
          const Spacer(),
          IconButton(
            icon: Icon(Icons.close_rounded, size: 16, color: colors.tableSecondaryTextColor),
            tooltip: 'Clear selection (Esc)',
            splashRadius: 14,
            onPressed: () {
              setState(() {
                _selectedInfoHashes.clear();
                _primarySelectedIH = null;
                _anchorInfoHash = null;
              });
            },
          ),
        ],
      ),
    );
  }

  Widget _batchActionButton({
    required BuildContext context,
    required IconData icon,
    required Color iconColor,
    required String label,
    required String tooltip,
    required VoidCallback onPressed,
  }) {
    final colors = context.appColors;
    return Tooltip(
      message: tooltip,
      child: InkWell(
        borderRadius: BorderRadius.circular(4),
        onTap: onPressed,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(4),
            border: Border.all(color: colors.tableDividerColor),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 16, color: iconColor),
              const SizedBox(width: 5),
              Text(
                label,
                style: TextStyle(
                  color: colors.tablePrimaryTextColor,
                  fontSize: 12,
                  fontWeight: FontWeight.w500,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  List<TorrentStructure> _getFilteredTorrents(List<TorrentStructure> list) {
    var result = list;

    if (widget.filter != null && widget.filter != 'all') {
      final f = widget.filter!;
      if (f.startsWith('status:')) {
        final targetStatus = f.substring('status:'.length).toLowerCase();
        result = result.where((t) {
          final st = t.status.toLowerCase();
          if (targetStatus == 'downloading') {
            return st == 'downloading' || st == 'stalled';
          }
          if (targetStatus == 'paused') {
            return st == 'paused' || st == 'stopped';
          }
          if (targetStatus == 'completed') {
            return st == 'completed' || st == 'seeding';
          }
          if (targetStatus == 'queued') {
            return st == 'queued';
          }
          return st == targetStatus;
        }).toList();
      } else if (f.startsWith('category:')) {
        final targetCat = f.substring('category:'.length).trim();
        if (targetCat.isEmpty) {
          result = result.where((t) => t.category.trim().isEmpty).toList();
        } else {
          result = result.where((t) {
            final c = t.category.trim();
            return c == targetCat || c.startsWith('$targetCat/');
          }).toList();
        }
      }
    }

    if (widget.searchQuery != null && widget.searchQuery!.trim().isNotEmpty) {
      final q = widget.searchQuery!.trim();
      if (widget.isRegex) {
        try {
          var pattern = q;
          if (pattern.startsWith('*') || pattern.startsWith('+') || pattern.startsWith('?')) {
            pattern = '.$pattern';
          }
          final regex = RegExp(pattern, caseSensitive: false);
          result = result.where((t) {
            return regex.hasMatch(t.name) ||
                regex.hasMatch(t.category) ||
                regex.hasMatch(t.infoHash);
          }).toList();
        } catch (_) {
          // If regex fails (e.g. incomplete syntax while typing), fall back to substring match
          final cleanQ = q.replaceAll(RegExp(r'^[+*?]+'), '').toLowerCase();
          result = result.where((t) {
            return t.name.toLowerCase().contains(cleanQ) ||
                t.category.toLowerCase().contains(cleanQ) ||
                t.infoHash.toLowerCase().contains(cleanQ);
          }).toList();
        }
      } else {
        final lowerQ = q.toLowerCase();
        result = result.where((t) {
          return t.name.toLowerCase().contains(lowerQ) ||
              t.category.toLowerCase().contains(lowerQ) ||
              t.infoHash.toLowerCase().contains(lowerQ);
        }).toList();
      }
    }

    return result;
  }

  List<TorrentStructure> _getSortedTorrents(List<TorrentStructure> list) {
    if (_sortColumnKey == null) return list;
    final sorted = List<TorrentStructure>.from(list);
    sorted.sort((a, b) {
      final int cmp;
      switch (_sortColumnKey) {
        case 'Name':
          cmp = a.name.toLowerCase().compareTo(b.name.toLowerCase());
          break;
        case 'Category':
          cmp = a.category.toLowerCase().compareTo(b.category.toLowerCase());
          break;
        case 'Size':
          cmp = a.sizeBytes.compareTo(b.sizeBytes);
          break;
        case 'Progress':
          cmp = a.progressValue.compareTo(b.progressValue);
          break;
        case 'Status':
          cmp = a.status.toLowerCase().compareTo(b.status.toLowerCase());
          break;
        case 'Peers':
          cmp = a.peersCount.compareTo(b.peersCount);
          break;
        case 'Down Speed':
          cmp = a.downloadSpeedValue.compareTo(b.downloadSpeedValue);
          break;
        case 'Up Speed':
          cmp = a.uploadSpeedValue.compareTo(b.uploadSpeedValue);
          break;
        case 'ETA':
          cmp = a.etaSeconds.compareTo(b.etaSeconds);
          break;
        case 'Ratio':
          cmp = a.ratio.compareTo(b.ratio);
          break;
        case 'Downloaded':
          cmp = a.downloadedBytes.compareTo(b.downloadedBytes);
          break;
        case 'Uploaded':
          cmp = a.uploadedBytes.compareTo(b.uploadedBytes);
          break;
        case 'Total Pieces':
          cmp = a.totalPiecesCount.compareTo(b.totalPiecesCount);
          break;
        case 'Completed Pieces':
          cmp = a.completedPiecesCount.compareTo(b.completedPiecesCount);
          break;
        case 'Info Hash':
          cmp = a.infoHash.compareTo(b.infoHash);
          break;
        default:
          cmp = 0;
      }
      return _isAscending ? cmp : -cmp;
    });
    return sorted;
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    return Scaffold(
      backgroundColor: colors.scaffoldBackgroundColor,
      body: BlocBuilder<TorrentsBloc, TorrentsState>(
        builder: (context, state) {
          final torrentsList = state.maybeWhen(
            loaded: (torrents) => torrents,
            orElse: () => const <TorrentStructure>[],
          );

          final filteredTorrents = _getFilteredTorrents(torrentsList);
          final sortedTorrents = _getSortedTorrents(filteredTorrents);

          TorrentStructure? selectedTorrent;
          if (_primarySelectedIH != null) {
            for (final t in torrentsList) {
              if (t.infoHash == _primarySelectedIH) {
                selectedTorrent = t;
                break;
              }
            }
          }
          if (selectedTorrent == null && _selectedInfoHashes.isNotEmpty) {
            for (final t in torrentsList) {
              if (_selectedInfoHashes.contains(t.infoHash)) {
                selectedTorrent = t;
                break;
              }
            }
          }

          final selectedTorrents = sortedTorrents
              .where((t) => _selectedInfoHashes.contains(t.infoHash))
              .toList();

          final Widget tableContent = state.when(
            initial: () => const Center(
              child: Text(
                'Connecting to daemon...',
                style: TextStyle(color: Colors.white54, fontSize: 13),
              ),
            ),
            loading: () => const Center(
              child: CircularProgressIndicator(
                color: Color.fromARGB(255, 0, 255, 8),
              ),
            ),
            loaded: (_) {
              return TorrentDataTable(
                torrents: sortedTorrents,
                selectedInfoHashes: _selectedInfoHashes,
                primarySelectedInfoHash: _primarySelectedIH,
                columnKeys: _columnKeys,
                columnWidths: _columnWidths,
                sortColumnKey: _sortColumnKey,
                isAscending: _isAscending,
                verticalScrollController: _verticalScrollController,
                horizontalScrollController: _horizontalScrollController,
                onSelectTorrent: (torrent, {isToggle = false, isRange = false}) {
                  _onSelectTorrent(
                    torrent,
                    isToggle: isToggle,
                    isRange: isRange,
                    currentList: sortedTorrents,
                  );
                },
                onColumnReordered: _onColumnReordered,
                onColumnResized: _onColumnResized,
                onSort: _onSort,
                onResumeTorrents: _resumeTorrents,
                onPauseTorrents: _pauseTorrents,
                onReannounceTorrents: _reannounceTorrents,
                onRecheckTorrents: _recheckTorrents,
                onToggleSequentialTorrents: _toggleSequentialTorrents,
                onRemoveTorrents: _removeTorrents,
              );
            },
          );

          final memoizedTable = RepaintBoundary(child: tableContent);

          return ValueListenableBuilder<double>(
            valueListenable: _panelContentHeightNotifier,
            builder: (context, panelContentHeight, _) {
              final double currentPanelTotalHeight =
                  (_isPanelExpanded
                      ? (panelContentHeight + _tabBarHeight)
                      : _tabBarHeight) +
                  _borderThickness;

              return Stack(
                children: [
                  // Top Table Area with isolated RepaintBoundary
                  Positioned.fill(
                    child: Padding(
                      padding: EdgeInsets.only(
                        left: 4.0,
                        right: 4.0,
                        top: 2.0,
                        bottom: currentPanelTotalHeight,
                      ),
                      child: CallbackShortcuts(
                        bindings: <ShortcutActivator, VoidCallback>{
                          const SingleActivator(LogicalKeyboardKey.delete): () {
                            if (selectedTorrents.isNotEmpty) {
                              _removeTorrents(selectedTorrents);
                            }
                          },
                          const SingleActivator(LogicalKeyboardKey.keyA, control: true): () {
                            _onSelectAll(true, sortedTorrents);
                          },
                          const SingleActivator(LogicalKeyboardKey.keyA, meta: true): () {
                            _onSelectAll(true, sortedTorrents);
                          },
                          const SingleActivator(LogicalKeyboardKey.escape): () {
                            if (_selectedInfoHashes.isNotEmpty) {
                              setState(() {
                                _selectedInfoHashes.clear();
                                _primarySelectedIH = null;
                                _anchorInfoHash = null;
                              });
                            } else if (_isPanelExpanded) {
                              setState(() {
                                _isPanelExpanded = false;
                              });
                            }
                          },
                        },
                        child: Focus(
                          autofocus: true,
                          child: Column(
                            children: [
                              if (selectedTorrents.length > 1)
                                _buildBatchActionBar(context, selectedTorrents),
                              Expanded(child: memoizedTable),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),

                  // Floating Overlay Bottom Details Panel
                  Positioned(
                    left: 0,
                    right: 0,
                    bottom: 0,
                    height: currentPanelTotalHeight,
                    child: RepaintBoundary(
                      child: TorrentDetailsPanel(
                        selectedTorrent: selectedTorrent,
                        isExpanded: _isPanelExpanded,
                        activeTabIndex: _activeTabIndex,
                        contentHeight: panelContentHeight,
                        minHeight: _minPanelHeight,
                        maxHeight: _maxPanelHeight,
                        tabBarHeight: _tabBarHeight,
                        borderThickness: _borderThickness,
                        onToggleExpand: () {
                          setState(() {
                            _isPanelExpanded = !_isPanelExpanded;
                          });
                        },
                        onTabChanged: (index) {
                          setState(() {
                            _activeTabIndex = index;
                          });
                        },
                        onHeightChanged: (newHeight) {
                          _panelContentHeight = newHeight;
                          _panelContentHeightNotifier.value = newHeight;
                        },
                      ),
                    ),
                  ),
                ],
              );
            },
          );
        },
      ),
    );
  }
}
