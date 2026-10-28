import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hydroxy_rc/configs/converter.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/routes/routes_name.dart';
import 'package:hydroxy_rc/state/themes_bloc/themes_bloc.dart';
import 'package:hydroxy_rc/state/torrents_bloc/torrents_bloc.dart';
import 'package:hydroxy_rc/ui/screens/main_content/models/torrent_structure_ext.dart';
import 'package:hydroxy_rc/ui/screens/mobile/mobile_torrent_details_screen.dart';
import 'package:hydroxy_rc/ui/screens/mobile/widgets/mobile_categories_drawer.dart';
import 'package:hydroxy_rc/ui/screens/mobile/widgets/mobile_torrent_card.dart';
import 'package:hydroxy_rc/ui/widgets/log_viewer_dialog.dart';

class MobileMainScreen extends StatefulWidget {
  final String? initialFilter;

  const MobileMainScreen({
    super.key,
    this.initialFilter,
  });

  @override
  State<MobileMainScreen> createState() => _MobileMainScreenState();
}

class _MobileMainScreenState extends State<MobileMainScreen> {
  final GlobalKey<ScaffoldState> _scaffoldKey = GlobalKey<ScaffoldState>();
  String? _selectedCategoryFilter;

  // Search state
  bool _isSearchVisible = false;
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';
  bool _isRegex = false;
  bool _hasRegexError = false;

  Timer? _statsTimer;
  bool _isFetchingStats = false;
  SystemStatsModel? _systemStats;

  @override
  void initState() {
    super.initState();
    _selectedCategoryFilter = widget.initialFilter;
    context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
    _fetchSystemStats();
    _statsTimer = Timer.periodic(const Duration(seconds: 2), (_) {
      _fetchSystemStats();
    });
  }

  @override
  void dispose() {
    _searchController.dispose();
    _statsTimer?.cancel();
    super.dispose();
  }

  Future<void> _fetchSystemStats() async {
    if (_isFetchingStats) return;
    _isFetchingStats = true;
    try {
      final stats = await getSystemStats();
      if (mounted) {
        setState(() {
          _systemStats = stats;
        });
      }
    } catch (_) {
      if (mounted && _systemStats != null) {
        setState(() {
          _systemStats = null;
        });
      }
    } finally {
      _isFetchingStats = false;
    }
  }

  void _validateRegex() {
    if (!_isRegex || _searchQuery.isEmpty) {
      _hasRegexError = false;
      return;
    }
    try {
      var p = _searchQuery.trim();
      if (p.startsWith('*') || p.startsWith('+') || p.startsWith('?')) {
        p = '.$p';
      }
      RegExp(p);
      _hasRegexError = false;
    } catch (_) {
      _hasRegexError = true;
    }
  }

  List<TorrentStructure> _filterTorrents(List<TorrentStructure> list) {
    var result = list;

    // Filter by Category or Status
    if (_selectedCategoryFilter != null && _selectedCategoryFilter != 'all') {
      final f = _selectedCategoryFilter!;
      if (f.startsWith('status:')) {
        final target = f.substring('status:'.length).toLowerCase();
        result = result.where((t) {
          final s = t.status.toLowerCase();
          if (target == 'downloading') return s == 'downloading';
          if (target == 'stalled') return s == 'stalled';
          if (target == 'paused') return s == 'paused' || s == 'stopped';
          if (target == 'completed') return s == 'completed' || s == 'seeding';
          if (target == 'queued') return s == 'queued';
          return s == target;
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

    // Filter by Search Query
    if (_searchQuery.trim().isNotEmpty) {
      final q = _searchQuery.trim();
      if (_isRegex && !_hasRegexError) {
        try {
          var pattern = q;
          if (pattern.startsWith('*') || pattern.startsWith('+') || pattern.startsWith('?')) {
            pattern = '.$pattern';
          }
          final regex = RegExp(pattern, caseSensitive: false);
          result = result.where((t) => regex.hasMatch(t.name) || regex.hasMatch(t.category) || regex.hasMatch(t.infoHash)).toList();
        } catch (_) {
          final cleanQ = q.toLowerCase();
          result = result.where((t) => t.name.toLowerCase().contains(cleanQ) || t.category.toLowerCase().contains(cleanQ) || t.infoHash.toLowerCase().contains(cleanQ)).toList();
        }
      } else {
        final cleanQ = q.toLowerCase();
        result = result.where((t) => t.name.toLowerCase().contains(cleanQ) || t.category.toLowerCase().contains(cleanQ) || t.infoHash.toLowerCase().contains(cleanQ)).toList();
      }
    }

    return result;
  }

  Future<void> _recheckTorrentDialog(TorrentStructure torrent) async {
    final colors = context.appColors;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: colors.popupMenuBackgroundColor,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(10),
          side: BorderSide(color: colors.popupMenuBorderColor),
        ),
        title: Row(
          children: [
            Icon(Icons.sync_rounded, color: colors.accentBlueColor, size: 22),
            const SizedBox(width: 8),
            const Text('Force Recheck', style: TextStyle(fontSize: 16)),
          ],
        ),
        content: Text(
          'Re-verify downloaded data on disk for "${torrent.name}"? Active transfer will be paused during verification.',
          style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: Text('Cancel', style: TextStyle(color: colors.tableSecondaryTextColor)),
          ),
          ElevatedButton(
            style: ElevatedButton.styleFrom(
              backgroundColor: colors.accentBlueColor,
              foregroundColor: Colors.white,
            ),
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Recheck'),
          ),
        ],
      ),
    );

    if (confirmed == true && mounted) {
      await recheckTorrent(infoHash: torrent.infoHash);
      if (mounted) {
        context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
      }
    }
  }

  Future<void> _removeTorrentDialog(TorrentStructure torrent) async {
    final colors = context.appColors;
    bool deleteFiles = false;

    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => StatefulBuilder(
        builder: (ctx, setDialogState) => AlertDialog(
          backgroundColor: colors.popupMenuBackgroundColor,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(10),
            side: BorderSide(color: colors.popupMenuBorderColor),
          ),
          title: Row(
            children: [
              Icon(Icons.delete_outline_rounded, color: colors.accentRedColor, size: 22),
              const SizedBox(width: 8),
              const Text('Remove Torrent', style: TextStyle(fontSize: 16)),
            ],
          ),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Remove "${torrent.name}" from transfer list?',
                style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 13),
              ),
              const SizedBox(height: 12),
              InkWell(
                onTap: () {
                  setDialogState(() {
                    deleteFiles = !deleteFiles;
                  });
                },
                child: Row(
                  children: [
                    Checkbox(
                      value: deleteFiles,
                      activeColor: colors.accentRedColor,
                      onChanged: (val) {
                        setDialogState(() {
                          deleteFiles = val ?? false;
                        });
                      },
                    ),
                    Expanded(
                      child: Text(
                        'Also delete downloaded files from disk',
                        style: TextStyle(
                          color: deleteFiles ? colors.accentRedColor : colors.tablePrimaryTextColor,
                          fontSize: 13,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: Text('Cancel', style: TextStyle(color: colors.tableSecondaryTextColor)),
            ),
            ElevatedButton(
              style: ElevatedButton.styleFrom(
                backgroundColor: colors.accentRedColor,
                foregroundColor: Colors.white,
              ),
              onPressed: () => Navigator.of(ctx).pop(true),
              child: const Text('Remove'),
            ),
          ],
        ),
      ),
    );

    if (confirmed == true && mounted) {
      await removeTorrent(infoHash: torrent.infoHash, deleteFiles: deleteFiles);
      if (mounted) {
        context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
      }
    }
  }

  void _showLogoutDialog() {
    final colors = context.appColors;
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: colors.popupMenuBackgroundColor,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(10),
          side: BorderSide(color: colors.popupMenuBorderColor),
        ),
        title: Row(
          children: [
            Icon(Icons.logout, color: colors.titleBarIconColor, size: 20),
            const SizedBox(width: 8),
            const Text('Confirm Logout', style: TextStyle(fontSize: 16)),
          ],
        ),
        content: Text(
          'Are you sure you want to log out of HydroxyTorrent?',
          style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: Text('Cancel', style: TextStyle(color: colors.tableSecondaryTextColor)),
          ),
          ElevatedButton(
            style: ElevatedButton.styleFrom(
              backgroundColor: colors.titleBarLogoColor,
              foregroundColor: Colors.white,
            ),
            onPressed: () async {
              Navigator.of(ctx).pop();
              await ApiClient.instance.logout();
              if (mounted) {
                Navigator.of(context).pushReplacementNamed(RoutesName.initialScreen);
              }
            },
            child: const Text('Logout'),
          ),
        ],
      ),
    );
  }

  void _showShutdownDialog() {
    final colors = context.appColors;
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: colors.popupMenuBackgroundColor,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(10),
          side: BorderSide(color: colors.popupMenuBorderColor),
        ),
        title: Row(
          children: [
            Icon(Icons.power_settings_new, color: colors.accentRedColor, size: 20),
            const SizedBox(width: 8),
            const Text('Shutdown Daemon', style: TextStyle(fontSize: 16)),
          ],
        ),
        content: Text(
          'Are you sure you want to stop the HydroxyTorrent background daemon process? You will lose remote connectivity.',
          style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: Text('Cancel', style: TextStyle(color: colors.tableSecondaryTextColor)),
          ),
          ElevatedButton(
            style: ElevatedButton.styleFrom(
              backgroundColor: colors.accentRedColor,
              foregroundColor: Colors.white,
            ),
            onPressed: () async {
              Navigator.of(ctx).pop();
              await shutdownDaemon();
              if (mounted) {
                Navigator.of(context).pushReplacementNamed(
                  RoutesName.initialScreen,
                  arguments: 'Daemon has been shut down.',
                );
              }
            },
            child: const Text('Shutdown'),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;

    return Scaffold(
      key: _scaffoldKey,
      backgroundColor: colors.scaffoldBackgroundColor,
      drawer: MobileCategoriesDrawer(
        selectedFilter: _selectedCategoryFilter,
        systemStats: _systemStats,
        onFilterSelected: (filter) {
          setState(() {
            _selectedCategoryFilter = filter;
          });
        },
        onShowLogs: () => LogViewerDialog.show(context),
        onLogout: _showLogoutDialog,
        onShutdown: _showShutdownDialog,
      ),
      appBar: AppBar(
        backgroundColor: colors.titleBarBackgroundColor,
        elevation: 0,
        leading: IconButton(
          icon: Icon(Icons.menu_rounded, color: colors.titleBarTextColor),
          tooltip: 'Open Categories & Filters',
          onPressed: () => _scaffoldKey.currentState?.openDrawer(),
        ),
        title: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.image_not_supported_outlined, size: 18, color: colors.titleBarLogoColor),
            const SizedBox(width: 6),
            Text(
              'HydroxyTorrent',
              style: TextStyle(
                color: colors.titleBarTextColor,
                fontSize: 16,
                fontWeight: FontWeight.bold,
              ),
            ),
          ],
        ),
        actions: [
          // Search Toggle Button
          IconButton(
            icon: Icon(
              _isSearchVisible ? Icons.search_off_rounded : Icons.search_rounded,
              color: _isSearchVisible ? colors.titleBarLogoColor : colors.titleBarIconColor,
            ),
            tooltip: _isSearchVisible ? 'Close Search' : 'Search Torrents',
            onPressed: () {
              setState(() {
                _isSearchVisible = !_isSearchVisible;
                if (!_isSearchVisible) {
                  _searchController.clear();
                  _searchQuery = '';
                  _hasRegexError = false;
                }
              });
            },
          ),

          // More Options Menu (Theme, Logs, Logout, Shutdown)
          PopupMenuButton<String>(
            icon: Icon(Icons.more_vert_rounded, color: colors.titleBarIconColor),
            color: colors.popupMenuBackgroundColor,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(8),
              side: BorderSide(color: colors.popupMenuBorderColor),
            ),
            onSelected: (val) {
              switch (val) {
                case 'theme':
                  _showThemeDialog();
                  break;
                case 'logs':
                  LogViewerDialog.show(context);
                  break;
                case 'logout':
                  _showLogoutDialog();
                  break;
                case 'shutdown':
                  _showShutdownDialog();
                  break;
              }
            },
            itemBuilder: (context) => [
              PopupMenuItem(
                value: 'theme',
                child: Row(
                  children: [
                    Icon(Icons.palette_outlined, size: 16, color: colors.tablePrimaryTextColor),
                    const SizedBox(width: 8),
                    Text('Theme', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
                  ],
                ),
              ),
              PopupMenuItem(
                value: 'logs',
                child: Row(
                  children: [
                    Icon(Icons.terminal_rounded, size: 16, color: colors.tablePrimaryTextColor),
                    const SizedBox(width: 8),
                    Text('Daemon Logs', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
                  ],
                ),
              ),
              PopupMenuItem(
                value: 'logout',
                child: Row(
                  children: [
                    Icon(Icons.logout_rounded, size: 16, color: colors.tablePrimaryTextColor),
                    const SizedBox(width: 8),
                    Text('Logout', style: TextStyle(color: colors.tablePrimaryTextColor, fontSize: 13)),
                  ],
                ),
              ),
              const PopupMenuDivider(),
              PopupMenuItem(
                value: 'shutdown',
                child: Row(
                  children: [
                    Icon(Icons.power_settings_new_rounded, size: 16, color: colors.accentRedColor),
                    const SizedBox(width: 8),
                    Text('Shutdown Daemon', style: TextStyle(color: colors.accentRedColor, fontSize: 13)),
                  ],
                ),
              ),
            ],
          ),
        ],
      ),
      body: Column(
        children: [
          // Animated Search Bar Area
          if (_isSearchVisible) _buildSearchBar(context),

          // Mini Status Bar (connection, RAM, total speeds)
          _buildStatusBar(context),

          // Horizontal Status Filter Chips Bar
          _buildStatusFilterChips(context),

          // Torrents List
          Expanded(
            child: BlocBuilder<TorrentsBloc, TorrentsState>(
              builder: (context, state) {
                return state.when(
                  initial: () => const Center(child: CircularProgressIndicator()),
                  loading: () => const Center(child: CircularProgressIndicator()),
                  loaded: (allTorrents) {
                    final filteredTorrents = _filterTorrents(allTorrents);

                    if (filteredTorrents.isEmpty) {
                      return _buildEmptyState(context, allTorrents.isEmpty);
                    }

                    return RefreshIndicator(
                      onRefresh: () async {
                        context.read<TorrentsBloc>().add(const TorrentsEvent.fetch());
                      },
                      child: ListView.builder(
                        padding: const EdgeInsets.only(top: 4, bottom: 8),
                        itemCount: filteredTorrents.length,
                        itemBuilder: (context, index) {
                          final torrent = filteredTorrents[index];
                          return MobileTorrentCard(
                            key: ValueKey(torrent.infoHash),
                            torrent: torrent,
                            onTap: () {
                              Navigator.of(context).push(
                                MaterialPageRoute(
                                  builder: (ctx) => MobileTorrentDetailsScreen(
                                    initialTorrent: torrent,
                                  ),
                                ),
                              );
                            },
                            onRecheck: () => _recheckTorrentDialog(torrent),
                            onRemove: () => _removeTorrentDialog(torrent),
                          );
                        },
                      ),
                    );
                  },
                );
              },
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildSearchBar(BuildContext context) {
    final colors = context.appColors;

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      color: colors.titleBarBackgroundColor,
      child: Container(
        height: 36,
        decoration: BoxDecoration(
          color: colors.searchBarBackgroundColor,
          borderRadius: BorderRadius.circular(6),
          border: Border.all(
            color: _hasRegexError
                ? colors.searchBarErrorBorderColor
                : (_isRegex ? colors.searchBarActiveBorderColor : colors.searchBarBorderColor),
            width: 0.8,
          ),
        ),
        child: Row(
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8),
              child: Icon(Icons.search_rounded, size: 16, color: colors.searchBarIconColor),
            ),
            Expanded(
              child: TextField(
                controller: _searchController,
                autofocus: true,
                onChanged: (val) {
                  setState(() {
                    _searchQuery = val;
                    _validateRegex();
                  });
                },
                style: TextStyle(
                  color: _hasRegexError ? colors.searchBarErrorBorderColor : colors.searchBarTextColor,
                  fontSize: 13,
                ),
                decoration: InputDecoration(
                  isDense: true,
                  hintText: _isRegex ? 'Regex filter (e.g. ^ubuntu)...' : 'Search torrents...',
                  hintStyle: TextStyle(color: colors.searchBarHintColor, fontSize: 13),
                  border: InputBorder.none,
                  contentPadding: const EdgeInsets.symmetric(vertical: 8),
                ),
              ),
            ),
            // Regex Button
            InkWell(
              onTap: () {
                setState(() {
                  _isRegex = !_isRegex;
                  _validateRegex();
                });
              },
              borderRadius: BorderRadius.circular(4),
              child: Container(
                margin: const EdgeInsets.symmetric(horizontal: 4),
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(
                  color: _isRegex
                      ? (_hasRegexError
                          ? Color.fromRGBO(colors.searchBarErrorBorderColor.red, colors.searchBarErrorBorderColor.green, colors.searchBarErrorBorderColor.blue, 0.2)
                          : colors.searchBarRegexActiveBackgroundColor)
                      : Colors.transparent,
                  borderRadius: BorderRadius.circular(4),
                  border: Border.all(
                    color: _isRegex
                        ? (_hasRegexError ? colors.searchBarErrorBorderColor : colors.searchBarActiveBorderColor)
                        : Colors.transparent,
                    width: 0.8,
                  ),
                ),
                child: Text(
                  '.*',
                  style: TextStyle(
                    color: _isRegex
                        ? (_hasRegexError ? colors.searchBarErrorBorderColor : colors.searchBarRegexActiveTextColor)
                        : colors.searchBarRegexInactiveTextColor,
                    fontSize: 11,
                    fontWeight: FontWeight.bold,
                    fontFamily: 'monospace',
                  ),
                ),
              ),
            ),
            if (_searchQuery.isNotEmpty)
              IconButton(
                icon: Icon(Icons.clear_rounded, size: 14, color: colors.searchBarIconColor),
                onPressed: () {
                  _searchController.clear();
                  setState(() {
                    _searchQuery = '';
                    _hasRegexError = false;
                  });
                },
              ),
          ],
        ),
      ),
    );
  }

  Widget _buildStatusFilterChips(BuildContext context) {
    final colors = context.appColors;

    return BlocBuilder<TorrentsBloc, TorrentsState>(
      builder: (context, state) {
        final torrents = state.maybeWhen(
          loaded: (torrents) => torrents,
          orElse: () => const <TorrentStructure>[],
        );

        final chips = [
          _FilterChipData('all', 'All', Icons.all_inbox_rounded, torrents.length, colors.accentBlueColor),
          _FilterChipData(
            'status:downloading',
            'Downloading',
            Icons.arrow_downward_rounded,
            torrents.where((t) => t.status.toLowerCase() == 'downloading').length,
            colors.accentGreenColor,
          ),
          _FilterChipData(
            'status:completed',
            'Seeding',
            Icons.arrow_upward_rounded,
            torrents.where((t) => t.status.toLowerCase() == 'completed' || t.status.toLowerCase() == 'seeding').length,
            colors.accentPurpleColor,
          ),
          _FilterChipData(
            'status:stalled',
            'Stalled',
            Icons.hourglass_empty_rounded,
            torrents.where((t) => t.status.toLowerCase() == 'stalled').length,
            colors.accentAmberColor,
          ),
          _FilterChipData(
            'status:queued',
            'Queued',
            Icons.schedule_rounded,
            torrents.where((t) => t.status.toLowerCase() == 'queued').length,
            colors.tableAccentTextColor,
          ),
          _FilterChipData(
            'status:paused',
            'Paused',
            Icons.pause_circle_outline_rounded,
            torrents.where((t) => t.status.toLowerCase() == 'paused' || t.status.toLowerCase() == 'stopped').length,
            colors.tableSecondaryTextColor,
          ),
        ];

        return Container(
          height: 42,
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: ListView.separated(
            scrollDirection: Axis.horizontal,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            itemCount: chips.length,
            separatorBuilder: (_, __) => const SizedBox(width: 6),
            itemBuilder: (context, i) {
              final chip = chips[i];
              final isSelected = _selectedCategoryFilter == chip.filterKey ||
                  (_selectedCategoryFilter == null && chip.filterKey == 'all');

              return InkWell(
                onTap: () {
                  setState(() {
                    _selectedCategoryFilter = chip.filterKey == 'all' ? null : chip.filterKey;
                  });
                },
                borderRadius: BorderRadius.circular(16),
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                  decoration: BoxDecoration(
                    color: isSelected ? Color.fromRGBO(chip.color.red, chip.color.green, chip.color.blue, 0.18) : colors.tableHeaderBackgroundColor,
                    borderRadius: BorderRadius.circular(16),
                    border: Border.all(
                      color: isSelected ? chip.color : Color.fromRGBO(colors.dividerColor.red, colors.dividerColor.green, colors.dividerColor.blue, 0.5),
                      width: isSelected ? 1.2 : 0.8,
                    ),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(chip.icon, size: 13, color: isSelected ? chip.color : colors.tableSecondaryTextColor),
                      const SizedBox(width: 5),
                      Text(
                        chip.label,
                        style: TextStyle(
                          color: isSelected ? chip.color : colors.tablePrimaryTextColor,
                          fontSize: 12,
                          fontWeight: isSelected ? FontWeight.bold : FontWeight.w500,
                        ),
                      ),
                      const SizedBox(width: 5),
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                        decoration: BoxDecoration(
                          color: isSelected ? Color.fromRGBO(chip.color.red, chip.color.green, chip.color.blue, 0.25) : Color.fromRGBO(colors.dividerColor.red, colors.dividerColor.green, colors.dividerColor.blue, 0.3),
                          borderRadius: BorderRadius.circular(8),
                        ),
                        child: Text(
                          '${chip.count}',
                          style: TextStyle(
                            color: isSelected ? chip.color : colors.tableSecondaryTextColor,
                            fontSize: 10,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              );
            },
          ),
        );
      },
    );
  }

  Widget _buildEmptyState(BuildContext context, bool noTorrentsAtAll) {
    final colors = context.appColors;

    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              noTorrentsAtAll ? Icons.cloud_download_outlined : Icons.filter_list_off_rounded,
              size: 48,
              color: Color.fromRGBO(colors.tableSecondaryTextColor.red, colors.tableSecondaryTextColor.green, colors.tableSecondaryTextColor.blue, 0.4),
            ),
            const SizedBox(height: 12),
            Text(
              noTorrentsAtAll ? 'No Torrents Added' : 'No Matching Torrents',
              style: TextStyle(
                color: colors.tablePrimaryTextColor,
                fontSize: 16,
                fontWeight: FontWeight.bold,
              ),
            ),
            const SizedBox(height: 6),
            Text(
              noTorrentsAtAll
                  ? 'Torrents downloaded via daemon will appear here.'
                  : 'Try changing category filter or search query.',
              style: TextStyle(
                color: colors.tableSecondaryTextColor,
                fontSize: 12,
              ),
              textAlign: TextAlign.center,
            ),
            if (!noTorrentsAtAll) ...[
              const SizedBox(height: 12),
              TextButton(
                onPressed: () {
                  setState(() {
                    _selectedCategoryFilter = null;
                    _searchController.clear();
                    _searchQuery = '';
                    _hasRegexError = false;
                  });
                },
                child: const Text('Reset Filters'),
              ),
            ],
          ],
        ),
      ),
    );
  }

  Widget _buildStatusBar(BuildContext context) {
    final colors = context.appColors;

    return ValueListenableBuilder<DaemonConnectionState>(
      valueListenable: ApiClient.instance.connectionStateNotifier,
      builder: (context, connState, _) {
        final isOnline = connState == DaemonConnectionState.connected;
        final statusColor = connState == DaemonConnectionState.connected
            ? colors.accentGreenColor
            : (connState == DaemonConnectionState.disconnected
                ? colors.accentRedColor
                : colors.accentAmberColor);
        final statusText = connState == DaemonConnectionState.connected
            ? 'Connected'
            : (connState == DaemonConnectionState.disconnected
                ? 'Disconnected'
                : 'Connecting...');

        return Container(
          height: 26,
          decoration: BoxDecoration(
            color: colors.statusBarBackgroundColor,
            border: Border.symmetric(
              horizontal: BorderSide(color: colors.statusBarBorderColor),
            ),
          ),
          padding: const EdgeInsets.symmetric(horizontal: 10),
          child: Row(
            children: [
              Container(
                width: 7,
                height: 7,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: statusColor,
                ),
              ),
              const SizedBox(width: 5),
              Text(
                statusText,
                style: TextStyle(color: colors.statusBarTextColor, fontSize: 11),
              ),
              if (isOnline && _systemStats != null) ...[
                const SizedBox(width: 8),
                Text(
                  '•',
                  style: TextStyle(
                    color: Color.fromRGBO(colors.statusBarTextColor.red, colors.statusBarTextColor.green, colors.statusBarTextColor.blue, 0.5),
                    fontSize: 10,
                  ),
                ),
                const SizedBox(width: 8),
                Text(
                  'RAM: ${formatBytesDynamic(_systemStats!.processRssBytes.toString())}',
                  style: TextStyle(
                    color: colors.statusBarTextColor,
                    fontSize: 11,
                    fontFamily: 'monospace',
                  ),
                ),
              ],
              const Spacer(),
              BlocBuilder<TorrentsBloc, TorrentsState>(
                builder: (context, state) {
                  final torrents = isOnline
                      ? state.maybeWhen(
                          loaded: (torrents) => torrents,
                          orElse: () => const <TorrentStructure>[],
                        )
                      : const <TorrentStructure>[];

                  int totalDown = 0;
                  int totalUp = 0;
                  for (final t in torrents) {
                    totalDown += t.downloadSpeedBytes;
                    totalUp += t.uploadSpeedBytes;
                  }

                  return Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(Icons.arrow_downward_rounded, size: 12, color: colors.downloadSpeedColor),
                      const SizedBox(width: 2),
                      Text(
                        formatBytesDynamic(totalDown.toString(), isSpeed: true),
                        style: TextStyle(
                          color: colors.downloadSpeedColor,
                          fontSize: 11,
                          fontWeight: FontWeight.bold,
                          fontFamily: 'monospace',
                        ),
                      ),
                      const SizedBox(width: 8),
                      Icon(Icons.arrow_upward_rounded, size: 12, color: colors.uploadSpeedColor),
                      const SizedBox(width: 2),
                      Text(
                        formatBytesDynamic(totalUp.toString(), isSpeed: true),
                        style: TextStyle(
                          color: colors.uploadSpeedColor,
                          fontSize: 11,
                          fontWeight: FontWeight.bold,
                          fontFamily: 'monospace',
                        ),
                      ),
                    ],
                  );
                },
              ),
            ],
          ),
        );
      },
    );
  }

  void _showThemeDialog() {
    final colors = context.appColors;
    final themesBloc = context.read<ThemesBloc>();

    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: colors.popupMenuBackgroundColor,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(10),
          side: BorderSide(color: colors.popupMenuBorderColor),
        ),
        title: Row(
          children: [
            Icon(Icons.palette_outlined, color: colors.titleBarLogoColor, size: 20),
            const SizedBox(width: 8),
            const Text('Select Theme', style: TextStyle(fontSize: 16)),
          ],
        ),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: ThemesBloc.availableThemes.map((theme) {
            final isSelected = theme.themeCode == colors.themeCode;
            return ListTile(
              dense: true,
              leading: Icon(
                isSelected ? Icons.check_circle : Icons.circle_outlined,
                size: 18,
                color: isSelected ? colors.accentGreenColor : colors.tableSecondaryTextColor,
              ),
              title: Text(
                theme.themeName,
                style: TextStyle(
                  color: isSelected ? colors.tablePrimaryTextColor : colors.tableSecondaryTextColor,
                  fontSize: 14,
                  fontWeight: isSelected ? FontWeight.bold : FontWeight.normal,
                ),
              ),
              onTap: () {
                themesBloc.add(ChangeThemeEvent(theme));
                Navigator.of(ctx).pop();
              },
            );
          }).toList(),
        ),
      ),
    );
  }
}

class _FilterChipData {
  final String filterKey;
  final String label;
  final IconData icon;
  final int count;
  final Color color;

  _FilterChipData(this.filterKey, this.label, this.icon, this.count, this.color);
}
