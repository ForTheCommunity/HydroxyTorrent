import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/state/torrents_bloc/torrents_bloc.dart';

class CategoryNode {
  final String name;
  final String fullPath;
  int count;
  final Map<String, CategoryNode> children = {};

  CategoryNode({required this.name, required this.fullPath, this.count = 0});
}

class TorrentCategoriesSidebar extends StatefulWidget {
  final String? selectedFilter;
  final ValueChanged<String?> onFilterSelected;

  const TorrentCategoriesSidebar({
    super.key,
    required this.selectedFilter,
    required this.onFilterSelected,
  });

  @override
  State<TorrentCategoriesSidebar> createState() =>
      _TorrentCategoriesSidebarState();
}

class _TorrentCategoriesSidebarState extends State<TorrentCategoriesSidebar> {
  final Set<String> _expandedPaths = {};
  bool _statusExpanded = true;
  bool _categoriesExpanded = true;
  bool _hasAutoExpanded = false;

  @override
  void initState() {
    super.initState();
  }

  // Build hierarchical category tree from a list of torrents
  Map<String, CategoryNode> _buildCategoryTree(
    List<TorrentStructure> torrents,
  ) {
    final rootNodes = <String, CategoryNode>{};

    for (final torrent in torrents) {
      final rawCat = torrent.category.trim();
      if (rawCat.isEmpty) continue;

      final segments = rawCat
          .split('/')
          .map((s) => s.trim())
          .where((s) => s.isNotEmpty)
          .toList();
      if (segments.isEmpty) continue;

      Map<String, CategoryNode> currentLevel = rootNodes;
      String currentPath = '';

      for (int i = 0; i < segments.length; i++) {
        final segment = segments[i];
        currentPath = currentPath.isEmpty ? segment : '$currentPath/$segment';

        if (!currentLevel.containsKey(segment)) {
          currentLevel[segment] = CategoryNode(
            name: segment,
            fullPath: currentPath,
            count: 0,
          );
        }

        // Each node in the branch counts the torrent
        currentLevel[segment]!.count += 1;
        currentLevel = currentLevel[segment]!.children;
      }
    }

    return rootNodes;
  }

  int _countUncategorized(List<TorrentStructure> torrents) {
    return torrents.where((t) => t.category.trim().isEmpty).length;
  }

  Map<String, int> _countStatuses(List<TorrentStructure> torrents) {
    int downloading = 0;
    int seeding = 0;
    int completed = 0;
    int paused = 0;
    int checking = 0;

    for (final t in torrents) {
      final st = t.status.toLowerCase();
      if (st == 'downloading') {
        downloading++;
      } else if (st == 'seeding') {
        seeding++;
      } else if (st == 'completed') {
        completed++;
      } else if (st == 'paused' || st == 'stopped') {
        paused++;
      } else if (st == 'checking') {
        checking++;
      }
    }

    return {
      'downloading': downloading,
      'seeding': seeding,
      'completed': completed,
      'paused': paused,
      'checking': checking,
    };
  }

  @override
  Widget build(BuildContext context) {
    return BlocBuilder<TorrentsBloc, TorrentsState>(
      builder: (context, state) {
        final torrents = state.maybeWhen(
          loaded: (list) => list,
          orElse: () => const <TorrentStructure>[],
        );

        final totalTorrents = torrents.length;
        final uncategorizedCount = _countUncategorized(torrents);
        final statusCounts = _countStatuses(torrents);
        final categoryTree = _buildCategoryTree(torrents);

        // Auto-expand root category nodes on first load only
        if (!_hasAutoExpanded && categoryTree.isNotEmpty) {
          _expandedPaths.addAll(categoryTree.keys);
          _hasAutoExpanded = true;
        }

        final colors = context.appColors;

        return Container(
          decoration: BoxDecoration(
            color: colors.sidebarBackgroundColor,
            border: Border(right: BorderSide(color: colors.sidebarBorderColor)),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              // Header title
              Container(
                height: 38,
                padding: const EdgeInsets.symmetric(horizontal: 12),
                alignment: Alignment.centerLeft,
                decoration: BoxDecoration(
                  color: colors.titleBarBackgroundColor,
                  border: Border(bottom: BorderSide(color: colors.sidebarBorderColor)),
                ),
                child: Row(
                  children: [
                    Icon(
                      Icons.folder_special_outlined,
                      size: 16,
                      color: colors.sidebarSelectedIconColor,
                    ),
                    const SizedBox(width: 8),
                    Text(
                      'CATEGORIES',
                      style: TextStyle(
                        color: colors.sidebarHeaderTextColor,
                        fontSize: 11,
                        fontWeight: FontWeight.bold,
                        letterSpacing: 0.8,
                      ),
                    ),
                  ],
                ),
              ),

              // Scrollable navigation list
              Expanded(
                child: ListView(
                  padding: const EdgeInsets.symmetric(vertical: 6),
                  children: [
                    // "All Torrents" filter
                    _buildItem(
                      label: 'All Torrents',
                      icon: Icons.layers_outlined,
                      count: totalTorrents,
                      isSelected:
                          widget.selectedFilter == null ||
                          widget.selectedFilter == 'all',
                      onTap: () => widget.onFilterSelected(null),
                      depth: 0,
                    ),

                    Divider(
                      color: colors.dividerColor,
                      height: 12,
                      indent: 8,
                      endIndent: 8,
                    ),

                    // Status section header
                    _buildSectionHeader(
                      title: 'STATUS',
                      isExpanded: _statusExpanded,
                      onToggle: () =>
                          setState(() => _statusExpanded = !_statusExpanded),
                    ),
                    if (_statusExpanded) ...[
                      _buildItem(
                        label: 'Downloading',
                        icon: Icons.arrow_downward_rounded,
                        iconColor: colors.downloadSpeedColor,
                        count: statusCounts['downloading'] ?? 0,
                        isSelected:
                            widget.selectedFilter == 'status:downloading',
                        onTap: () =>
                            widget.onFilterSelected('status:downloading'),
                        depth: 1,
                      ),
                      _buildItem(
                        label: 'Completed',
                        icon: Icons.check_circle_outline,
                        iconColor: colors.accentBlueColor,
                        count: statusCounts['completed'] ?? 0,
                        isSelected: widget.selectedFilter == 'status:completed',
                        onTap: () =>
                            widget.onFilterSelected('status:completed'),
                        depth: 1,
                      ),
                      _buildItem(
                        label: 'Paused',
                        icon: Icons.pause_circle_outline,
                        iconColor: colors.accentAmberColor,
                        count: statusCounts['paused'] ?? 0,
                        isSelected: widget.selectedFilter == 'status:paused',
                        onTap: () => widget.onFilterSelected('status:paused'),
                        depth: 1,
                      ),
                      if ((statusCounts['checking'] ?? 0) > 0)
                        _buildItem(
                          label: 'Checking',
                          icon: Icons.sync_rounded,
                          iconColor: colors.accentAmberColor,
                          count: statusCounts['checking'] ?? 0,
                          isSelected:
                              widget.selectedFilter == 'status:checking',
                          onTap: () =>
                              widget.onFilterSelected('status:checking'),
                          depth: 1,
                        ),
                    ],

                    Divider(
                      color: colors.dividerColor,
                      height: 12,
                      indent: 8,
                      endIndent: 8,
                    ),

                    // Categories section header
                    _buildSectionHeader(
                      title: 'WATCH CATEGORIES',
                      isExpanded: _categoriesExpanded,
                      onToggle: () => setState(
                        () => _categoriesExpanded = !_categoriesExpanded,
                      ),
                    ),

                    if (_categoriesExpanded) ...[
                      if (categoryTree.isEmpty && uncategorizedCount == 0)
                        const Padding(
                          padding: EdgeInsets.symmetric(
                            horizontal: 16,
                            vertical: 8,
                          ),
                          child: Text(
                            'No categories found in watch directory',
                            style: TextStyle(
                              color: Colors.white24,
                              fontSize: 10,
                              fontStyle: FontStyle.italic,
                            ),
                          ),
                        ),

                      // Render recursive category tree nodes
                      for (final node in categoryTree.values)
                        ..._renderCategoryNode(node, depth: 1),

                      // Uncategorized items
                      if (uncategorizedCount > 0)
                        _buildItem(
                          label: 'Uncategorized',
                          icon: Icons.folder_off_outlined,
                          iconColor: Colors.white38,
                          count: uncategorizedCount,
                          isSelected: widget.selectedFilter == 'category:',
                          onTap: () => widget.onFilterSelected('category:'),
                          depth: 1,
                        ),
                    ],
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _buildSectionHeader({
    required String title,
    required bool isExpanded,
    required VoidCallback onToggle,
  }) {
    final colors = context.appColors;
    return InkWell(
      onTap: onToggle,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
        child: Row(
          children: [
            Icon(
              isExpanded
                  ? Icons.keyboard_arrow_down
                  : Icons.keyboard_arrow_right,
              size: 14,
              color: colors.sidebarIconColor,
            ),
            const SizedBox(width: 4),
            Text(
              title,
              style: TextStyle(
                color: colors.sidebarHeaderTextColor,
                fontSize: 10,
                fontWeight: FontWeight.bold,
                letterSpacing: 0.5,
              ),
            ),
          ],
        ),
      ),
    );
  }

  List<Widget> _renderCategoryNode(CategoryNode node, {required int depth}) {
    final widgets = <Widget>[];
    final hasChildren = node.children.isNotEmpty;
    final isExpanded = _expandedPaths.contains(node.fullPath);
    final filterKey = 'category:${node.fullPath}';
    final isSelected = widget.selectedFilter == filterKey;

    widgets.add(
      _buildCategoryItem(
        label: node.name,
        count: node.count,
        hasChildren: hasChildren,
        isExpanded: isExpanded,
        isSelected: isSelected,
        depth: depth,
        onToggleExpand: () {
          setState(() {
            if (isExpanded) {
              _expandedPaths.remove(node.fullPath);
            } else {
              _expandedPaths.add(node.fullPath);
            }
          });
        },
        onSelect: () => widget.onFilterSelected(filterKey),
      ),
    );

    if (hasChildren && isExpanded) {
      for (final child in node.children.values) {
        widgets.addAll(_renderCategoryNode(child, depth: depth + 1));
      }
    }

    return widgets;
  }

  Widget _buildCategoryItem({
    required String label,
    required int count,
    required bool hasChildren,
    required bool isExpanded,
    required bool isSelected,
    required int depth,
    required VoidCallback onToggleExpand,
    required VoidCallback onSelect,
  }) {
    final colors = context.appColors;
    final indent = (depth * 14.0).clamp(6.0, 60.0);

    return Container(
      height: 26,
      padding: EdgeInsets.only(left: indent, right: 8),
      decoration: BoxDecoration(
        color: isSelected ? colors.sidebarSelectedItemBackgroundColor : Colors.transparent,
        border: Border(
          left: BorderSide(
            color: isSelected ? colors.sidebarSelectedIconColor : Colors.transparent,
            width: 2.5,
          ),
        ),
      ),
      child: Row(
        children: [
          if (hasChildren)
            GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: onToggleExpand,
              child: Container(
                width: 18,
                height: 24,
                alignment: Alignment.center,
                child: Icon(
                  isExpanded
                      ? Icons.keyboard_arrow_down
                      : Icons.keyboard_arrow_right,
                  size: 14,
                  color: colors.sidebarIconColor,
                ),
              ),
            )
          else
            const SizedBox(width: 18),
          const SizedBox(width: 2),
          Expanded(
            child: InkWell(
              onTap: onSelect,
              borderRadius: BorderRadius.circular(3),
              child: Row(
                children: [
                  Icon(
                    hasChildren
                        ? (isExpanded
                              ? Icons.folder_open_rounded
                              : Icons.folder_rounded)
                        : Icons.folder_outlined,
                    size: 14,
                    color: isSelected
                        ? colors.sidebarSelectedIconColor
                        : (hasChildren
                              ? colors.accentAmberColor
                              : colors.sidebarIconColor),
                  ),
                  const SizedBox(width: 6),
                  Expanded(
                    child: Text(
                      label,
                      style: TextStyle(
                        color: isSelected ? colors.sidebarSelectedItemTextColor : colors.sidebarItemTextColor,
                        fontSize: 11,
                        fontWeight:
                            isSelected ? FontWeight.w600 : FontWeight.w400,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  _buildCountBadge(count, isSelected),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildItem({
    required String label,
    required IconData icon,
    Color? iconColor,
    required int count,
    required bool isSelected,
    required VoidCallback onTap,
    required int depth,
  }) {
    final colors = context.appColors;
    final indent = (depth * 14.0).clamp(10.0, 40.0);

    return InkWell(
      onTap: onTap,
      child: Container(
        height: 26,
        padding: EdgeInsets.only(left: indent, right: 10),
        decoration: BoxDecoration(
          color: isSelected ? colors.sidebarSelectedItemBackgroundColor : Colors.transparent,
          border: Border(
            left: BorderSide(
              color: isSelected ? colors.sidebarSelectedIconColor : Colors.transparent,
              width: 2.5,
            ),
          ),
        ),
        child: Row(
          children: [
            Icon(
              icon,
              size: 14,
              color: isSelected
                  ? colors.sidebarSelectedIconColor
                  : (iconColor ?? colors.sidebarIconColor),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                label,
                style: TextStyle(
                  color: isSelected ? colors.sidebarSelectedItemTextColor : colors.sidebarItemTextColor,
                  fontSize: 11,
                  fontWeight: isSelected ? FontWeight.w600 : FontWeight.w400,
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
            _buildCountBadge(count, isSelected),
          ],
        ),
      ),
    );
  }

  Widget _buildCountBadge(int count, bool isSelected) {
    final colors = context.appColors;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
      decoration: BoxDecoration(
        color: isSelected
            ? colors.sidebarSelectedBadgeBackgroundColor
            : colors.sidebarBadgeBackgroundColor,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Text(
        '$count',
        style: TextStyle(
          color: isSelected ? colors.sidebarSelectedBadgeTextColor : colors.sidebarBadgeTextColor,
          fontSize: 9,
          fontWeight: FontWeight.bold,
        ),
      ),
    );
  }
}
