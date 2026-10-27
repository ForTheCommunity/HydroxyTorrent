import 'dart:async';
import 'package:flutter/material.dart';
import 'package:hydroxy_rc/configs/converter.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/ui/widgets/info_card.dart';

class TorrentPeersTab extends StatefulWidget {
  final String infoHash;
  final String torrentName;

  /// When true, peers are shown as a vertical list of cards (mobile layout)
  /// instead of a wide data table.
  final bool compact;

  const TorrentPeersTab({
    super.key,
    required this.infoHash,
    required this.torrentName,
    this.compact = false,
  });

  @override
  State<TorrentPeersTab> createState() => _TorrentPeersTabState();
}

class _TorrentPeersTabState extends State<TorrentPeersTab> {
  Timer? _pollingTimer;
  List<PeerStatsSnapshot>? _peers;
  bool _isLoading = true;
  String? _error;

  // Sorting State
  int? _sortColumnIndex = 4; // Default to Down Speed descending
  bool _isAscending = false;

  @override
  void initState() {
    super.initState();
    _fetchPeers();
    _pollingTimer = Timer.periodic(const Duration(seconds: 2), (_) {
      if (mounted) {
        _fetchPeers(isPolling: true);
      }
    });
  }

  @override
  void didUpdateWidget(covariant TorrentPeersTab oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.infoHash != widget.infoHash) {
      setState(() {
        _peers = null;
        _isLoading = true;
        _error = null;
      });
      _fetchPeers();
    }
  }

  @override
  void dispose() {
    _pollingTimer?.cancel();
    super.dispose();
  }

  Future<void> _fetchPeers({bool isPolling = false}) async {
    try {
      final detail = await getTorrentDetail(infoHash: widget.infoHash);
      if (mounted) {
        setState(() {
          _peers = detail.peerList;
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

  List<PeerStatsSnapshot> _getSortedPeers(List<PeerStatsSnapshot> list) {
    if (_sortColumnIndex == null) return list;
    final sorted = List<PeerStatsSnapshot>.from(list);
    sorted.sort((a, b) {
      final int cmp;
      switch (_sortColumnIndex) {
        case 0: // IP:Port
          cmp = a.endpoint.compareTo(b.endpoint);
          break;
        case 1: // Client
          cmp = a.clientId.toLowerCase().compareTo(b.clientId.toLowerCase());
          break;
        case 2: // State
          cmp = a.state.toLowerCase().compareTo(b.state.toLowerCase());
          break;
        case 3: // Progress
          cmp = a.progress.compareTo(b.progress);
          break;
        case 4: // Down Speed
          cmp = a.downloadSpeed.compareTo(b.downloadSpeed);
          break;
        case 5: // Up Speed
          cmp = a.uploadSpeed.compareTo(b.uploadSpeed);
          break;
        case 6: // Downloaded
          cmp = a.downloadedBytes.compareTo(b.downloadedBytes);
          break;
        case 7: // Uploaded
          cmp = a.uploadedBytes.compareTo(b.uploadedBytes);
          break;
        case 8: // Pieces
          cmp = a.piecesReceived.compareTo(b.piecesReceived);
          break;
        case 9: // Duration
          cmp = a.connectedDurationSecs.compareTo(b.connectedDurationSecs);
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

    if (_isLoading && _peers == null) {
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

    if (_error != null && _peers == null) {
      return Center(
        child: Text(
          'Error loading peers: $_error',
          style: TextStyle(color: colors.accentRedColor, fontSize: 11),
        ),
      );
    }

    final rawPeers = _peers ?? [];
    if (rawPeers.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.people_outline, size: 32, color: Color.fromRGBO(colors.tableSecondaryTextColor.red, colors.tableSecondaryTextColor.green, colors.tableSecondaryTextColor.blue, 0.4)),
            const SizedBox(height: 8),
            Text(
              'No peers connected for "${widget.torrentName}"',
              style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 12),
            ),
          ],
        ),
      );
    }

    final sortedPeers = _getSortedPeers(rawPeers);

    if (widget.compact) {
      return _buildCompactList(context, sortedPeers);
    }

    final headerStyle = TextStyle(
      color: colors.tableHeaderTextColor,
      fontSize: 11,
      fontWeight: FontWeight.bold,
    );
    final cellStyle = TextStyle(
      color: colors.tableAccentTextColor,
      fontSize: 11,
    );

    return LayoutBuilder(
      builder: (context, constraints) {
        return SingleChildScrollView(
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
                columnSpacing: 20,
                horizontalMargin: 12,
                headingRowColor:
                    WidgetStateProperty.all(colors.tableHeaderBackgroundColor),
                columns: [
                  DataColumn(
                    label: Text('IP:Port', style: headerStyle),
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('Client', style: headerStyle),
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('State', style: headerStyle),
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('Progress', style: headerStyle),
                    numeric: true,
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('Down Speed', style: headerStyle),
                    numeric: true,
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('Up Speed', style: headerStyle),
                    numeric: true,
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('Downloaded', style: headerStyle),
                    numeric: true,
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('Uploaded', style: headerStyle),
                    numeric: true,
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('Pieces', style: headerStyle),
                    numeric: true,
                    onSort: _onSort,
                  ),
                  DataColumn(
                    label: Text('Duration', style: headerStyle),
                    numeric: true,
                    onSort: _onSort,
                  ),
                ],
                rows: sortedPeers.map((peer) {
                  return DataRow(
                    cells: [
                      DataCell(
                        Text(peer.endpoint, style: cellStyle),
                      ),
                      DataCell(
                        Text(
                          peer.clientId.isNotEmpty ? peer.clientId : 'Unknown',
                          style: cellStyle,
                        ),
                      ),
                      DataCell(_buildStateBadge(context, peer.state)),
                      DataCell(
                        Text('${peer.progress.toStringAsFixed(1)}%', style: cellStyle),
                      ),
                      DataCell(
                        Text(
                          formatBytesDynamic(
                            peer.downloadSpeed.toStringAsFixed(0),
                            isSpeed: true,
                          ),
                          style: cellStyle,
                        ),
                      ),
                      DataCell(
                        Text(
                          formatBytesDynamic(
                            peer.uploadSpeed.toStringAsFixed(0),
                            isSpeed: true,
                          ),
                          style: cellStyle,
                        ),
                      ),
                      DataCell(
                        Text(
                          formatBytesDynamic(peer.downloadedBytes.toString()),
                          style: cellStyle,
                        ),
                      ),
                      DataCell(
                        Text(
                          formatBytesDynamic(peer.uploadedBytes.toString()),
                          style: cellStyle,
                        ),
                      ),
                      DataCell(
                        Text('${peer.piecesReceived}', style: cellStyle),
                      ),
                      DataCell(
                        Text('${peer.connectedDurationSecs}s', style: cellStyle),
                      ),
                    ],
                  );
                }).toList(),
              ),
            ),
          ),
        );
      },
    );
  }

  static const _compactSortOptions = <int, String>{
    4: 'Down Speed',
    5: 'Up Speed',
    3: 'Progress',
    6: 'Downloaded',
    7: 'Uploaded',
    9: 'Duration',
    1: 'Client',
    0: 'IP:Port',
  };

  String _formatDuration(int secs) {
    if (secs < 60) return '${secs}s';
    final m = secs ~/ 60;
    if (m < 60) return '${m}m ${secs % 60}s';
    final h = m ~/ 60;
    return '${h}h ${m % 60}m';
  }

  Widget _buildCompactList(BuildContext context, List<PeerStatsSnapshot> peers) {
    final colors = context.appColors;

    return ListView(
      padding: const EdgeInsets.all(12),
      children: [
        Padding(
          padding: const EdgeInsets.only(bottom: 10),
          child: Row(
            children: [
              Text(
                'Peers (${peers.length})',
                style: TextStyle(
                  color: colors.tablePrimaryTextColor,
                  fontSize: 13,
                  fontWeight: FontWeight.bold,
                ),
              ),
              const Spacer(),
              PopupMenuButton<int>(
                tooltip: 'Sort peers',
                color: colors.popupMenuBackgroundColor,
                initialValue: _sortColumnIndex,
                onSelected: (idx) {
                  // Selecting the active option flips the direction.
                  _onSort(idx, idx == _sortColumnIndex ? !_isAscending : false);
                },
                itemBuilder: (_) => _compactSortOptions.entries
                    .map((e) => PopupMenuItem<int>(
                          value: e.key,
                          height: 36,
                          child: Text(e.value, style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
                        ))
                    .toList(),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(
                      _isAscending ? Icons.arrow_upward_rounded : Icons.arrow_downward_rounded,
                      size: 14,
                      color: colors.tableSecondaryTextColor,
                    ),
                    const SizedBox(width: 4),
                    Text(
                      _compactSortOptions[_sortColumnIndex] ?? 'Sort',
                      style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 12),
                    ),
                    Icon(Icons.arrow_drop_down, size: 18, color: colors.tableSecondaryTextColor),
                  ],
                ),
              ),
            ],
          ),
        ),
        ...peers.map((peer) {
          final stateColor = _stateColor(context, peer.state);
          final progress = (peer.progress / 100).clamp(0.0, 1.0);
          return InfoCard(
            accentColor: stateColor,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            peer.endpoint,
                            style: TextStyle(
                              color: colors.tablePrimaryTextColor,
                              fontSize: 12,
                              fontWeight: FontWeight.w600,
                              fontFamily: 'monospace',
                            ),
                            overflow: TextOverflow.ellipsis,
                          ),
                          const SizedBox(height: 2),
                          Text(
                            peer.clientId.isNotEmpty ? peer.clientId : 'Unknown client',
                            style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 11),
                            overflow: TextOverflow.ellipsis,
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 8),
                    _buildStateBadge(context, peer.state),
                  ],
                ),
                const SizedBox(height: 10),
                Row(
                  children: [
                    Expanded(
                      child: ClipRRect(
                        borderRadius: BorderRadius.circular(3),
                        child: LinearProgressIndicator(
                          value: progress,
                          minHeight: 5,
                          backgroundColor: colors.tableProgressBarTrackColor,
                          color: colors.accentBlueColor,
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Text(
                      '${peer.progress.toStringAsFixed(1)}%',
                      style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 11, fontWeight: FontWeight.w500),
                    ),
                  ],
                ),
                const SizedBox(height: 10),
                InfoFieldGrid(
                  columns: 3,
                  fields: [
                    InfoField(
                      label: 'Down',
                      value: formatBytesDynamic(peer.downloadSpeed.toStringAsFixed(0), isSpeed: true),
                      valueColor: colors.downloadSpeedColor,
                      icon: Icons.arrow_downward_rounded,
                    ),
                    InfoField(
                      label: 'Up',
                      value: formatBytesDynamic(peer.uploadSpeed.toStringAsFixed(0), isSpeed: true),
                      valueColor: colors.uploadSpeedColor,
                      icon: Icons.arrow_upward_rounded,
                    ),
                    InfoField(label: 'Pieces', value: '${peer.piecesReceived}'),
                    InfoField(label: 'Downloaded', value: formatBytesDynamic(peer.downloadedBytes.toString())),
                    InfoField(label: 'Uploaded', value: formatBytesDynamic(peer.uploadedBytes.toString())),
                    InfoField(label: 'Connected', value: _formatDuration(peer.connectedDurationSecs)),
                  ],
                ),
              ],
            ),
          );
        }),
      ],
    );
  }

  Color _stateColor(BuildContext context, String state) {
    final colors = context.appColors;
    switch (state.toLowerCase()) {
      case 'downloading':
        return colors.accentGreenColor;
      case 'connected':
        return colors.accentBlueColor;
      case 'choked':
      case 'connecting':
        return colors.accentAmberColor;
      default:
        return colors.tableSecondaryTextColor;
    }
  }

  Widget _buildStateBadge(BuildContext context, String state) {
    final colors = context.appColors;
    Color badgeColor;
    switch (state.toLowerCase()) {
      case 'downloading':
        badgeColor = colors.accentGreenColor;
        break;
      case 'connected':
        badgeColor = colors.accentBlueColor;
        break;
      case 'choked':
        badgeColor = colors.accentAmberColor;
        break;
      case 'connecting':
        badgeColor = colors.accentAmberColor;
        break;
      default:
        badgeColor = colors.tableSecondaryTextColor;
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
          state,
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
