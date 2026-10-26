import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/network/models.dart';

class TorrentTrackersTab extends StatefulWidget {
  final String infoHash;
  final String torrentName;

  const TorrentTrackersTab({
    super.key,
    required this.infoHash,
    required this.torrentName,
  });

  @override
  State<TorrentTrackersTab> createState() => _TorrentTrackersTabState();
}

class _TorrentTrackersTabState extends State<TorrentTrackersTab> {
  Timer? _pollingTimer;
  List<TrackerStatsSnapshot>? _trackers;
  bool _isLoading = true;
  String? _error;

  int? _sortColumnIndex = 0; // Default sort by Tier
  bool _isAscending = true;

  @override
  void initState() {
    super.initState();
    _fetchTrackers();
    _pollingTimer = Timer.periodic(const Duration(seconds: 2), (_) {
      if (mounted) {
        _fetchTrackers(isPolling: true);
      }
    });
  }

  @override
  void didUpdateWidget(covariant TorrentTrackersTab oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.infoHash != widget.infoHash) {
      setState(() {
        _trackers = null;
        _isLoading = true;
        _error = null;
      });
      _fetchTrackers();
    }
  }

  @override
  void dispose() {
    _pollingTimer?.cancel();
    super.dispose();
  }

  Future<void> _fetchTrackers({bool isPolling = false}) async {
    try {
      final detail = await getTorrentDetail(infoHash: widget.infoHash);
      if (mounted) {
        setState(() {
          _trackers = detail.trackers;
          _isLoading = false;
          _error = null;
        });
      }
    } catch (e) {
      if (mounted && !isPolling) {
        setState(() {
          _error = e.toString();
          _isLoading = false;
        });
      }
    }
  }

  void _onSort(int columnIndex, bool ascending) {
    setState(() {
      _sortColumnIndex = columnIndex;
      _isAscending = ascending;
    });
  }

  List<TrackerStatsSnapshot> _getSortedTrackers(List<TrackerStatsSnapshot> list) {
    if (_sortColumnIndex == null) return list;
    final sorted = List<TrackerStatsSnapshot>.from(list);
    sorted.sort((a, b) {
      final int cmp;
      switch (_sortColumnIndex) {
        case 0: // Tier
          cmp = a.tier.compareTo(b.tier);
          break;
        case 1: // URL
          cmp = a.url.toLowerCase().compareTo(b.url.toLowerCase());
          break;
        case 2: // Status
          cmp = a.status.toLowerCase().compareTo(b.status.toLowerCase());
          break;
        case 3: // Peers
          cmp = a.peers.compareTo(b.peers);
          break;
        case 4: // Reannounce In
          cmp = a.nextAnnounce.compareTo(b.nextAnnounce);
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

    if (_isLoading && _trackers == null) {
      return Center(
        child: SizedBox(
          width: 24,
          height: 24,
          child: CircularProgressIndicator(
            strokeWidth: 2,
            color: colors.accentGreenColor,
          ),
        ),
      );
    }

    if (_error != null && _trackers == null) {
      return Center(
        child: Text(
          'Error loading trackers: $_error',
          style: TextStyle(color: colors.accentRedColor, fontSize: 11),
        ),
      );
    }

    final rawTrackers = _trackers ?? [];
    if (rawTrackers.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.location_on_outlined, size: 32, color: colors.tableSecondaryTextColor.withOpacity(0.4)),
            const SizedBox(height: 8),
            Text(
              'No trackers found for "${widget.torrentName}"',
              style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 12),
            ),
          ],
        ),
      );
    }

    final sortedTrackers = _getSortedTrackers(rawTrackers);

    final headerStyle = TextStyle(
      color: colors.tableHeaderTextColor,
      fontSize: 11,
      fontWeight: FontWeight.bold,
    );
    final cellStyle = TextStyle(
      color: colors.tableAccentTextColor,
      fontSize: 11,
    );
    final urlStyle = TextStyle(
      color: colors.tablePrimaryTextColor,
      fontSize: 11,
    );

    return LayoutBuilder(
      builder: (context, constraints) {
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.only(bottom: 6),
              child: Row(
                children: [
                  Text(
                    'Trackers (${sortedTrackers.length})',
                    style: TextStyle(
                      color: colors.tableHeaderTextColor,
                      fontSize: 12,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  const Spacer(),
                  InkWell(
                    borderRadius: BorderRadius.circular(4),
                    onTap: () async {
                      await reannounceTorrent(infoHash: widget.infoHash);
                      if (context.mounted) {
                        _fetchTrackers();
                      }
                    },
                    child: Container(
                      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                      decoration: BoxDecoration(
                        color: colors.accentAmberColor.withOpacity(0.15),
                        borderRadius: BorderRadius.circular(4),
                        border: Border.all(
                          color: colors.accentAmberColor.withOpacity(0.5),
                          width: 0.8,
                        ),
                      ),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(Icons.campaign_outlined, size: 14, color: colors.accentAmberColor),
                          const SizedBox(width: 4),
                          Text(
                            'Force Reannounce',
                            style: TextStyle(
                              color: colors.accentAmberColor,
                              fontSize: 11,
                              fontWeight: FontWeight.bold,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ],
              ),
            ),
            Expanded(
              child: SingleChildScrollView(
                scrollDirection: Axis.vertical,
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: ConstrainedBox(
              constraints: BoxConstraints(minWidth: constraints.maxWidth),
              child: DataTable(
                sortColumnIndex: _sortColumnIndex,
                sortAscending: _isAscending,
                headingRowHeight: 28,
                dataRowMinHeight: 24,
                dataRowMaxHeight: 26,
                columnSpacing: 24,
                horizontalMargin: 12,
                headingRowColor:
                    WidgetStateProperty.all(colors.tableHeaderBackgroundColor),
                columns: [
                  DataColumn(
                    label: Text('Tier', style: headerStyle),
                    numeric: true,
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('URL', style: headerStyle),
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('Status', style: headerStyle),
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('Peers', style: headerStyle),
                    numeric: true,
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('Reannounce In', style: headerStyle),
                    onSort: _onSort,
                  ),
                ],
                rows: sortedTrackers.map((tracker) {
                  return DataRow(
                    cells: [
                      DataCell(
                        Text('${tracker.tier}', style: cellStyle),
                      ),
                      DataCell(
                        InkWell(
                          onTap: () {
                            Clipboard.setData(ClipboardData(text: tracker.url));
                            ScaffoldMessenger.of(context).showSnackBar(
                              SnackBar(
                                content: Text('Copied tracker URL: ${tracker.url}'),
                                duration: const Duration(seconds: 1),
                                backgroundColor: colors.snackBarBackgroundColor,
                                behavior: SnackBarBehavior.floating,
                                width: 320,
                              ),
                            );
                          },
                          child: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              Text(tracker.url, style: urlStyle),
                              const SizedBox(width: 6),
                              Icon(
                                Icons.copy_rounded,
                                size: 12,
                                color: colors.tableSecondaryTextColor,
                              ),
                            ],
                          ),
                        ),
                      ),
                      DataCell(_buildStatusBadge(context, tracker.status)),
                      DataCell(
                        Text('${tracker.peers}', style: cellStyle),
                      ),
                      DataCell(
                        Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Icon(
                              Icons.timer_outlined,
                              size: 13,
                              color: colors.tableSecondaryTextColor,
                            ),
                            const SizedBox(width: 4),
                            Text(tracker.reannounceInString, style: cellStyle),
                          ],
                        ),
                      ),
                    ],
                  );
                }).toList(),
              ),
            ),
          ),
        ),
      ),
    ],
  );
},
);
  }

  Widget _buildStatusBadge(BuildContext context, String status) {
    final colors = context.appColors;
    Color badgeColor;
    final lower = status.toLowerCase();
    if (lower.contains('working') || lower.contains('ok')) {
      badgeColor = colors.accentGreenColor;
    } else if (lower.contains('updating')) {
      badgeColor = colors.accentBlueColor;
    } else if (lower.contains('error') || lower.contains('fail')) {
      badgeColor = colors.accentRedColor;
    } else if (lower.contains('not contacted')) {
      badgeColor = colors.tableSecondaryTextColor;
    } else {
      badgeColor = colors.accentAmberColor;
    }

    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 6,
          height: 6,
          decoration: BoxDecoration(
            color: badgeColor,
            shape: BoxShape.circle,
          ),
        ),
        const SizedBox(width: 5),
        Text(
          status,
          style: TextStyle(
            color: badgeColor,
            fontSize: 11,
            fontWeight: FontWeight.w500,
          ),
        ),
      ],
    );
  }
}
