import 'package:data_table_2/data_table_2.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:hydroxy_rc/configs/converter.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/ui/screens/main_content/models/torrent_structure_ext.dart';

class TorrentDataTable extends StatelessWidget {
  final List<TorrentStructure> torrents;
  final Set<String> selectedInfoHashes;
  final String? primarySelectedInfoHash;
  final List<String> columnKeys;
  final Map<String, double> columnWidths;
  final String? sortColumnKey;
  final bool isAscending;
  final ScrollController verticalScrollController;
  final ScrollController horizontalScrollController;
  final void Function(TorrentStructure torrent, {bool isToggle, bool isRange}) onSelectTorrent;
  final void Function(int oldIndex, int newIndex) onColumnReordered;
  final void Function(String columnKey, double newWidth) onColumnResized;
  final void Function(String columnKey) onSort;
  final void Function(List<TorrentStructure> torrents)? onResumeTorrents;
  final void Function(List<TorrentStructure> torrents)? onPauseTorrents;
  final void Function(List<TorrentStructure> torrents)? onReannounceTorrents;
  final void Function(List<TorrentStructure> torrents)? onRecheckTorrents;
  final void Function(List<TorrentStructure> torrents, bool enable)? onToggleSequentialTorrents;
  final void Function(List<TorrentStructure> torrents)? onRemoveTorrents;

  const TorrentDataTable({
    super.key,
    required this.torrents,
    required this.selectedInfoHashes,
    this.primarySelectedInfoHash,
    required this.columnKeys,
    required this.columnWidths,
    required this.sortColumnKey,
    required this.isAscending,
    required this.verticalScrollController,
    required this.horizontalScrollController,
    required this.onSelectTorrent,
    required this.onColumnReordered,
    required this.onColumnResized,
    required this.onSort,
    this.onResumeTorrents,
    this.onPauseTorrents,
    this.onReannounceTorrents,
    this.onRecheckTorrents,
    this.onToggleSequentialTorrents,
    this.onRemoveTorrents,
  });

  void _showContextMenu(
    BuildContext context,
    Offset position,
    List<TorrentStructure> targetTorrents,
  ) {
    if (targetTorrents.isEmpty) return;
    final colors = context.appColors;
    final isMultiple = targetTorrents.length > 1;
    final count = targetTorrents.length;

    final singleTorrent = targetTorrents.first;
    final isSinglePaused = singleTorrent.status.toLowerCase() == 'paused' ||
        singleTorrent.status.toLowerCase() == 'stopped';

    showMenu<String>(
      context: context,
      position: RelativeRect.fromLTRB(
        position.dx,
        position.dy,
        position.dx + 1,
        position.dy + 1,
      ),
      color: colors.popupMenuBackgroundColor,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(6),
        side: BorderSide(color: colors.popupMenuBorderColor),
      ),
      items: [
        if (isMultiple) ...[
          PopupMenuItem<String>(
            value: 'resume',
            child: Row(
              children: [
                Icon(Icons.play_arrow_rounded, color: colors.accentGreenColor, size: 18),
                const SizedBox(width: 8),
                Text('Resume ($count)', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
              ],
            ),
          ),
          PopupMenuItem<String>(
            value: 'pause',
            child: Row(
              children: [
                Icon(Icons.pause_rounded, color: colors.accentAmberColor, size: 18),
                const SizedBox(width: 8),
                Text('Stop ($count)', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
              ],
            ),
          ),
          PopupMenuDivider(height: 1),
          PopupMenuItem<String>(
            value: 'reannounce',
            child: Row(
              children: [
                Icon(Icons.campaign_outlined, color: colors.accentAmberColor, size: 18),
                const SizedBox(width: 8),
                Text('Force Reannounce ($count)', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
              ],
            ),
          ),
          PopupMenuItem<String>(
            value: 'recheck',
            child: Row(
              children: [
                Icon(Icons.sync_rounded, color: colors.accentBlueColor, size: 18),
                const SizedBox(width: 8),
                Text('Force Recheck ($count)...', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
              ],
            ),
          ),
          PopupMenuDivider(height: 1),
          PopupMenuItem<String>(
            value: 'seq_enable',
            child: Row(
              children: [
                Icon(Icons.check_box_outlined, color: colors.accentGreenColor, size: 18),
                const SizedBox(width: 8),
                Text('Enable Sequential ($count)', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
              ],
            ),
          ),
          PopupMenuItem<String>(
            value: 'seq_disable',
            child: Row(
              children: [
                Icon(Icons.check_box_outline_blank, color: colors.tableSecondaryTextColor, size: 18),
                const SizedBox(width: 8),
                Text('Disable Sequential ($count)', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
              ],
            ),
          ),
          PopupMenuDivider(height: 1),
          PopupMenuItem<String>(
            value: 'remove',
            child: Row(
              children: [
                Icon(Icons.delete_outline_rounded, color: colors.accentRedColor, size: 18),
                const SizedBox(width: 8),
                Text('Remove ($count)...', style: TextStyle(color: colors.accentRedColor, fontSize: 13)),
              ],
            ),
          ),
        ] else ...[
          if (isSinglePaused)
            PopupMenuItem<String>(
              value: 'resume',
              child: Row(
                children: [
                  Icon(Icons.play_arrow_rounded, color: colors.accentGreenColor, size: 18),
                  const SizedBox(width: 8),
                  Text('Resume', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
                ],
              ),
            )
          else
            PopupMenuItem<String>(
              value: 'pause',
              child: Row(
                children: [
                  Icon(Icons.pause_rounded, color: colors.accentAmberColor, size: 18),
                  const SizedBox(width: 8),
                  Text('Stop', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
                ],
              ),
            ),
          PopupMenuDivider(height: 1),
          PopupMenuItem<String>(
            value: 'reannounce',
            child: Row(
              children: [
                Icon(Icons.campaign_outlined, color: colors.accentAmberColor, size: 18),
                const SizedBox(width: 8),
                Text('Force Reannounce', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
              ],
            ),
          ),
          PopupMenuItem<String>(
            value: 'recheck',
            child: Row(
              children: [
                Icon(Icons.sync_rounded, color: colors.accentBlueColor, size: 18),
                const SizedBox(width: 8),
                Text('Force Recheck...', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
              ],
            ),
          ),
          PopupMenuDivider(height: 1),
          PopupMenuItem<String>(
            value: 'sequential',
            child: Row(
              children: [
                Icon(
                  singleTorrent.sequential ? Icons.check_box_outlined : Icons.check_box_outline_blank,
                  color: singleTorrent.sequential ? colors.accentGreenColor : colors.tableSecondaryTextColor,
                  size: 18,
                ),
                const SizedBox(width: 8),
                Text('Sequential Download', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
              ],
            ),
          ),
          PopupMenuDivider(height: 1),
          PopupMenuItem<String>(
            value: 'remove',
            child: Row(
              children: [
                Icon(Icons.delete_outline_rounded, color: colors.accentRedColor, size: 18),
                const SizedBox(width: 8),
                Text('Remove...', style: TextStyle(color: colors.accentRedColor, fontSize: 13)),
              ],
            ),
          ),
        ],
      ],
    ).then((value) {
      if (value == null) return;
      switch (value) {
        case 'resume':
          onResumeTorrents?.call(targetTorrents);
          break;
        case 'pause':
          onPauseTorrents?.call(targetTorrents);
          break;
        case 'reannounce':
          onReannounceTorrents?.call(targetTorrents);
          break;
        case 'recheck':
          onRecheckTorrents?.call(targetTorrents);
          break;
        case 'seq_enable':
          onToggleSequentialTorrents?.call(targetTorrents, true);
          break;
        case 'seq_disable':
          onToggleSequentialTorrents?.call(targetTorrents, false);
          break;
        case 'sequential':
          onToggleSequentialTorrents?.call(targetTorrents, !singleTorrent.sequential);
          break;
        case 'remove':
          onRemoveTorrents?.call(targetTorrents);
          break;
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final double totalCalculatedWidth = columnKeys.fold(
      0.0,
      (sum, key) => sum + (columnWidths[key] ?? 100.0),
    );

    final colors = context.appColors;

    return ScrollbarTheme(
      data: ScrollbarThemeData(
        thumbColor: WidgetStateProperty.resolveWith((states) {
          if (states.contains(WidgetState.hovered) ||
              states.contains(WidgetState.dragged)) {
            return colors.accentGreenColor;
          }
          return colors.scrollbarThumbColor;
        }),
        trackColor: WidgetStateProperty.all(colors.scrollbarTrackColor),
        trackBorderColor: WidgetStateProperty.all(Colors.transparent),
        thickness: WidgetStateProperty.all(8.0),
        radius: const Radius.circular(4.0),
      ),
      child: Scrollbar(
        controller: verticalScrollController,
        thumbVisibility: true,
        trackVisibility: true,
        child: Scrollbar(
          controller: horizontalScrollController,
          thumbVisibility: true,
          trackVisibility: true,
          notificationPredicate: (notif) => notif.depth == 1,
          child: DataTable2(
            scrollController: verticalScrollController,
            horizontalScrollController: horizontalScrollController,
            columnSpacing: 0,
            horizontalMargin: 8,
            minWidth: totalCalculatedWidth + 50.0,
            dataRowHeight: 26,
            headingRowHeight: 28,
            headingRowColor: WidgetStateProperty.all(colors.tableHeaderBackgroundColor),
            dataRowColor: WidgetStateProperty.resolveWith<Color?>((states) {
              if (states.contains(WidgetState.selected)) {
                return colors.tableRowSelectedColor;
              }
              if (states.contains(WidgetState.hovered)) {
                return colors.tableRowHoverColor;
              }
              return Colors.transparent;
            }),
            showCheckboxColumn: false,
            isVerticalScrollBarVisible: false,
            isHorizontalScrollBarVisible: false,
            sortColumnIndex: null,
            sortAscending: isAscending,
            columns: List.generate(columnKeys.length, (index) {
              final key = columnKeys[index];
              return _buildDataColumn2(context, key, index);
            }),
            rows: torrents.map((torrent) {
              final isSelected = selectedInfoHashes.contains(torrent.infoHash);
              return DataRow2(
                selected: isSelected,
                onTap: () {
                  final isCtrl = HardwareKeyboard.instance.isControlPressed ||
                      HardwareKeyboard.instance.isMetaPressed;
                  final isShift = HardwareKeyboard.instance.isShiftPressed;
                  onSelectTorrent(
                    torrent,
                    isToggle: isCtrl,
                    isRange: isShift,
                  );
                },
                onSecondaryTapDown: (details) {
                  final List<TorrentStructure> targetTorrents;
                  if (selectedInfoHashes.contains(torrent.infoHash)) {
                    targetTorrents = torrents
                        .where((t) => selectedInfoHashes.contains(t.infoHash))
                        .toList();
                  } else {
                    onSelectTorrent(torrent, isToggle: false, isRange: false);
                    targetTorrents = [torrent];
                  }
                  _showContextMenu(context, details.globalPosition, targetTorrents);
                },
                cells: columnKeys
                    .map((key) => _buildDataCell(context, torrent, key))
                    .toList(),
              );
            }).toList(),
          ),
        ),
      ),
    );
  }

  DataColumn2 _buildDataColumn2(BuildContext context, String columnKey, int colIndex) {
    final width = columnWidths[columnKey] ?? 100.0;

    return DataColumn2(
      fixedWidth: width,
      numeric: false,
      label: _buildHeaderContent(context, columnKey, colIndex),
      onSort: (_, __) => onSort(columnKey),
    );
  }

  Widget _buildHeaderContent(BuildContext context, String columnKey, int colIndex) {
    final colors = context.appColors;
    final bool isSorted = sortColumnKey == columnKey;
    final bool isNameCol = columnKey == 'Name';

    return Row(
      children: [
        Expanded(
          child: DragTarget<int>(
            onAcceptWithDetails: (details) =>
                onColumnReordered(details.data, colIndex),
            builder: (context, candidateData, rejectedData) {
              return LongPressDraggable<int>(
                data: colIndex,
                axis: Axis.horizontal,
                delay: const Duration(milliseconds: 200),
                feedback: Material(
                  color: Colors.transparent,
                  child: Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 6,
                    ),
                    decoration: BoxDecoration(
                      color: colors.dataCardBackgroundColor,
                      borderRadius: BorderRadius.circular(4),
                      boxShadow: const [
                        BoxShadow(
                          color: Colors.black45,
                          blurRadius: 4,
                          offset: Offset(0, 2),
                        ),
                      ],
                    ),
                    child: Text(
                      columnKey,
                      style: TextStyle(
                        color: colors.accentGreenColor,
                        fontSize: 12,
                        decoration: TextDecoration.none,
                      ),
                    ),
                  ),
                ),
                child: InkWell(
                  onTap: () => onSort(columnKey),
                  hoverColor: colors.tableRowHoverColor,
                  borderRadius: BorderRadius.circular(2),
                  child: Container(
                    alignment: isNameCol
                        ? Alignment.centerLeft
                        : Alignment.center,
                    padding:
                        const EdgeInsets.symmetric(vertical: 4, horizontal: 2),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      mainAxisAlignment: isNameCol
                          ? MainAxisAlignment.start
                          : MainAxisAlignment.center,
                      children: [
                        Flexible(
                          child: Text(
                            columnKey,
                            style: TextStyle(
                              color: isSorted ? colors.tablePrimaryTextColor : colors.tableHeaderTextColor,
                              fontSize: 12,
                              fontWeight: isSorted ? FontWeight.bold : FontWeight.normal,
                            ),
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        if (isSorted) ...[
                          const SizedBox(width: 4),
                          Icon(
                            isAscending
                                ? Icons.arrow_upward
                                : Icons.arrow_downward,
                            size: 13,
                            color: colors.accentGreenColor,
                          ),
                        ],
                      ],
                    ),
                  ),
                ),
              );
            },
          ),
        ),
        MouseRegion(
          cursor: SystemMouseCursors.resizeColumn,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onHorizontalDragUpdate: (details) {
              final currentWidth = columnWidths[columnKey] ?? 100.0;
              final newWidth = (currentWidth + details.delta.dx).clamp(
                50.0,
                500.0,
              );
              onColumnResized(columnKey, newWidth);
            },
            child: Container(
              width: 8,
              height: double.infinity,
              alignment: Alignment.center,
              child: Container(width: 1, height: 16, color: colors.tableDividerColor),
            ),
          ),
        ),
      ],
    );
  }

  DataCell _buildDataCell(
    BuildContext context,
    TorrentStructure torrent,
    String columnKey,
  ) {
    final colors = context.appColors;
    final cellTextStyle = TextStyle(
      color: colors.tableAccentTextColor,
      fontSize: 12,
    );

    Widget child;
    switch (columnKey) {
      case 'Name':
        child = Row(
          children: [
            if (torrent.sequential) ...[
              Container(
                margin: const EdgeInsets.only(right: 6),
                padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
                decoration: BoxDecoration(
                  color: colors.accentBlueColor.withOpacity(0.18),
                  borderRadius: BorderRadius.circular(3),
                  border: Border.all(
                    color: colors.accentBlueColor.withOpacity(0.5),
                    width: 0.5,
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
            Expanded(
              child: Text(
                torrent.name,
                style: cellTextStyle,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        );
        break;
      case 'Category':
        child = Text(
          torrent.category.isNotEmpty ? torrent.category : '—',
          style: TextStyle(
            color: torrent.category.isNotEmpty ? colors.tablePrimaryTextColor : colors.tableSecondaryTextColor,
            fontSize: 11,
          ),
          overflow: TextOverflow.ellipsis,
        );
        break;
      case 'Size':
        child = Text(formatBytesDynamic(torrent.size), style: cellTextStyle);
        break;
      case 'Progress':
        child = Row(
          children: [
            Expanded(
              child: ClipRRect(
                borderRadius: BorderRadius.circular(2.0),
                child: LinearProgressIndicator(
                  value: torrent.progressValue,
                  backgroundColor: colors.tableProgressBarTrackColor,
                  color: colors.tableProgressBarProgressColor,
                  minHeight: 8,
                ),
              ),
            ),
            const SizedBox(width: 6),
            Text(torrent.progressPercentage, style: cellTextStyle),
          ],
        );
        break;
      case 'Status':
        child = Text(torrent.status, style: cellTextStyle);
        break;
      case 'Peers':
        child = Text(torrent.peers, style: cellTextStyle);
        break;
      case 'Down Speed':
        child = Text(
          formatBytesDynamic(torrent.downloadSpeed, isSpeed: true),
          style: cellTextStyle,
        );
        break;
      case 'Up Speed':
        child = Text(
          formatBytesDynamic(torrent.uploadSpeed, isSpeed: true),
          style: cellTextStyle,
        );
        break;
      case 'ETA':
        child = Text(torrent.etaString, style: cellTextStyle);
        break;
      case 'Ratio':
        child = Text(torrent.ratioString, style: cellTextStyle);
        break;
      case 'Downloaded':
        child = Text(
          formatBytesDynamic(torrent.downloaded),
          style: cellTextStyle,
        );
        break;
      case 'Uploaded':
        child = Text(
          formatBytesDynamic(torrent.uploaded),
          style: cellTextStyle,
        );
        break;
      case 'Total Pieces':
        child = Text(torrent.totalPieces, style: cellTextStyle);
        break;
      case 'Completed Pieces':
        child = Text(torrent.completedPieces, style: cellTextStyle);
        break;
      case 'Info Hash':
        child = Text(
          torrent.infoHash,
          style: cellTextStyle,
          overflow: TextOverflow.ellipsis,
        );
        break;
      default:
        child = Text('', style: cellTextStyle);
    }

    return DataCell(
      Align(
        alignment:
            columnKey == 'Name' ? Alignment.centerLeft : Alignment.center,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 4.0),
          child: child,
        ),
      ),
    );
  }
}
