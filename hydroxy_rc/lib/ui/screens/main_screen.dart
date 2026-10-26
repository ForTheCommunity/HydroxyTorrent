import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hydroxy_rc/configs/converter.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/routes/routes_name.dart';
import 'package:hydroxy_rc/state/themes_bloc/themes_bloc.dart';
import 'package:hydroxy_rc/state/torrents_bloc/torrents_bloc.dart';
import 'package:hydroxy_rc/themes/app_theme.dart';
import 'package:hydroxy_rc/ui/screens/main_content/models/torrent_structure_ext.dart';
import 'package:hydroxy_rc/ui/screens/main_content/torrent_dashboard_screen.dart';
import 'package:hydroxy_rc/ui/screens/main_content/widgets/torrent_categories_sidebar.dart';
import 'package:hydroxy_rc/ui/screens/mobile/mobile_main_screen.dart';
import 'package:hydroxy_rc/ui/widgets/log_viewer_dialog.dart';

class MainScreen extends StatefulWidget {
  const MainScreen({super.key});

  @override
  State<MainScreen> createState() => _MainScreenState();
}

class _MainScreenState extends State<MainScreen> {
  // Initial width of the left sidebar
  double _leftSidebarWidth = 220.0;

  // Set min/max bounds for dragging
  static const double _leftSidebarMinWidth = 100.0;
  static const double _leftSidebarMaxWidth = 500.0;

  String? _selectedCategoryFilter;
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';
  bool _isRegex = false;
  bool _hasRegexError = false;

  String _normalizeRegexPattern(String input) {
    var p = input.trim();
    if (p.startsWith('*') || p.startsWith('+') || p.startsWith('?')) {
      p = '.$p';
    }
    return p;
  }

  void _validateRegex() {
    if (!_isRegex || _searchQuery.isEmpty) {
      _hasRegexError = false;
      return;
    }
    try {
      RegExp(_normalizeRegexPattern(_searchQuery));
      _hasRegexError = false;
    } catch (_) {
      _hasRegexError = true;
    }
  }

  Timer? _statsTimer;
  SystemStatsModel? _systemStats;

  @override
  void initState() {
    super.initState();
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
    }
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        // Breakpoint threshold for Desktop / Mobile layout.
        // Also triggers mobile UI if running on native mobile platform (Android / iOS)
        // or when viewed on narrow screens like mobile browsers.
        final isMobile = constraints.maxWidth < 768 ||
            defaultTargetPlatform == TargetPlatform.android ||
            defaultTargetPlatform == TargetPlatform.iOS;

        if (isMobile) {
          return MobileMainScreen(
            initialFilter: _selectedCategoryFilter,
          );
        }

        return _buildDesktopLayout(context);
      },
    );
  }

  Widget _buildDesktopLayout(BuildContext context) {
    final colors = context.appColors;
    return Scaffold(
      backgroundColor: colors.scaffoldBackgroundColor,
      body: Column(
        children: [
          // Top Bar with Toolbar Buttons
          _buildTopBar(context),

          // Main Body Split
          Expanded(
            child: Row(
              children: [
                // Left Sidebar (Dynamic Width)
                SizedBox(
                  width: _leftSidebarWidth,
                  child: TorrentCategoriesSidebar(
                    selectedFilter: _selectedCategoryFilter,
                    onFilterSelected: (filter) {
                      setState(() {
                        _selectedCategoryFilter = filter;
                      });
                    },
                  ),
                ),

                // Resizable Divider Handle
                MouseRegion(
                  cursor: SystemMouseCursors.resizeColumn,
                  child: GestureDetector(
                    behavior: HitTestBehavior.opaque,
                    onHorizontalDragUpdate: (details) {
                      setState(() {
                        _leftSidebarWidth =
                            (_leftSidebarWidth + details.delta.dx).clamp(
                              _leftSidebarMinWidth,
                              _leftSidebarMaxWidth,
                            );
                      });
                    },
                    child: Container(
                      width: 6,
                      color: colors.sidebarBackgroundColor,
                      child: VerticalDivider(
                        width: 1,
                        thickness: 1,
                        color: colors.dividerColor,
                      ),
                    ),
                  ),
                ),

                // Right Main Content
                Expanded(
                  child: TorrentDashboardScreen(
                    filter: _selectedCategoryFilter,
                    searchQuery: _searchQuery,
                    isRegex: _isRegex && !_hasRegexError,
                  ),
                ),
              ],
            ),
          ),

          // Status Bar with Live Speed Metrics
          _buildStatusBar(context),
        ],
      ),
    );
  }

  Widget _buildStatusBar(BuildContext context) {
    return ValueListenableBuilder<DaemonConnectionState>(
      valueListenable: ApiClient.instance.connectionStateNotifier,
      builder: (context, connState, _) {
        final isOnline = connState == DaemonConnectionState.connected;
        final statusColor = connState == DaemonConnectionState.connected
            ? context.appColors.accentGreenColor
            : (connState == DaemonConnectionState.disconnected
                ? context.appColors.accentRedColor
                : context.appColors.accentAmberColor);
        final statusText = connState == DaemonConnectionState.connected
            ? 'Connected'
            : (connState == DaemonConnectionState.disconnected
                ? 'Disconnected'
                : 'Connecting...');

        return BlocBuilder<TorrentsBloc, TorrentsState>(
          builder: (context, state) {
            final torrents = isOnline
                ? state.maybeWhen(
                    loaded: (torrents) => torrents,
                    orElse: () => const <TorrentStructure>[],
                  )
                : const <TorrentStructure>[];

            int totalDownSpeed = 0;
            int totalUpSpeed = 0;
            for (final t in torrents) {
              totalDownSpeed += t.downloadSpeedBytes;
              totalUpSpeed += t.uploadSpeedBytes;
            }

            final downSpeedStr = formatBytesDynamic(
              totalDownSpeed.toString(),
              isSpeed: true,
            );
            final upSpeedStr = formatBytesDynamic(
              totalUpSpeed.toString(),
              isSpeed: true,
            );

            final colors = context.appColors;
            return Container(
              height: 22,
              width: double.infinity,
              decoration: BoxDecoration(
                color: colors.statusBarBackgroundColor,
                border: Border(
                  top: BorderSide(color: colors.statusBarBorderColor),
                ),
              ),
              padding: const EdgeInsets.symmetric(horizontal: 12),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  // Online Indicator
                  Container(
                    width: 7,
                    height: 7,
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: statusColor,
                    ),
                  ),
                  const SizedBox(width: 6),
                  Text(
                    statusText,
                    style: TextStyle(
                      color: colors.statusBarTextColor,
                      fontSize: 11,
                      fontWeight: FontWeight.w400,
                    ),
                  ),

                  if (isOnline && _systemStats != null) ...[
                    const SizedBox(width: 14),
                    VerticalDivider(
                      color: colors.statusBarBorderColor,
                      width: 1,
                      indent: 4,
                      endIndent: 4,
                    ),
                    const SizedBox(width: 14),

                    // Program (Daemon) RAM Usage
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(
                          Icons.memory,
                          size: 13,
                          color: colors.accentPurpleColor,
                        ),
                        const SizedBox(width: 5),
                        Text(
                          'App RAM: ${formatBytesDynamic(_systemStats!.processRssBytes.toString())}',
                          style: TextStyle(
                            color: colors.statusBarTextColor,
                            fontSize: 11,
                            fontFamily: 'monospace',
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(width: 14),

                    // System RAM Usage
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(
                          Icons.pie_chart_outline,
                          size: 13,
                          color: colors.accentBlueColor,
                        ),
                        const SizedBox(width: 5),
                        Text(
                          'Sys RAM: ${formatBytesDynamic(_systemStats!.systemUsedBytes.toString())} / ${formatBytesDynamic(_systemStats!.systemTotalBytes.toString())} (${(_systemStats!.systemMemoryUsagePercent * 100).toStringAsFixed(0)}%)',
                          style: TextStyle(
                            color: colors.statusBarTextColor,
                            fontSize: 11,
                            fontFamily: 'monospace',
                          ),
                        ),
                      ],
                    ),
                  ],

                  const Spacer(),

                  // Download Speed Metric
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(
                        Icons.arrow_downward,
                        size: 13,
                        color: colors.downloadSpeedColor,
                      ),
                      const SizedBox(width: 4),
                      Text(
                        downSpeedStr,
                        style: TextStyle(
                          color: colors.downloadSpeedColor,
                          fontSize: 11,
                          fontWeight: FontWeight.w500,
                          fontFamily: 'monospace',
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(width: 16),

                  // Upload Speed Metric
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(
                        Icons.arrow_upward,
                        size: 13,
                        color: colors.uploadSpeedColor,
                      ),
                      const SizedBox(width: 4),
                      Text(
                        upSpeedStr,
                        style: TextStyle(
                          color: colors.uploadSpeedColor,
                          fontSize: 11,
                          fontWeight: FontWeight.w500,
                          fontFamily: 'monospace',
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            );
          },
        );
      },
    );
  }

  Widget _buildTopBar(BuildContext context) {
    final colors = context.appColors;
    return Container(
      height: 32,
      decoration: BoxDecoration(
        color: colors.titleBarBackgroundColor,
        border: Border(
          bottom: BorderSide(color: colors.titleBarBorderColor, width: 1.0),
        ),
      ),
      padding: const EdgeInsets.symmetric(horizontal: 10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          // App Logo / Title (Placeholder until custom logo is created)
          Icon(
            Icons.image_not_supported_outlined,
            color: colors.titleBarLogoColor,
            size: 16,
          ),
          const SizedBox(width: 6),
          Text(
            'HydroxyTorrent',
            style: TextStyle(
              color: colors.titleBarTextColor,
              fontWeight: FontWeight.bold,
              fontSize: 12,
              letterSpacing: 0.5,
            ),
          ),

          const Spacer(),

          // Centered Search Bar
          Container(
            width: 270,
            height: 24,
            decoration: BoxDecoration(
              color: colors.searchBarBackgroundColor,
              borderRadius: BorderRadius.circular(4),
              border: Border.all(
                color: _hasRegexError
                    ? colors.searchBarErrorBorderColor
                    : (_isRegex
                        ? colors.searchBarActiveBorderColor
                        : colors.searchBarBorderColor),
                width: 0.8,
              ),
            ),
            child: Row(
              children: [
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 5),
                  child: Icon(
                    Icons.search,
                    size: 13,
                    color: colors.searchBarIconColor,
                  ),
                ),
                Expanded(
                  child: TextField(
                    controller: _searchController,
                    onChanged: (value) {
                      setState(() {
                        _searchQuery = value;
                        _validateRegex();
                      });
                    },
                    style: TextStyle(
                      color: _hasRegexError ? colors.searchBarErrorBorderColor : colors.searchBarTextColor,
                      fontSize: 11,
                    ),
                    cursorColor: colors.searchBarActiveBorderColor,
                    cursorHeight: 12,
                    decoration: InputDecoration(
                      isDense: true,
                      contentPadding: const EdgeInsets.symmetric(vertical: 4),
                      hintText: _isRegex ? 'Regex filter (e.g. ^[0-9]+)...' : 'Search torrents...',
                      hintStyle: TextStyle(color: colors.searchBarHintColor, fontSize: 11),
                      border: InputBorder.none,
                    ),
                  ),
                ),
                if (_searchQuery.isNotEmpty)
                  InkWell(
                    onTap: () {
                      _searchController.clear();
                      setState(() {
                        _searchQuery = '';
                        _hasRegexError = false;
                      });
                    },
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 3),
                      child: Icon(
                        Icons.close,
                        size: 12,
                        color: colors.searchBarIconColor,
                      ),
                    ),
                  ),
                // Regex Toggle Button
                Tooltip(
                  message: _isRegex
                      ? (_hasRegexError ? 'Invalid Regex pattern' : 'Regex filter active (.*)')
                      : 'Use Regular Expression (.*)',
                  child: InkWell(
                    onTap: () {
                      setState(() {
                        _isRegex = !_isRegex;
                        _validateRegex();
                      });
                    },
                    borderRadius: BorderRadius.circular(3),
                    child: Container(
                      margin: const EdgeInsets.symmetric(horizontal: 3),
                      padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
                      decoration: BoxDecoration(
                        color: _isRegex
                            ? (_hasRegexError
                                ? Color.fromRGBO(colors.searchBarErrorBorderColor.red, colors.searchBarErrorBorderColor.green, colors.searchBarErrorBorderColor.blue, 0.2)
                                : colors.searchBarRegexActiveBackgroundColor)
                            : Colors.transparent,
                        borderRadius: BorderRadius.circular(3),
                        border: Border.all(
                          color: _isRegex
                              ? (_hasRegexError
                                  ? colors.searchBarErrorBorderColor
                                  : colors.searchBarActiveBorderColor)
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
                          fontSize: 10,
                          fontWeight: FontWeight.bold,
                          fontFamily: 'monospace',
                        ),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),

          const Spacer(),

          // Theme Selector Menu
          _buildThemeSelector(context),
          const SizedBox(width: 4),

          // Toolbar Buttons on the right
          _buildToolbarButton(
            icon: Icons.terminal,
            label: 'Logs',
            tooltip: 'View Daemon Logs',
            textColor: colors.titleBarTextColor,
            iconColor: colors.titleBarIconColor,
            onTap: () => LogViewerDialog.show(context),
          ),
          const SizedBox(width: 6),
          _buildToolbarButton(
            icon: Icons.logout,
            label: 'Logout',
            tooltip: 'Log out of Web UI session',
            textColor: colors.titleBarTextColor,
            iconColor: colors.titleBarIconColor,
            onTap: () => _showLogoutDialog(context),
          ),
          const SizedBox(width: 6),
          _buildToolbarButton(
            icon: Icons.power_settings_new,
            label: 'Shutdown',
            tooltip: 'Shutdown Hydroxy Daemon process',
            textColor: colors.accentRedColor,
            iconColor: colors.accentRedColor,
            onTap: () => _showShutdownDialog(context),
          ),
        ],
      ),
    );
  }

  Widget _buildThemeSelector(BuildContext context) {
    final colors = context.appColors;
    final themesBloc = context.read<ThemesBloc>();
    return PopupMenuButton<AppTheme>(
      tooltip: 'Change Theme',
      color: colors.popupMenuBackgroundColor,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(6),
        side: BorderSide(color: colors.popupMenuBorderColor),
      ),
      initialValue: colors,
      onSelected: (theme) {
        themesBloc.add(ChangeThemeEvent(theme));
      },
      itemBuilder: (context) {
        return ThemesBloc.availableThemes.map((theme) {
          final isSelected = theme.themeCode == colors.themeCode;
          return PopupMenuItem<AppTheme>(
            value: theme,
            height: 32,
            child: Row(
              children: [
                Icon(
                  isSelected ? Icons.check_circle : Icons.circle_outlined,
                  size: 14,
                  color: isSelected ? colors.accentGreenColor : colors.tableSecondaryTextColor,
                ),
                const SizedBox(width: 8),
                Text(
                  theme.themeName,
                  style: TextStyle(
                    color: isSelected ? colors.tablePrimaryTextColor : colors.tableSecondaryTextColor,
                    fontSize: 12,
                    fontWeight: isSelected ? FontWeight.bold : FontWeight.normal,
                  ),
                ),
              ],
            ),
          );
        }).toList();
      },
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.palette_outlined, size: 14, color: colors.titleBarIconColor),
            const SizedBox(width: 4),
            Text(
              colors.themeName,
              style: TextStyle(
                color: colors.titleBarTextColor,
                fontSize: 11,
                fontWeight: FontWeight.w500,
              ),
            ),
            const SizedBox(width: 2),
            Icon(Icons.arrow_drop_down, size: 14, color: colors.titleBarIconColor),
          ],
        ),
      ),
    );
  }

  Widget _buildToolbarButton({
    required IconData icon,
    required String label,
    required String tooltip,
    required VoidCallback onTap,
    Color? iconColor,
    Color? textColor,
  }) {
    return Tooltip(
      message: tooltip,
      child: InkWell(
        borderRadius: BorderRadius.circular(4),
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 14, color: iconColor ?? Colors.white70),
              const SizedBox(width: 5),
              Text(
                label,
                style: TextStyle(
                  color: textColor ?? Colors.white70,
                  fontSize: 11,
                  fontWeight: FontWeight.w500,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  void _showLogoutDialog(BuildContext context) {
    final colors = context.appColors;
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: colors.popupMenuBackgroundColor,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(8),
          side: BorderSide(color: colors.popupMenuBorderColor),
        ),
        title: Row(
          children: [
            Icon(Icons.logout, color: colors.accentAmberColor, size: 20),
            const SizedBox(width: 10),
            Text(
              'Logout',
              style: TextStyle(
                color: colors.tablePrimaryTextColor,
                fontSize: 16,
                fontWeight: FontWeight.bold,
              ),
            ),
          ],
        ),
        content: Text(
          'Are you sure you want to log out of the HydroxyTorrent Web UI session?',
          style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: Text('Cancel', style: TextStyle(color: colors.tableSecondaryTextColor)),
          ),
          ElevatedButton(
            style: ElevatedButton.styleFrom(
              backgroundColor: colors.dataCardBackgroundColor,
              foregroundColor: colors.tablePrimaryTextColor,
            ),
            onPressed: () {
              Navigator.of(ctx).pop();
              ApiClient.instance.logout();
              if (context.mounted) {
                Navigator.of(context).pushNamedAndRemoveUntil(
                  RoutesName.initialScreen,
                  (route) => false,
                );
              }
            },
            child: const Text('Logout'),
          ),
        ],
      ),
    );
  }

  void _showShutdownDialog(BuildContext context) {
    final colors = context.appColors;
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: colors.popupMenuBackgroundColor,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(8),
          side: BorderSide(color: Color.fromRGBO(colors.accentRedColor.red, colors.accentRedColor.green, colors.accentRedColor.blue, 0.5)),
        ),
        title: Row(
          children: [
            Icon(Icons.power_settings_new, color: colors.accentRedColor, size: 20),
            const SizedBox(width: 10),
            Text(
              'Shutdown Daemon',
              style: TextStyle(
                color: colors.tablePrimaryTextColor,
                fontSize: 16,
                fontWeight: FontWeight.bold,
              ),
            ),
          ],
        ),
        content: Text(
          'Are you sure you want to shut down the HydroxyTorrent daemon?\n\nAll active torrent downloads, seeding, and connections will be terminated.',
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
              ScaffoldMessenger.of(context).showSnackBar(
                SnackBar(
                  content: Text(
                    'Sending shutdown command to daemon...',
                    style: TextStyle(color: colors.snackBarTextColor, fontSize: 13, fontWeight: FontWeight.w500),
                  ),
                  backgroundColor: colors.snackBarBackgroundColor,
                  behavior: SnackBarBehavior.floating,
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(8),
                    side: BorderSide(color: colors.popupMenuBorderColor),
                  ),
                  duration: const Duration(seconds: 2),
                ),
              );
              await shutdownDaemon();
              if (context.mounted) {
                final message = (kIsWeb || kIsWasm)
                    ? 'Daemon has been shut down. Restart hydroxy_daemon on your host to reconnect.'
                    : 'Daemon has been shut down.';
                Navigator.of(context).pushNamedAndRemoveUntil(
                  RoutesName.initialScreen,
                  (route) => false,
                  arguments: message,
                );
              }
            },
            child: const Text('Shutdown'),
          ),
        ],
      ),
    );
  }
}
