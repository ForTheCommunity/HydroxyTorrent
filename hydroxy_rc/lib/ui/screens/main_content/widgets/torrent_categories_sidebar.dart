import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';
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
  final Set<String> _collapsedStorages = {};
  List<StorageTargetModel> _storages = [];
  Timer? _storagesTimer;
  bool _statusExpanded = true;
  bool _storagesExpanded = true;

  @override
  void initState() {
    super.initState();
    _loadStorages();
    _storagesTimer = Timer.periodic(
      const Duration(seconds: 4),
      (_) => _loadStorages(),
    );
  }

  @override
  void dispose() {
    _storagesTimer?.cancel();
    super.dispose();
  }

  Future<void> _loadStorages() async {
    if (!ApiClient.instance.isAuthenticated) return;
    try {
      final items = await getStorages();
      if (mounted) {
        setState(() {
          _storages = items;
        });
      }
    } catch (_) {}
  }

  // Build hierarchical category tree for a specific storage target
  Map<String, CategoryNode> _buildCategoryTreeForStorage(
    String storageName,
    List<String> knownCategories,
    List<TorrentStructure> storageTorrents,
  ) {
    final rootNodes = <String, CategoryNode>{};

    void ensurePath(String catPath) {
      final segments = catPath
          .split('/')
          .map((s) => s.trim())
          .where((s) => s.isNotEmpty)
          .toList();
      if (segments.isEmpty) return;

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
        currentLevel = currentLevel[segment]!.children;
      }
    }

    // 1. Ensure all discovered categories exist in tree (including empty folders like anime)
    for (final cat in knownCategories) {
      ensurePath(cat);
    }

    // 2. Also ensure any categories from active torrents on this storage exist
    for (final t in storageTorrents) {
      final cat = t.category.trim();
      if (cat.isNotEmpty) {
        ensurePath(cat);
      }
    }

    // 3. Count matching torrents for each category node
    for (final t in storageTorrents) {
      final rawCat = t.category.trim();
      if (rawCat.isEmpty) continue;

      final segments = rawCat
          .split('/')
          .map((s) => s.trim())
          .where((s) => s.isNotEmpty)
          .toList();
      if (segments.isEmpty) continue;

      Map<String, CategoryNode> currentLevel = rootNodes;
      for (final segment in segments) {
        if (currentLevel.containsKey(segment)) {
          currentLevel[segment]!.count += 1;
          currentLevel = currentLevel[segment]!.children;
        } else {
          break;
        }
      }
    }

    return rootNodes;
  }

  Map<String, int> _countStatuses(List<TorrentStructure> torrents) {
    int downloading = 0;
    int stalled = 0;
    int seeding = 0;
    int completed = 0;
    int paused = 0;
    int checking = 0;
    int queued = 0;

    for (final t in torrents) {
      final st = t.status.toLowerCase();
      if (st == 'downloading') {
        downloading++;
      } else if (st == 'stalled') {
        stalled++;
      } else if (st == 'seeding') {
        seeding++;
        completed++;
      } else if (st == 'completed') {
        completed++;
      } else if (st == 'paused' || st == 'stopped') {
        paused++;
      } else if (st == 'checking') {
        checking++;
      } else if (st == 'queued') {
        queued++;
      }
    }

    return {
      'downloading': downloading,
      'stalled': stalled,
      'seeding': seeding,
      'completed': completed,
      'paused': paused,
      'checking': checking,
      'queued': queued,
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
        final statusCounts = _countStatuses(torrents);

        // Collect all distinct storage names
        final storageNames = <String>{
          ..._storages.map((s) => s.name),
          ...torrents.map((t) => t.storage.isEmpty ? 'default' : t.storage),
        };
        if (storageNames.isEmpty) {
          storageNames.add('default');
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
                        label: 'Stalled',
                        icon: Icons.pause_presentation_rounded,
                        iconColor: colors.accentAmberColor,
                        count: statusCounts['stalled'] ?? 0,
                        isSelected: widget.selectedFilter == 'status:stalled',
                        onTap: () => widget.onFilterSelected('status:stalled'),
                        depth: 1,
                      ),
                      _buildItem(
                        label: 'Queued',
                        icon: Icons.hourglass_top_rounded,
                        iconColor: colors.accentAmberColor,
                        count: statusCounts['queued'] ?? 0,
                        isSelected: widget.selectedFilter == 'status:queued',
                        onTap: () => widget.onFilterSelected('status:queued'),
                        depth: 1,
                      ),
                      _buildItem(
                        label: 'Seeding',
                        icon: Icons.arrow_upward_rounded,
                        iconColor: colors.uploadSpeedColor,
                        count: statusCounts['seeding'] ?? 0,
                        isSelected: widget.selectedFilter == 'status:seeding',
                        onTap: () => widget.onFilterSelected('status:seeding'),
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

                    // Storages and Categories section header
                    _buildSectionHeader(
                      title: 'STORAGE DISKS',
                      isExpanded: _storagesExpanded,
                      onToggle: () => setState(
                        () => _storagesExpanded = !_storagesExpanded,
                      ),
                    ),

                    if (_storagesExpanded) ...[
                      for (final storageName in storageNames)
                        _buildStorageItem(
                          storageName: storageName,
                          storageTorrents: torrents
                              .where(
                                (t) =>
                                    (t.storage.isEmpty ? 'default' : t.storage) ==
                                    storageName,
                              )
                              .toList(),
                          knownCategories: _storages
                              .firstWhere(
                                (s) => s.name == storageName,
                                orElse: () => StorageTargetModel(
                                  name: storageName,
                                  watchDirectory: '',
                                  downloadDirectory: '',
                                ),
                              )
                              .categories,
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

  Widget _buildStorageItem({
    required String storageName,
    required List<TorrentStructure> storageTorrents,
    required List<String> knownCategories,
  }) {
    final colors = context.appColors;
    final isStorageExpanded = !_collapsedStorages.contains(storageName);
    final filterKey = 'storage:$storageName';
    final isSelected = widget.selectedFilter == filterKey;
    final categoryTree = _buildCategoryTreeForStorage(
      storageName,
      knownCategories,
      storageTorrents,
    );
    final uncategorizedCount =
        storageTorrents.where((t) => t.category.trim().isEmpty).length;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        // Storage Disk Header Item
        Container(
          height: 28,
          padding: const EdgeInsets.only(left: 10, right: 8),
          decoration: BoxDecoration(
            color: isSelected
                ? colors.sidebarSelectedItemBackgroundColor
                : Colors.transparent,
            border: Border(
              left: BorderSide(
                color: isSelected
                    ? colors.sidebarSelectedIconColor
                    : Colors.transparent,
                width: 2.5,
              ),
            ),
          ),
          child: Row(
            children: [
              GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTap: () {
                  setState(() {
                    if (isStorageExpanded) {
                      _collapsedStorages.add(storageName);
                    } else {
                      _collapsedStorages.remove(storageName);
                    }
                  });
                },
                child: Container(
                  width: 20,
                  height: 24,
                  alignment: Alignment.center,
                  child: Icon(
                    isStorageExpanded
                        ? Icons.keyboard_arrow_down
                        : Icons.keyboard_arrow_right,
                    size: 14,
                    color: colors.sidebarIconColor,
                  ),
                ),
              ),
              const SizedBox(width: 2),
              Expanded(
                child: InkWell(
                  onTap: () => widget.onFilterSelected(filterKey),
                  borderRadius: BorderRadius.circular(3),
                  child: Row(
                    children: [
                      Icon(
                        Icons.dns_rounded,
                        size: 14,
                        color: isSelected
                            ? colors.sidebarSelectedIconColor
                            : colors.accentPurpleColor,
                      ),
                      const SizedBox(width: 6),
                      Expanded(
                        child: Text(
                          storageName,
                          style: TextStyle(
                            color: isSelected
                                ? colors.sidebarSelectedItemTextColor
                                : colors.sidebarItemTextColor,
                            fontSize: 12,
                            fontWeight: FontWeight.bold,
                          ),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      _buildCountBadge(storageTorrents.length, isSelected),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),

        // Nested Categories under this storage
        if (isStorageExpanded) ...[
          if (categoryTree.isEmpty && uncategorizedCount == 0)
            Padding(
              padding: const EdgeInsets.only(left: 36, top: 4, bottom: 4),
              child: Text(
                'No categories in this storage',
                style: TextStyle(
                  color: colors.sidebarItemTextColor.withOpacity(0.4),
                  fontSize: 10,
                  fontStyle: FontStyle.italic,
                ),
              ),
            ),
          for (final node in categoryTree.values)
            ..._renderStorageCategoryNode(storageName, node, depth: 2),
          if (uncategorizedCount > 0)
            _buildItem(
              label: 'Uncategorized',
              icon: Icons.folder_off_outlined,
              iconColor: colors.sidebarItemTextColor.withOpacity(0.5),
              count: uncategorizedCount,
              isSelected:
                  widget.selectedFilter == 'storage:$storageName/category:',
              onTap: () =>
                  widget.onFilterSelected('storage:$storageName/category:'),
              depth: 2,
            ),
        ],
      ],
    );
  }

  List<Widget> _renderStorageCategoryNode(
    String storageName,
    CategoryNode node, {
    required int depth,
  }) {
    final widgets = <Widget>[];
    final hasChildren = node.children.isNotEmpty;
    final nodeKey = '$storageName:${node.fullPath}';
    final isExpanded = _expandedPaths.contains(nodeKey);
    final filterKey = 'storage:$storageName/category:${node.fullPath}';
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
              _expandedPaths.remove(nodeKey);
            } else {
              _expandedPaths.add(nodeKey);
            }
          });
        },
        onSelect: () => widget.onFilterSelected(filterKey),
      ),
    );

    if (hasChildren && isExpanded) {
      for (final child in node.children.values) {
        widgets.addAll(
          _renderStorageCategoryNode(storageName, child, depth: depth + 1),
        );
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
        color: isSelected
            ? colors.sidebarSelectedItemBackgroundColor
            : Colors.transparent,
        border: Border(
          left: BorderSide(
            color: isSelected
                ? colors.sidebarSelectedIconColor
                : Colors.transparent,
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
                        color: isSelected
                            ? colors.sidebarSelectedItemTextColor
                            : colors.sidebarItemTextColor,
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
          color: isSelected
              ? colors.sidebarSelectedItemBackgroundColor
              : Colors.transparent,
          border: Border(
            left: BorderSide(
              color: isSelected
                  ? colors.sidebarSelectedIconColor
                  : Colors.transparent,
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
                  color: isSelected
                      ? colors.sidebarSelectedItemTextColor
                      : colors.sidebarItemTextColor,
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
          color: isSelected
              ? colors.sidebarSelectedBadgeTextColor
              : colors.sidebarBadgeTextColor,
          fontSize: 9,
          fontWeight: FontWeight.bold,
        ),
      ),
    );
  }
}
