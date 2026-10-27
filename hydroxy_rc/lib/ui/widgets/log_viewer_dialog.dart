import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';

enum LogLevel { all, error, warn, info, debug, trace, normal }

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
  final ScrollController _verticalController = ScrollController();
  final ScrollController _horizontalController = ScrollController();
  final TextEditingController _searchController = TextEditingController();

  Timer? _timer;
  String _rawLogs = '';
  List<_LogLine> _parsedLines = [];
  bool _isLoading = true;
  bool _autoScroll = true;

  bool _isSearchVisible = false;
  String _searchQuery = '';
  LogLevel _selectedLevelFilter = LogLevel.all;

  @override
  void initState() {
    super.initState();
    _fetchLogs();
    _timer = Timer.periodic(const Duration(seconds: 2), (_) {
      if (mounted) {
        _fetchLogs(isAuto: true);
      }
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    _verticalController.dispose();
    _horizontalController.dispose();
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _fetchLogs({bool isAuto = false}) async {
    try {
      final logs = await getDaemonLogs();
      if (mounted) {
        final shouldScroll = _autoScroll && (isAuto || _isLoading);
        final trimmed = logs.trim();
        final raw = trimmed.isEmpty ? 'No logs recorded yet.' : logs;

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
          _rawLogs = raw;
          _parsedLines = parsed;
          _isLoading = false;
        });

        if (shouldScroll) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (_verticalController.hasClients) {
              _verticalController.jumpTo(
                _verticalController.position.maxScrollExtent,
              );
            }
          });
        }
      }
    } catch (e) {
      if (mounted && !_isLoading) {
        // Silently keep existing logs on polling failure
      } else if (mounted) {
        final errText = 'Failed to fetch logs: $e\nEnsure the daemon is running.';
        setState(() {
          _rawLogs = errText;
          _parsedLines = [
            _LogLine(number: 1, text: errText, level: LogLevel.error),
          ];
          _isLoading = false;
        });
      }
    }
  }

  List<_LogLine> _getFilteredLines() {
    return _parsedLines.where((line) {
      if (_selectedLevelFilter != LogLevel.all && line.level != _selectedLevelFilter) {
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
    final filteredLines = _getFilteredLines();
    final maxLineNumber = _parsedLines.isEmpty ? 1 : _parsedLines.last.number;
    final padWidth = math.max(2, maxLineNumber.toString().length);

    final errorCount = _parsedLines.where((l) => l.level == LogLevel.error).length;
    final warnCount = _parsedLines.where((l) => l.level == LogLevel.warn).length;
    final infoCount = _parsedLines.where((l) => l.level == LogLevel.info).length;

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
            const Icon(Icons.terminal_rounded, color: Color(0xFF56D364), size: 18),
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
              padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1.5),
              decoration: BoxDecoration(
                color: const Color(0xFF21262D),
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: const Color(0xFF30363D), width: 0.8),
              ),
              child: const Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    Icons.fiber_manual_record,
                    size: 7,
                    color: Color(0xFF56D364),
                  ),
                  SizedBox(width: 3),
                  Text(
                    'Live',
                    style: TextStyle(color: Colors.white70, fontSize: 9),
                  ),
                ],
              ),
            ),
          ],
        ),
        actions: [
          // Search Toggle
          IconButton(
            tooltip: _isSearchVisible ? 'Close Search' : 'Search Logs',
            visualDensity: VisualDensity.compact,
            padding: const EdgeInsets.symmetric(horizontal: 4),
            icon: Icon(
              _isSearchVisible ? Icons.search_off_rounded : Icons.search_rounded,
              size: 20,
              color: _isSearchVisible ? const Color(0xFF56D364) : Colors.white70,
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
              _autoScroll ? Icons.vertical_align_bottom_rounded : Icons.pause_rounded,
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
            icon: const Icon(Icons.copy_rounded, size: 18, color: Colors.white70),
            onPressed: () {
              Clipboard.setData(ClipboardData(text: _rawLogs));
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
                      hintStyle: const TextStyle(color: Colors.white38, fontSize: 13),
                      contentPadding: const EdgeInsets.symmetric(horizontal: 10, vertical: 0),
                      isDense: true,
                      prefixIcon: const Icon(Icons.search, size: 16, color: Colors.white54),
                      suffixIcon: _searchQuery.isNotEmpty
                          ? IconButton(
                              icon: const Icon(Icons.clear, size: 16, color: Colors.white54),
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
                        borderSide: const BorderSide(color: Color(0xFF30363D)),
                      ),
                      enabledBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(6),
                        borderSide: const BorderSide(color: Color(0xFF30363D)),
                      ),
                      focusedBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(6),
                        borderSide: const BorderSide(color: Color(0xFF56D364)),
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
                  _buildFilterChip('All (${_parsedLines.length})', LogLevel.all, const Color(0xFF58A6FF)),
                  const SizedBox(width: 6),
                  _buildFilterChip('Error ($errorCount)', LogLevel.error, const Color(0xFFFF5555)),
                  const SizedBox(width: 6),
                  _buildFilterChip('Warn ($warnCount)', LogLevel.warn, const Color(0xFFFFCC00)),
                  const SizedBox(width: 6),
                  _buildFilterChip('Info ($infoCount)', LogLevel.info, const Color(0xFF56D364)),
                ],
              ),
            ),
            const Divider(color: Color(0xFF30363D), height: 1),

            // Main Log Terminal
            Expanded(
              child: _isLoading
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
                          child: Text(
                            _searchQuery.isNotEmpty || _selectedLevelFilter != LogLevel.all
                                ? 'No logs match current filter.'
                                : 'No logs recorded yet.',
                            style: const TextStyle(color: Color(0xFF6E7681), fontSize: 13),
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
