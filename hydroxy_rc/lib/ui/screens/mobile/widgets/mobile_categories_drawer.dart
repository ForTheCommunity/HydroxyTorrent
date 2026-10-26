import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hydroxy_rc/configs/converter.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/state/themes_bloc/themes_bloc.dart';
import 'package:hydroxy_rc/state/torrents_bloc/torrents_bloc.dart';
import 'package:hydroxy_rc/themes/app_theme.dart';

class MobileCategoriesDrawer extends StatelessWidget {
  final String? selectedFilter;
  final ValueChanged<String?> onFilterSelected;
  final SystemStatsModel? systemStats;
  final VoidCallback onShowLogs;
  final VoidCallback onLogout;
  final VoidCallback onShutdown;

  const MobileCategoriesDrawer({
    super.key,
    required this.selectedFilter,
    required this.onFilterSelected,
    this.systemStats,
    required this.onShowLogs,
    required this.onLogout,
    required this.onShutdown,
  });

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;

    return Drawer(
      backgroundColor: colors.sidebarBackgroundColor,
      child: SafeArea(
        child: Column(
          children: [
            // Drawer Header
            _buildDrawerHeader(context),

            Divider(color: colors.dividerColor, height: 1),

            // Scrollable Filters & Categories List
            Expanded(
              child: BlocBuilder<TorrentsBloc, TorrentsState>(
                builder: (context, state) {
                  final torrents = state.maybeWhen(
                    loaded: (torrents) => torrents,
                    orElse: () => const <TorrentStructure>[],
                  );

                  return ListView(
                    padding: const EdgeInsets.symmetric(vertical: 8),
                    children: [
                      _buildSectionHeader(context, 'STATUS'),
                      _buildStatusItem(
                        context: context,
                        filterKey: 'all',
                        label: 'All Torrents',
                        icon: Icons.all_inbox_rounded,
                        count: torrents.length,
                        activeColor: colors.accentBlueColor,
                      ),
                      _buildStatusItem(
                        context: context,
                        filterKey: 'status:downloading',
                        label: 'Downloading',
                        icon: Icons.arrow_downward_rounded,
                        count: torrents
                            .where(
                              (t) => t.status.toLowerCase() == 'downloading',
                            )
                            .length,
                        activeColor: colors.accentGreenColor,
                      ),
                      _buildStatusItem(
                        context: context,
                        filterKey: 'status:completed',
                        label: 'Seeding',
                        icon: Icons.arrow_upward_rounded,
                        count: torrents
                            .where(
                              (t) =>
                                  t.status.toLowerCase() == 'completed' ||
                                  t.status.toLowerCase() == 'seeding',
                            )
                            .length,
                        activeColor: colors.accentPurpleColor,
                      ),
                      _buildStatusItem(
                        context: context,
                        filterKey: 'status:stalled',
                        label: 'Stalled',
                        icon: Icons.hourglass_empty_rounded,
                        count: torrents
                            .where((t) => t.status.toLowerCase() == 'stalled')
                            .length,
                        activeColor: colors.accentAmberColor,
                      ),
                      _buildStatusItem(
                        context: context,
                        filterKey: 'status:queued',
                        label: 'Queued',
                        icon: Icons.schedule_rounded,
                        count: torrents
                            .where((t) => t.status.toLowerCase() == 'queued')
                            .length,
                        activeColor: colors.tableAccentTextColor,
                      ),
                      _buildStatusItem(
                        context: context,
                        filterKey: 'status:paused',
                        label: 'Paused',
                        icon: Icons.pause_circle_outline_rounded,
                        count: torrents
                            .where(
                              (t) =>
                                  t.status.toLowerCase() == 'paused' ||
                                  t.status.toLowerCase() == 'stopped',
                            )
                            .length,
                        activeColor: colors.tableSecondaryTextColor,
                      ),

                      const SizedBox(height: 12),
                      _buildCategoriesSection(context, torrents),
                    ],
                  );
                },
              ),
            ),

            Divider(color: colors.dividerColor, height: 1),

            // Bottom Actions (Theme, Logs, Logout, Shutdown)
            _buildDrawerFooter(context),
          ],
        ),
      ),
    );
  }

  Widget _buildDrawerHeader(BuildContext context) {
    final colors = context.appColors;

    return Container(
      padding: const EdgeInsets.all(16),
      color: colors.titleBarBackgroundColor,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Container(
                padding: const EdgeInsets.all(8),
                decoration: BoxDecoration(
                  color: Color.fromRGBO(colors.accentBlueColor.red, colors.accentBlueColor.green, colors.accentBlueColor.blue, 0.15),
                  shape: BoxShape.circle,
                ),
                child: Icon(
                  Icons.image_not_supported_outlined,
                  color: colors.titleBarLogoColor,
                  size: 22,
                ),
              ),
              const SizedBox(width: 12),
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'HydroxyTorrent',
                    style: TextStyle(
                      color: colors.titleBarTextColor,
                      fontSize: 16,
                      fontWeight: FontWeight.bold,
                      letterSpacing: 0.3,
                    ),
                  ),
                  Text(
                    'Remote Client',
                    style: TextStyle(
                      color: Color.fromRGBO(colors.sidebarItemTextColor.red, colors.sidebarItemTextColor.green, colors.sidebarItemTextColor.blue, 0.7),
                      fontSize: 11,
                    ),
                  ),
                ],
              ),
            ],
          ),

          if (systemStats != null) ...[
            const SizedBox(height: 12),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
              decoration: BoxDecoration(
                color: colors.searchBarBackgroundColor,
                borderRadius: BorderRadius.circular(6),
                border: Border.all(color: colors.dividerColor, width: 0.8),
              ),
              child: Row(
                children: [
                  Icon(
                    Icons.memory_rounded,
                    size: 14,
                    color: colors.accentPurpleColor,
                  ),
                  const SizedBox(width: 6),
                  Text(
                    'Daemon: ${formatBytesDynamic(systemStats!.processRssBytes.toString())}',
                    style: TextStyle(
                      color: colors.sidebarItemTextColor,
                      fontSize: 11,
                      fontFamily: 'monospace',
                    ),
                  ),
                  const Spacer(),
                  Icon(
                    Icons.pie_chart_outline_rounded,
                    size: 14,
                    color: colors.accentBlueColor,
                  ),
                  const SizedBox(width: 6),
                  Text(
                    'Sys: ${(systemStats!.systemMemoryUsagePercent * 100).toStringAsFixed(0)}%',
                    style: TextStyle(
                      color: colors.sidebarItemTextColor,
                      fontSize: 11,
                      fontFamily: 'monospace',
                    ),
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildSectionHeader(BuildContext context, String title) {
    final colors = context.appColors;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Text(
        title,
        style: TextStyle(
          color: Color.fromRGBO(colors.sidebarItemTextColor.red, colors.sidebarItemTextColor.green, colors.sidebarItemTextColor.blue, 0.6),
          fontSize: 11,
          fontWeight: FontWeight.bold,
          letterSpacing: 0.8,
        ),
      ),
    );
  }

  Widget _buildStatusItem({
    required BuildContext context,
    required String filterKey,
    required String label,
    required IconData icon,
    required int count,
    required Color activeColor,
  }) {
    final colors = context.appColors;
    final isSelected =
        selectedFilter == filterKey ||
        (selectedFilter == null && filterKey == 'all');

    return InkWell(
      onTap: () {
        onFilterSelected(filterKey == 'all' ? null : filterKey);
        Navigator.of(context).pop();
      },
      child: Container(
        margin: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        decoration: BoxDecoration(
          color: isSelected
              ? colors.sidebarSelectedItemBackgroundColor
              : Colors.transparent,
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(
          children: [
            Icon(
              icon,
              size: 18,
              color: isSelected ? activeColor : colors.sidebarIconColor,
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Text(
                label,
                style: TextStyle(
                  color: isSelected
                      ? colors.sidebarSelectedItemTextColor
                      : colors.sidebarItemTextColor,
                  fontSize: 13,
                  fontWeight: isSelected ? FontWeight.bold : FontWeight.w500,
                ),
              ),
            ),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
              decoration: BoxDecoration(
                color: isSelected
                    ? Color.fromRGBO(activeColor.red, activeColor.green, activeColor.blue, 0.2)
                    : Color.fromRGBO(colors.dividerColor.red, colors.dividerColor.green, colors.dividerColor.blue, 0.3),
                borderRadius: BorderRadius.circular(10),
              ),
              child: Text(
                '$count',
                style: TextStyle(
                  color: isSelected
                      ? activeColor
                      : Color.fromRGBO(colors.sidebarItemTextColor.red, colors.sidebarItemTextColor.green, colors.sidebarItemTextColor.blue, 0.7),
                  fontSize: 11,
                  fontWeight: FontWeight.bold,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildCategoriesSection(
    BuildContext context,
    List<TorrentStructure> torrents,
  ) {
    final colors = context.appColors;

    // Build category map: category name -> count
    final categoryCounts = <String, int>{};
    int uncategorizedCount = 0;

    for (final t in torrents) {
      final cat = t.category.trim();
      if (cat.isEmpty) {
        uncategorizedCount++;
      } else {
        categoryCounts[cat] = (categoryCounts[cat] ?? 0) + 1;
      }
    }

    if (categoryCounts.isEmpty && uncategorizedCount == 0) {
      return const SizedBox.shrink();
    }

    final sortedCats = categoryCounts.keys.toList()..sort();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _buildSectionHeader(context, 'CATEGORIES'),
        if (uncategorizedCount > 0)
          _buildCategoryItem(
            context: context,
            category: '',
            displayLabel: 'Uncategorized',
            icon: Icons.folder_off_outlined,
            count: uncategorizedCount,
          ),
        ...sortedCats.map((cat) {
          return _buildCategoryItem(
            context: context,
            category: cat,
            displayLabel: cat,
            icon: Icons.folder_outlined,
            count: categoryCounts[cat] ?? 0,
          );
        }),
      ],
    );
  }

  Widget _buildCategoryItem({
    required BuildContext context,
    required String category,
    required String displayLabel,
    required IconData icon,
    required int count,
  }) {
    final colors = context.appColors;
    final filterKey = 'category:$category';
    final isSelected = selectedFilter == filterKey;

    return InkWell(
      onTap: () {
        onFilterSelected(filterKey);
        Navigator.of(context).pop();
      },
      child: Container(
        margin: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
        decoration: BoxDecoration(
          color: isSelected
              ? colors.sidebarSelectedItemBackgroundColor
              : Colors.transparent,
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(
          children: [
            Icon(
              icon,
              size: 16,
              color: isSelected
                  ? colors.accentAmberColor
                  : colors.sidebarIconColor,
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Text(
                displayLabel,
                style: TextStyle(
                  color: isSelected
                      ? colors.sidebarSelectedItemTextColor
                      : colors.sidebarItemTextColor,
                  fontSize: 13,
                  fontWeight: isSelected ? FontWeight.bold : FontWeight.normal,
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
              decoration: BoxDecoration(
                color: isSelected
                    ? Color.fromRGBO(colors.accentAmberColor.red, colors.accentAmberColor.green, colors.accentAmberColor.blue, 0.2)
                    : Color.fromRGBO(colors.dividerColor.red, colors.dividerColor.green, colors.dividerColor.blue, 0.3),
                borderRadius: BorderRadius.circular(10),
              ),
              child: Text(
                '$count',
                style: TextStyle(
                  color: isSelected
                      ? colors.accentAmberColor
                      : Color.fromRGBO(colors.sidebarItemTextColor.red, colors.sidebarItemTextColor.green, colors.sidebarItemTextColor.blue, 0.7),
                  fontSize: 11,
                  fontWeight: FontWeight.bold,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildDrawerFooter(BuildContext context) {
    final colors = context.appColors;
    final themesBloc = context.read<ThemesBloc>();

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      child: Column(
        children: [
          // Theme Selector Tile
          PopupMenuButton<AppTheme>(
            color: colors.popupMenuBackgroundColor,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(8),
              side: BorderSide(color: colors.popupMenuBorderColor),
            ),
            onSelected: (theme) => themesBloc.add(ChangeThemeEvent(theme)),
            itemBuilder: (context) {
              return ThemesBloc.availableThemes.map((theme) {
                final isSelected = theme.themeCode == colors.themeCode;
                return PopupMenuItem<AppTheme>(
                  value: theme,
                  child: Row(
                    children: [
                      Icon(
                        isSelected ? Icons.check_circle : Icons.circle_outlined,
                        size: 16,
                        color: isSelected
                            ? colors.accentGreenColor
                            : colors.tableSecondaryTextColor,
                      ),
                      const SizedBox(width: 10),
                      Text(
                        theme.themeName,
                        style: TextStyle(
                          color: isSelected
                              ? colors.tablePrimaryTextColor
                              : colors.tableSecondaryTextColor,
                          fontSize: 13,
                        ),
                      ),
                    ],
                  ),
                );
              }).toList();
            },
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(6),
                border: Border.all(color: colors.dividerColor),
              ),
              child: Row(
                children: [
                  Icon(
                    Icons.palette_outlined,
                    size: 18,
                    color: colors.titleBarIconColor,
                  ),
                  const SizedBox(width: 10),
                  Text(
                    'Theme: ${colors.themeName}',
                    style: TextStyle(
                      color: colors.sidebarItemTextColor,
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  const Spacer(),
                  Icon(
                    Icons.arrow_drop_down,
                    size: 18,
                    color: colors.sidebarItemTextColor,
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 6),

          // Action Buttons Row: Logs, Logout, Shutdown
          Row(
            children: [
              Expanded(
                child: TextButton.icon(
                  onPressed: () {
                    Navigator.of(context).pop();
                    onShowLogs();
                  },
                  icon: Icon(
                    Icons.terminal_rounded,
                    size: 16,
                    color: colors.titleBarTextColor,
                  ),
                  label: Text(
                    'Logs',
                    style: TextStyle(
                      color: colors.titleBarTextColor,
                      fontSize: 12,
                    ),
                  ),
                  style: TextButton.styleFrom(
                    padding: const EdgeInsets.symmetric(vertical: 8),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(6),
                    ),
                  ),
                ),
              ),
              Expanded(
                child: TextButton.icon(
                  onPressed: () {
                    Navigator.of(context).pop();
                    onLogout();
                  },
                  icon: Icon(
                    Icons.logout_rounded,
                    size: 16,
                    color: colors.titleBarTextColor,
                  ),
                  label: Text(
                    'Logout',
                    style: TextStyle(
                      color: colors.titleBarTextColor,
                      fontSize: 12,
                    ),
                  ),
                  style: TextButton.styleFrom(
                    padding: const EdgeInsets.symmetric(vertical: 8),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(6),
                    ),
                  ),
                ),
              ),
              Expanded(
                child: TextButton.icon(
                  onPressed: () {
                    Navigator.of(context).pop();
                    onShutdown();
                  },
                  icon: Icon(
                    Icons.power_settings_new_rounded,
                    size: 16,
                    color: colors.accentRedColor,
                  ),
                  label: Text(
                    'Shutdown',
                    style: TextStyle(
                      color: colors.accentRedColor,
                      fontSize: 12,
                    ),
                  ),
                  style: TextButton.styleFrom(
                    padding: const EdgeInsets.symmetric(vertical: 8),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(6),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
