import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hydroxy_rc/configs/converter.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/state/themes_bloc/themes_bloc.dart';
import 'package:hydroxy_rc/state/torrents_bloc/torrents_bloc.dart';
import 'package:hydroxy_rc/themes/app_theme.dart';

class MobileCategoriesDrawer extends StatefulWidget {
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
  State<MobileCategoriesDrawer> createState() => _MobileCategoriesDrawerState();
}

class _MobileCategoriesDrawerState extends State<MobileCategoriesDrawer> {
  List<StorageTargetModel> _storages = [];

  @override
  void initState() {
    super.initState();
    _loadStorages();
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
                      _buildStoragesSection(context, torrents),
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
                      color: Color.fromRGBO(colors.titleBarTextColor.red, colors.titleBarTextColor.green, colors.titleBarTextColor.blue, 0.7),
                      fontSize: 12,
                    ),
                  ),
                ],
              ),
            ],
          ),
          if (widget.systemStats != null) ...[
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: _buildHeaderStat(
                    icon: Icons.arrow_downward_rounded,
                    label: 'Down',
                    value: formatSpeed(widget.systemStats!.downloadSpeed),
                    color: colors.downloadSpeedColor,
                  ),
                ),
                Expanded(
                  child: _buildHeaderStat(
                    icon: Icons.arrow_upward_rounded,
                    label: 'Up',
                    value: formatSpeed(widget.systemStats!.uploadSpeed),
                    color: colors.uploadSpeedColor,
                  ),
                ),
                Expanded(
                  child: _buildHeaderStat(
                    icon: Icons.hub_rounded,
                    label: 'Peers',
                    value: '${widget.systemStats!.peers}',
                    color: colors.accentBlueColor,
                  ),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildHeaderStat({
    required IconData icon,
    required String label,
    required String value,
    required Color color,
  }) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 14, color: color),
        const SizedBox(width: 4),
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              label,
              style: const TextStyle(
                color: Colors.white54,
                fontSize: 9,
                fontWeight: FontWeight.w500,
              ),
            ),
            Text(
              value,
              style: TextStyle(
                color: color,
                fontSize: 11,
                fontWeight: FontWeight.bold,
              ),
            ),
          ],
        ),
      ],
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
        widget.selectedFilter == filterKey ||
        (widget.selectedFilter == null && filterKey == 'all');

    return InkWell(
      onTap: () {
        widget.onFilterSelected(filterKey == 'all' ? null : filterKey);
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

  Widget _buildStoragesSection(
    BuildContext context,
    List<TorrentStructure> torrents,
  ) {
    final storageNames = <String>{
      ..._storages.map((s) => s.name),
      ...torrents.map((t) => t.storage.isEmpty ? 'default' : t.storage),
    };
    if (storageNames.isEmpty) {
      storageNames.add('default');
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _buildSectionHeader(context, 'STORAGE DISKS'),
        for (final storageName in storageNames) ...[
          _buildStorageDiskItem(context, storageName, torrents),
        ],
      ],
    );
  }

  Widget _buildStorageDiskItem(
    BuildContext context,
    String storageName,
    List<TorrentStructure> torrents,
  ) {
    final storageTorrents = torrents
        .where(
          (t) => (t.storage.isEmpty ? 'default' : t.storage) == storageName,
        )
        .toList();

    final knownCats = _storages
        .firstWhere(
          (s) => s.name == storageName,
          orElse: () => StorageTargetModel(
            name: storageName,
            watchDirectory: '',
            downloadDirectory: '',
          ),
        )
        .categories;

    final allCats = <String>{
      ...knownCats,
      ...storageTorrents
          .map((t) => t.category.trim())
          .where((c) => c.isNotEmpty),
    }.toList()
      ..sort();

    final uncategorizedCount =
        storageTorrents.where((t) => t.category.trim().isEmpty).length;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // Storage Disk Header
        _buildCategoryItem(
          context: context,
          filterKey: 'storage:$storageName',
          displayLabel: storageName,
          icon: Icons.dns_rounded,
          count: storageTorrents.length,
          isStorageHeader: true,
        ),

        // Nested Categories (including empty categories with count 0!)
        ...allCats.map((cat) {
          final catCount = storageTorrents.where((t) {
            final tc = t.category.trim();
            return tc == cat || tc.startsWith('$cat/');
          }).length;

          return _buildCategoryItem(
            context: context,
            filterKey: 'storage:$storageName/category:$cat',
            displayLabel: cat,
            icon: Icons.folder_outlined,
            count: catCount,
            indent: 28.0,
          );
        }),

        if (uncategorizedCount > 0)
          _buildCategoryItem(
            context: context,
            filterKey: 'storage:$storageName/category:',
            displayLabel: 'Uncategorized',
            icon: Icons.folder_off_outlined,
            count: uncategorizedCount,
            indent: 28.0,
          ),
      ],
    );
  }

  Widget _buildCategoryItem({
    required BuildContext context,
    required String filterKey,
    required String displayLabel,
    required IconData icon,
    required int count,
    bool isStorageHeader = false,
    double indent = 12.0,
  }) {
    final colors = context.appColors;
    final isSelected = widget.selectedFilter == filterKey;

    return InkWell(
      onTap: () {
        widget.onFilterSelected(filterKey);
        Navigator.of(context).pop();
      },
      child: Container(
        margin: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
        padding: EdgeInsets.only(
          left: indent,
          right: 12,
          top: 9,
          bottom: 9,
        ),
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
              size: isStorageHeader ? 18 : 16,
              color: isSelected
                  ? colors.accentAmberColor
                  : (isStorageHeader
                        ? colors.accentPurpleColor
                        : colors.sidebarIconColor),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Text(
                displayLabel,
                style: TextStyle(
                  color: isSelected
                      ? colors.sidebarSelectedItemTextColor
                      : colors.sidebarItemTextColor,
                  fontSize: isStorageHeader ? 13 : 12,
                  fontWeight: (isSelected || isStorageHeader)
                      ? FontWeight.bold
                      : FontWeight.normal,
                ),
              ),
            ),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
              decoration: BoxDecoration(
                color: isSelected
                    ? Color.fromRGBO(colors.accentAmberColor.red, colors.accentAmberColor.green, colors.accentAmberColor.blue, 0.2)
                    : Color.fromRGBO(colors.dividerColor.red, colors.dividerColor.green, colors.dividerColor.blue, 0.3),
                borderRadius: BorderRadius.circular(8),
              ),
              child: Text(
                '$count',
                style: TextStyle(
                  color: isSelected
                      ? colors.accentAmberColor
                      : Color.fromRGBO(colors.sidebarItemTextColor.red, colors.sidebarItemTextColor.green, colors.sidebarItemTextColor.blue, 0.7),
                  fontSize: 10,
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

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      color: colors.titleBarBackgroundColor,
      child: Column(
        children: [
          // Theme Selector Row
          BlocBuilder<ThemesBloc, ThemesState>(
            builder: (context, state) {
              final currentTheme = state.maybeWhen(
                changed: (theme) => theme,
                orElse: () => AppTheme.dark,
              );

              return Row(
                children: [
                  Icon(
                    Icons.palette_outlined,
                    size: 16,
                    color: colors.titleBarTextColor,
                  ),
                  const SizedBox(width: 8),
                  Text(
                    'Theme:',
                    style: TextStyle(
                      color: colors.titleBarTextColor,
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Container(
                      height: 28,
                      padding: const EdgeInsets.symmetric(horizontal: 8),
                      decoration: BoxDecoration(
                        color: colors.sidebarBackgroundColor,
                        borderRadius: BorderRadius.circular(6),
                        border: Border.all(color: colors.dividerColor),
                      ),
                      child: DropdownButtonHideUnderline(
                        child: DropdownButton<AppTheme>(
                          value: currentTheme,
                          dropdownColor: colors.sidebarBackgroundColor,
                          isDense: true,
                          icon: Icon(
                            Icons.arrow_drop_down,
                            size: 16,
                            color: colors.titleBarTextColor,
                          ),
                          items: AppTheme.values.map((theme) {
                            return DropdownMenuItem<AppTheme>(
                              value: theme,
                              child: Text(
                                theme.name.toUpperCase(),
                                style: TextStyle(
                                  color: colors.tablePrimaryTextColor,
                                  fontSize: 11,
                                  fontWeight: FontWeight.w500,
                                ),
                              ),
                            );
                          }).toList(),
                          onChanged: (newTheme) {
                            if (newTheme != null) {
                              context.read<ThemesBloc>().add(
                                    ThemesEvent.changeTheme(theme: newTheme),
                                  );
                            }
                          },
                        ),
                      ),
                    ),
                  ),
                ],
              );
            },
          ),
          const SizedBox(height: 8),

          // Action Buttons: Logs, Logout, Shutdown
          Row(
            children: [
              Expanded(
                child: TextButton.icon(
                  onPressed: () {
                    Navigator.of(context).pop();
                    widget.onShowLogs();
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
                    widget.onLogout();
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
                    widget.onShutdown();
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
