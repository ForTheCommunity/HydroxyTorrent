import 'dart:async';
import 'package:flutter/material.dart';
import 'package:hydroxy_rc/configs/converter.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/network/models.dart';

class TorrentPeersTab extends StatefulWidget {
  final String infoHash;
  final String torrentName;

  const TorrentPeersTab({
    super.key,
    required this.infoHash,
    required this.torrentName,
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
