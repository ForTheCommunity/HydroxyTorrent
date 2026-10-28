import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';

enum LogLevel { all, error, warn, info, debug, trace, normal }

enum LogCategory { major, minor }

class _LogLine {
  final int number;
  final String text;
  final LogLevel level;

  _LogLine({
    required this.number,
    required this.text,
    required this.level,
  });

  static LogLevel detectLevel(String text) {
    final upper = text.toUpperCase();
    if (upper.contains('ERROR') ||
        upper.contains('FATAL') ||
        upper.contains('PANIC') ||
        upper.contains('FAIL')) {
      return LogLevel.error;
    }
    if (upper.contains('WARN')) {
      return LogLevel.warn;
    }
    if (upper.contains('INFO')) {
      return LogLevel.info;
    }
    if (upper.contains('DEBUG')) {
      return LogLevel.debug;
    }
    if (upper.contains('TRACE')) {
      return LogLevel.trace;
    }
    return LogLevel.normal;
  }
}

class LogViewerDialog extends StatefulWidget {
  const LogViewerDialog({super.key});

  static Future<void> show(BuildContext context) {
    return Navigator.of(context).push(
      MaterialPageRoute(
        fullscreenDialog: true,
        builder: (_) => const LogViewerDialog(),
      ),
    );
  }

  @override
  State<LogViewerDialog> createState() => _LogViewerDialogState();
}

class _LogViewerDialogState extends State<LogViewerDialog> {
  static const int _maxLiveMinorLines = 1000;

  final ScrollController _verticalController = ScrollController();
  final ScrollController _horizontalController = ScrollController();
  final TextEditingController _searchController = TextEditingController();

  late final String _subId;
  Timer? _timer;
  bool _isPolling = false;

  LogCategory _selectedCategory = LogCategory.major;

  // Major (Disk-backed) logs state
  String _rawMajorLogs = '';
  List<_LogLine> _majorLines = [];
  bool _isLoadingMajor = true;

  // Minor (Live Broadcast only, ephemeral) logs state
  final List<_LogLine> _minorLines = [];
  int _nextMinorLineNumber = 1;

  bool _autoScroll = true;
  bool _isSearchVisible = false;
  String _searchQuery = '';
  LogLevel _selectedLevelFilter = LogLevel.all;

  @override
  void initState() {
    super.initState();
    _subId =
        'rc_${DateTime.now().microsecondsSinceEpoch}_${math.Random().nextInt(999999)}';
    _startPollingForCurrentCategory();
  }

  @override
  void dispose() {
    _timer?.cancel();
    if (_selectedCategory == LogCategory.minor) {
      unsubscribeMinorLogs(subId: _subId);
    }
    _verticalController.dispose();
    _horizontalController.dispose();
    _searchController.dispose();
    super.dispose();
  }

  void _startPollingForCurrentCategory() {
    _timer?.cancel();
    if (_selectedCategory == LogCategory.major) {
      _fetchMajorLogs();
      _timer = Timer.periodic(const Duration(seconds: 2), (_) {
        if (mounted) {
          _fetchMajorLogs(isAuto: true);
        }
      });
    } else {
      _pollMinorLogs();
      _timer = Timer.periodic(const Duration(seconds: 1), (_) {
        if (mounted) {
          _pollMinorLogs();
        }
      });
    }
  }

  void _switchCategory(LogCategory category) {
    if (_selectedCategory == category) return;
    final previous = _selectedCategory;
    setState(() {
      _selectedCategory = category;
    });
    if (previous == LogCategory.minor) {
      unsubscribeMinorLogs(subId: _subId);
    }
    _startPollingForCurrentCategory();
    if (_autoScroll) {
      _scrollToBottom();
    }
  }

  void _scrollToBottom() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_verticalController.hasClients) {
        _verticalController.jumpTo(
          _verticalController.position.maxScrollExtent,
        );
      }
    });
  }

  Future<void> _fetchMajorLogs({bool isAuto = false}) async {
    if (_isPolling) return;
    _isPolling = true;
    try {
      final logs = await getDaemonLogs();
      if (mounted && _selectedCategory == LogCategory.major) {
        final shouldScroll = _autoScroll && (isAuto || _isLoadingMajor);
        final trimmed = logs.trim();
        final raw = trimmed.isEmpty ? 'No major events recorded yet.' : logs;

        final rawLines = raw.split('\n');
        final parsed = <_LogLine>[];
        for (int i = 0; i < rawLines.length; i++) {
          final line = rawLines[i];
          parsed.add(
            _LogLine(
              number: i + 1,
              text: line,
              level: _LogLine.detectLevel(line),
            ),
          );
        }

        setState(() {
          _rawMajorLogs = raw;
          _majorLines = parsed;
          _isLoadingMajor = false;
        });

        if (shouldScroll) {
          _scrollToBottom();
        }
      }
    } catch (e) {
      if (mounted && !_isLoadingMajor) {
        // Silently keep existing logs on polling failure
      } else if (mounted) {
        final errText =
            'Failed to fetch major logs: $e\nEnsure the daemon is running.';
        setState(() {
          _rawMajorLogs = errText;
          _majorLines = [
            _LogLine(number: 1, text: errText, level: LogLevel.error),
          ];
          _isLoadingMajor = false;
        });
      }
    } finally {
      _isPolling = false;
    }
  }

  Future<void> _pollMinorLogs() async {
    if (_isPolling) return;
    _isPolling = true;
    try {
      final incoming = await getMinorLogs(subId: _subId);
      if (mounted &&
          _selectedCategory == LogCategory.minor &&
          incoming.isNotEmpty) {
        setState(() {
          for (final text in incoming) {
            for (final subLine in text.split('\n')) {
              if (subLine.trim().isEmpty) continue;
              _minorLines.add(
                _LogLine(
                  number: _nextMinorLineNumber++,
                  text: subLine,
                  level: _LogLine.detectLevel(subLine),
                ),
              );
            }
          }
          if (_minorLines.length > _maxLiveMinorLines) {
            _minorLines.removeRange(
              0,
              _minorLines.length - _maxLiveMinorLines,
            );
          }
        });

        if (_autoScroll) {
          _scrollToBottom();
        }
      }
    } catch (_) {
      // Ignore transient poll errors on live stream
    } finally {
      _isPolling = false;
    }
  }

  List<_LogLine> get _activeLines =>
      _selectedCategory == LogCategory.major ? _majorLines : _minorLines;

  String get _activeRawText {
    if (_selectedCategory == LogCategory.major) {
      return _rawMajorLogs;
    }
    return _minorLines.map((l) => l.text).join('\n');
  }

  List<_LogLine> _getFilteredLines() {
    return _activeLines.where((line) {
      if (_selectedLevelFilter != LogLevel.all &&
          line.level != _selectedLevelFilter) {
        return false;
      }
      if (_searchQuery.isNotEmpty) {
        return line.text.toLowerCase().contains(_searchQuery.toLowerCase());
      }
      return true;
    }).toList();
  }

  List<TextSpan> _buildLineSpans(_LogLine line, int padWidth) {
    final spans = <TextSpan>[
      // Line number gutter
      TextSpan(
        text: '${line.number.toString().padLeft(padWidth)} | ',
        style: const TextStyle(
          color: Color(0xFF6E7681),
          fontFamily: 'monospace',
          fontSize: 12,
        ),
      ),
    ];

    final text = line.text;
    final level = line.level;

    final Color badgeColor;
    final Color messageColor;
    switch (level) {
      case LogLevel.error:
        badgeColor = const Color(0xFFFF5555);
        messageColor = const Color(0xFFFF7B72);
        break;
      case LogLevel.warn:
        badgeColor = const Color(0xFFFFCC00);
        messageColor = const Color(0xFFE3B341);
        break;
      case LogLevel.info:
        badgeColor = const Color(0xFF56D364);
        messageColor = const Color(0xFF85E89D);
        break;
      case LogLevel.debug:
        badgeColor = const Color(0xFF58A6FF);
        messageColor = const Color(0xFF79C0FF);
        break;
      case LogLevel.trace:
        badgeColor = const Color(0xFFBC8CFF);
        messageColor = const Color(0xFFD2A8FF);
        break;
      case LogLevel.normal:
      case LogLevel.all:
        badgeColor = const Color(0xFFC9D1D9);
        messageColor = const Color(0xFFC9D1D9);
        break;
    }

    if (level == LogLevel.normal || level == LogLevel.all) {
      spans.add(
        TextSpan(
          text: text,
          style: TextStyle(
            color: messageColor,
            fontFamily: 'monospace',
            fontSize: 12,
          ),
        ),
      );
      return spans;
    }

    final regExp = RegExp(
      r'(\[?(?:ERROR|FATAL|PANIC|WARN(?:ING)?|INFO|DEBUG|TRACE)\]?:?)',
      caseSensitive: false,
    );
    final match = regExp.firstMatch(text);

    if (match != null) {
      final before = text.substring(0, match.start);
      final badge = text.substring(match.start, match.end);
      final after = text.substring(match.end);

      if (before.isNotEmpty) {
        spans.add(
          TextSpan(
            text: before,
            style: const TextStyle(
              color: Color(0xFF8B949E),
              fontFamily: 'monospace',
              fontSize: 12,
            ),
          ),
        );
      }

      spans.add(
        TextSpan(
          text: badge,
          style: TextStyle(
            color: badgeColor,
            fontWeight: FontWeight.bold,
            fontFamily: 'monospace',
            fontSize: 12,
          ),
        ),
      );

      if (after.isNotEmpty) {
        spans.add(
          TextSpan(
            text: after,
            style: TextStyle(
              color: messageColor,
              fontFamily: 'monospace',
              fontSize: 12,
            ),
          ),
        );
      }
    } else {
      spans.add(
        TextSpan(
          text: text,
          style: TextStyle(
            color: messageColor,
            fontFamily: 'monospace',
            fontSize: 12,
          ),
        ),
      );
    }

    return spans;
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    final activeLines = _activeLines;
    final filteredLines = _getFilteredLines();
    final maxLineNumber = activeLines.isEmpty ? 1 : activeLines.last.number;
    final padWidth = math.max(2, maxLineNumber.toString().length);

    final errorCount =
        activeLines.where((l) => l.level == LogLevel.error).length;
    final warnCount =
        activeLines.where((l) => l.level == LogLevel.warn).length;
    final infoCount =
        activeLines.where((l) => l.level == LogLevel.info).length;

    final isMinor = _selectedCategory == LogCategory.minor;

    return Scaffold(
      backgroundColor: const Color(0xFF0D1117),
      appBar: AppBar(
        backgroundColor: const Color(0xFF161B22),
        elevation: 0,
        titleSpacing: 0,
        leading: IconButton(
          icon: const Icon(Icons.close_rounded, color: Colors.white),
          tooltip: 'Close Logs',
          onPressed: () => Navigator.of(context).pop(),
        ),
        title: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              isMinor ? Icons.sensors_rounded : Icons.terminal_rounded,
              color: isMinor
                  ? const Color(0xFF58A6FF)
                  : const Color(0xFF56D364),
              size: 18,
            ),
            const SizedBox(width: 6),
            const Text(
              'Daemon Logs',
              style: TextStyle(
                color: Colors.white,
                fontWeight: FontWeight.bold,
                fontSize: 15,
              ),
            ),
            const SizedBox(width: 8),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
              decoration: BoxDecoration(
                color: const Color(0xFF21262D),
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: const Color(0xFF30363D), width: 0.8),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    isMinor
                        ? Icons.fiber_manual_record
                        : Icons.save_outlined,
                    size: 8,
                    color: isMinor
                        ? const Color(0xFF58A6FF)
                        : const Color(0xFF56D364),
                  ),
                  const SizedBox(width: 4),
                  Text(
                    isMinor ? 'Live Logs' : 'Saved Logs',
                    style: const TextStyle(color: Colors.white70, fontSize: 9.5),
                  ),
                ],
              ),
            ),
          ],
        ),
        actions: [
          if (isMinor && _minorLines.isNotEmpty)
            IconButton(
              tooltip: 'Clear Live Minor Logs',
              visualDensity: VisualDensity.compact,
              padding: const EdgeInsets.symmetric(horizontal: 4),
              icon: const Icon(
                Icons.delete_sweep_rounded,
                size: 20,
                color: Colors.white70,
              ),
              onPressed: () {
                setState(() {
                  _minorLines.clear();
                  _nextMinorLineNumber = 1;
                });
              },
            ),

          // Search Toggle
          IconButton(
            tooltip: _isSearchVisible ? 'Close Search' : 'Search Logs',
            visualDensity: VisualDensity.compact,
            padding: const EdgeInsets.symmetric(horizontal: 4),
            icon: Icon(
              _isSearchVisible
                  ? Icons.search_off_rounded
                  : Icons.search_rounded,
              size: 20,
              color:
                  _isSearchVisible ? const Color(0xFF56D364) : Colors.white70,
            ),
            onPressed: () {
              setState(() {
                _isSearchVisible = !_isSearchVisible;
                if (!_isSearchVisible) {
                  _searchController.clear();
                  _searchQuery = '';
                }
              });
            },
          ),

          // Auto-scroll toggle
          IconButton(
            tooltip: _autoScroll ? 'Auto-scroll on' : 'Auto-scroll off',
            visualDensity: VisualDensity.compact,
            padding: const EdgeInsets.symmetric(horizontal: 4),
            icon: Icon(
              _autoScroll
                  ? Icons.vertical_align_bottom_rounded
                  : Icons.pause_rounded,
              size: 20,
              color: _autoScroll ? const Color(0xFF56D364) : Colors.white38,
            ),
            onPressed: () {
              setState(() {
                _autoScroll = !_autoScroll;
              });
            },
          ),

          // Copy All Logs
          IconButton(
            tooltip: 'Copy all logs',
            visualDensity: VisualDensity.compact,
            padding: const EdgeInsets.symmetric(horizontal: 6),
            icon: const Icon(
              Icons.copy_rounded,
              size: 18,
              color: Colors.white70,
            ),
            onPressed: () {
              Clipboard.setData(ClipboardData(text: _activeRawText));
              ScaffoldMessenger.of(context).showSnackBar(
                SnackBar(
                  content: Text(
                    'Logs copied to clipboard',
                    style: TextStyle(
                      color: colors.snackBarTextColor,
                      fontSize: 13,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  backgroundColor: colors.snackBarBackgroundColor,
                  duration: const Duration(seconds: 2),
                  behavior: SnackBarBehavior.floating,
                ),
              );
            },
          ),
          const SizedBox(width: 4),
        ],
      ),
      body: SafeArea(
        child: Column(
          children: [
            // Log Type Selector Bar (Major Events vs Minor Events)
            Container(
              color: const Color(0xFF161B22),
              padding: const EdgeInsets.fromLTRB(10, 4, 10, 6),
              child: Row(
                children: [
                  Expanded(
                    child: _buildCategoryTab(
                      title: 'Major Events (Saved)',
                      subtitle: 'Torrent added, found, completed, removed',
                      icon: Icons.event_note_rounded,
                      category: LogCategory.major,
                      activeColor: const Color(0xFF56D364),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: _buildCategoryTab(
                      title: 'Minor Events (Live Broadcast)',
                      subtitle:
                          'Piece I/O, tracker announce, PEX, peer timeouts',
                      icon: Icons.sensors_rounded,
                      category: LogCategory.minor,
                      activeColor: const Color(0xFF58A6FF),
                    ),
                  ),
                ],
              ),
            ),

            // Search Bar if open
            if (_isSearchVisible)
              Container(
                color: const Color(0xFF161B22),
                padding: const EdgeInsets.fromLTRB(12, 0, 12, 8),
                child: SizedBox(
                  height: 36,
                  child: TextField(
                    controller: _searchController,
                    autofocus: true,
                    style: const TextStyle(color: Colors.white, fontSize: 13),
                    decoration: InputDecoration(
                      hintText: 'Filter log output...',
                      hintStyle:
                          const TextStyle(color: Colors.white38, fontSize: 13),
                      contentPadding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 0,
                      ),
                      isDense: true,
                      prefixIcon: const Icon(
                        Icons.search,
                        size: 16,
                        color: Colors.white54,
                      ),
                      suffixIcon: _searchQuery.isNotEmpty
                          ? IconButton(
                              icon: const Icon(
                                Icons.clear,
                                size: 16,
                                color: Colors.white54,
                              ),
                              onPressed: () {
                                _searchController.clear();
                                setState(() => _searchQuery = '');
                              },
                            )
                          : null,
                      filled: true,
                      fillColor: const Color(0xFF0D1117),
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(6),
                        borderSide:
                            const BorderSide(color: Color(0xFF30363D)),
                      ),
                      enabledBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(6),
                        borderSide:
                            const BorderSide(color: Color(0xFF30363D)),
                      ),
                      focusedBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(6),
                        borderSide:
                            const BorderSide(color: Color(0xFF56D364)),
                      ),
                    ),
                    onChanged: (val) {
                      setState(() => _searchQuery = val);
                    },
                  ),
                ),
              ),

            // Level Filter Chips Row
            Container(
              height: 38,
              color: const Color(0xFF161B22),
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
              child: ListView(
                scrollDirection: Axis.horizontal,
                children: [
                  _buildFilterChip(
                    'All (${activeLines.length})',
                    LogLevel.all,
                    const Color(0xFF58A6FF),
                  ),
                  const SizedBox(width: 6),
                  _buildFilterChip(
                    'Error ($errorCount)',
                    LogLevel.error,
                    const Color(0xFFFF5555),
                  ),
                  const SizedBox(width: 6),
                  _buildFilterChip(
                    'Warn ($warnCount)',
                    LogLevel.warn,
                    const Color(0xFFFFCC00),
                  ),
                  const SizedBox(width: 6),
                  _buildFilterChip(
                    'Info ($infoCount)',
                    LogLevel.info,
                    const Color(0xFF56D364),
                  ),
                ],
              ),
            ),
            const Divider(color: Color(0xFF30363D), height: 1),

            // Main Log Terminal
            Expanded(
              child: (_selectedCategory == LogCategory.major && _isLoadingMajor)
                  ? const Center(
                      child: SizedBox(
                        width: 28,
                        height: 28,
                        child: CircularProgressIndicator(
                          strokeWidth: 2,
                          color: Color(0xFF56D364),
                        ),
                      ),
                    )
                  : filteredLines.isEmpty
                      ? Center(
                          child: Padding(
                            padding: const EdgeInsets.all(24),
                            child: Column(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                Icon(
                                  isMinor
                                      ? Icons.sensors_rounded
                                      : Icons.event_note_rounded,
                                  size: 32,
                                  color: const Color(0xFF484F58),
                                ),
                                const SizedBox(height: 10),
                                Text(
                                  _searchQuery.isNotEmpty ||
                                          _selectedLevelFilter != LogLevel.all
                                      ? 'No logs match current filter.'
                                      : isMinor
                                          ? 'Listening for live minor events...\n(Events are broadcast live while watching and are never saved to disk or memory.)'
                                          : 'No major logs recorded yet.',
                                  textAlign: TextAlign.center,
                                  style: const TextStyle(
                                    color: Color(0xFF6E7681),
                                    fontSize: 13,
                                    height: 1.5,
                                  ),
                                ),
                              ],
                            ),
                          ),
                        )
                      : Scrollbar(
                          controller: _verticalController,
                          thumbVisibility: true,
                          child: Scrollbar(
                            controller: _horizontalController,
                            thumbVisibility: true,
                            notificationPredicate: (n) => n.depth == 1,
                            child: SingleChildScrollView(
                              controller: _verticalController,
                              padding: const EdgeInsets.all(12),
                              child: SingleChildScrollView(
                                controller: _horizontalController,
                                scrollDirection: Axis.horizontal,
                                child: SelectableText.rich(
                                  TextSpan(
                                    children: [
                                      for (final line in filteredLines) ...[
                                        ..._buildLineSpans(line, padWidth),
                                        const TextSpan(text: '\n'),
                                      ],
                                    ],
                                  ),
                                  style: const TextStyle(
                                    height: 1.45,
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildCategoryTab({
    required String title,
    required String subtitle,
    required IconData icon,
    required LogCategory category,
    required Color activeColor,
  }) {
    final isSelected = _selectedCategory == category;
    return InkWell(
      borderRadius: BorderRadius.circular(8),
      onTap: () => _switchCategory(category),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
        decoration: BoxDecoration(
          color: isSelected
              ? activeColor.withValues(alpha: 0.14)
              : const Color(0xFF0D1117),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(
            color: isSelected ? activeColor : const Color(0xFF30363D),
            width: isSelected ? 1.2 : 0.8,
          ),
        ),
        child: Row(
          children: [
            Icon(
              icon,
              size: 16,
              color: isSelected ? activeColor : const Color(0xFF8B949E),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: isSelected ? Colors.white : const Color(0xFFC9D1D9),
                      fontSize: 12,
                      fontWeight:
                          isSelected ? FontWeight.bold : FontWeight.w500,
                    ),
                  ),
                  Text(
                    subtitle,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      color: Color(0xFF8B949E),
                      fontSize: 10,
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildFilterChip(String label, LogLevel level, Color activeColor) {
    final isSelected = _selectedLevelFilter == level;
    return InkWell(
      borderRadius: BorderRadius.circular(14),
      onTap: () {
        setState(() {
          _selectedLevelFilter = level;
        });
      },
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 3),
        decoration: BoxDecoration(
          color: isSelected
              ? activeColor.withValues(alpha: 0.2)
              : const Color(0xFF21262D),
          borderRadius: BorderRadius.circular(14),
          border: Border.all(
            color: isSelected ? activeColor : const Color(0xFF30363D),
            width: isSelected ? 1.2 : 0.8,
          ),
        ),
        child: Text(
          label,
          style: TextStyle(
            color: isSelected ? activeColor : const Color(0xFF8B949E),
            fontSize: 11,
            fontWeight: isSelected ? FontWeight.bold : FontWeight.w500,
          ),
        ),
      ),
    );
  }
}
