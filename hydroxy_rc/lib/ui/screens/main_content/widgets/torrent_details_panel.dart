import 'package:flutter/material.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/tabs/torrent_files_tab.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/tabs/torrent_general_tab.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/tabs/torrent_peers_tab.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/tabs/torrent_placeholder_tab.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/tabs/torrent_trackers_tab.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/torrent_piece_map.dart';

class TorrentDetailsPanel extends StatelessWidget {
  final TorrentStructure? selectedTorrent;
  final bool isExpanded;
  final int activeTabIndex;
  final double contentHeight;
  final double minHeight;
  final double maxHeight;
  final double tabBarHeight;
  final double borderThickness;
  final VoidCallback onToggleExpand;
  final ValueChanged<int> onTabChanged;
  final ValueChanged<double> onHeightChanged;

  const TorrentDetailsPanel({
    super.key,
    required this.selectedTorrent,
    required this.isExpanded,
    required this.activeTabIndex,
    required this.contentHeight,
    this.minHeight = 100.0,
    this.maxHeight = 500.0,
    this.tabBarHeight = 32.0,
    this.borderThickness = 1.0,
    required this.onToggleExpand,
    required this.onTabChanged,
    required this.onHeightChanged,
  });

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;

    return Container(
      decoration: BoxDecoration(
        color: colors.detailsPanelBackgroundColor,
        border: Border(
          top: BorderSide(
            color: colors.detailsPanelBorderColor,
            width: borderThickness,
          ),
        ),
      ),
      child: Column(
        children: [
          // Full-width Vertical Drag Resize Handle Bar
          if (isExpanded)
            MouseRegion(
              cursor: SystemMouseCursors.resizeRow,
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onVerticalDragUpdate: (details) {
                  final newHeight =
                      (contentHeight - details.delta.dy).clamp(
                        minHeight,
                        maxHeight,
                      );
                  onHeightChanged(newHeight);
                },
                child: Container(
                  height: 5,
                  width: double.infinity,
                  color: Colors.transparent,
                  alignment: Alignment.center,
                  child: Container(
                    width: 48,
                    height: 2,
                    decoration: BoxDecoration(
                      color: colors.detailsPanelResizeHandleColor,
                      borderRadius: BorderRadius.circular(1),
                    ),
                  ),
                ),
              ),
            ),

          // Fixed Tab Bar Header
          SizedBox(
            height: tabBarHeight,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                _buildTabButton(context, 0, Icons.info_outline, 'General'),
                _buildTabButton(context, 1, Icons.location_on_outlined, 'Trackers'),
                _buildTabButton(context, 2, Icons.people_outline, 'Peers'),
                _buildTabButton(context, 3, Icons.storage_outlined, 'HTTP Sources'),
                _buildTabButton(context, 4, Icons.folder_open, 'Content'),
                const Spacer(),
                IconButton(
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints(),
                  iconSize: 18,
                  icon: Icon(
                    isExpanded
                        ? Icons.keyboard_arrow_down
                        : Icons.keyboard_arrow_up,
                    color: colors.tabBarInactiveTextColor,
                  ),
                  onPressed: onToggleExpand,
                ),
                const SizedBox(width: 8),
              ],
            ),
          ),

          // Horizontal Piece Map Bar across all tabs
          if (isExpanded && selectedTorrent != null)
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
              color: colors.pieceMapBackgroundColor,
              child: TorrentPieceMap(
                infoHash: selectedTorrent!.infoHash,
                totalPieces: int.tryParse(selectedTorrent!.totalPieces) ?? 0,
                completedPieces: int.tryParse(selectedTorrent!.completedPieces) ?? 0,
                height: 8,
                showStats: false,
              ),
            ),

          // Expanded Content Area
          if (isExpanded)
            Expanded(
              child: Container(
                padding: const EdgeInsets.all(12),
                color: colors.detailsContentBackgroundColor,
                child: selectedTorrent == null
                    ? Center(
                        child: Text(
                          'Select a torrent to view details',
                          style: TextStyle(
                            color: colors.detailsEmptyTextColor,
                            fontSize: 12,
                          ),
                        ),
                      )
                    : _buildTabContent(selectedTorrent!),
              ),
            ),
        ],
      ),
    );
  }

  Widget _buildTabButton(BuildContext context, int index, IconData icon, String label) {
    final colors = context.appColors;
    final isActive = activeTabIndex == index && isExpanded;
    return InkWell(
      onTap: () {
        if (activeTabIndex == index && isExpanded) {
          onToggleExpand();
        } else {
          onTabChanged(index);
          if (!isExpanded) {
            onToggleExpand();
          }
        }
      },
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10),
        decoration: BoxDecoration(
          color: isActive ? colors.detailsContentBackgroundColor : Colors.transparent,
          border: Border(
            bottom: BorderSide(
              color: isActive ? colors.tabBarActiveIndicatorColor : Colors.transparent,
              width: 2,
            ),
          ),
        ),
        child: Row(
          children: [
            Icon(
              icon,
              size: 14,
              color: isActive ? colors.tabBarActiveIconColor : colors.tabBarInactiveIconColor,
            ),
            const SizedBox(width: 4),
            Text(
              label,
              style: TextStyle(
                color: isActive ? colors.tabBarActiveTextColor : colors.tabBarInactiveTextColor,
                fontSize: 11,
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildTabContent(TorrentStructure torrent) {
    switch (activeTabIndex) {
      case 0:
        return TorrentGeneralTab(torrent: torrent);
      case 1:
        return TorrentTrackersTab(
          infoHash: torrent.infoHash,
          torrentName: torrent.name,
        );
      case 2:
        return TorrentPeersTab(
          infoHash: torrent.infoHash,
          torrentName: torrent.name,
        );
      case 3:
        return TorrentPlaceholderTab(
          title: 'HTTP Sources',
          icon: Icons.storage_outlined,
          torrent: torrent,
          mockHeaders: const ['URL', 'Status', 'Speed'],
        );
      case 4:
        return TorrentFilesTab(
          infoHash: torrent.infoHash,
          torrentName: torrent.name,
        );
      default:
        return TorrentGeneralTab(torrent: torrent);
    }
  }
}
