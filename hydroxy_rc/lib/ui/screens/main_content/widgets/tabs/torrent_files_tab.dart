import 'dart:async';
import 'package:flutter/material.dart';
import 'package:hydroxy_rc/configs/converter.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/network/models.dart';

class TorrentFilesTab extends StatefulWidget {
  final String infoHash;
  final String torrentName;

  const TorrentFilesTab({
    super.key,
    required this.infoHash,
    required this.torrentName,
  });

  @override
  State<TorrentFilesTab> createState() => _TorrentFilesTabState();
}

class _TorrentFilesTabState extends State<TorrentFilesTab> {
  Timer? _pollingTimer;
  List<TorrentFileItem>? _files;
  bool _isLoading = true;
  String? _error;

  final TextEditingController _searchController = TextEditingController();
  String _searchFilter = '';

  int? _sortColumnIndex = 0; // Default sort by File Name
  bool _isAscending = true;

  @override
  void initState() {
    super.initState();
    _fetchFiles();
    _pollingTimer = Timer.periodic(const Duration(seconds: 2), (_) {
      if (mounted) {
        _fetchFiles(isPolling: true);
      }
    });
  }

  @override
  void didUpdateWidget(covariant TorrentFilesTab oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.infoHash != widget.infoHash) {
      setState(() {
        _files = null;
        _isLoading = true;
        _error = null;
        _searchFilter = '';
        _searchController.clear();
      });
      _fetchFiles();
    }
  }

  @override
  void dispose() {
    _pollingTimer?.cancel();
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _fetchFiles({bool isPolling = false}) async {
    try {
      final detail = await getTorrentDetail(infoHash: widget.infoHash);
      if (mounted) {
        setState(() {
          _files = detail.files;
          _isLoading = false;
          _error = null;
        });
      }
    } catch (e) {
      if (mounted && !isPolling) {
        setState(() {
          _error = e.toString();
          _isLoading = false;
        });
      }
    }
  }

  void _onSort(int columnIndex, bool ascending) {
    setState(() {
      _sortColumnIndex = columnIndex;
      _isAscending = ascending;
    });
  }

  List<TorrentFileItem> _getFilteredAndSortedFiles(List<TorrentFileItem> list) {
    var filtered = list;
    if (_searchFilter.isNotEmpty) {
      final query = _searchFilter.toLowerCase();
      filtered = list.where((f) => f.name.toLowerCase().contains(query)).toList();
    }

    if (_sortColumnIndex == null) return filtered;

    final sorted = List<TorrentFileItem>.from(filtered);
    sorted.sort((a, b) {
      final int cmp;
      switch (_sortColumnIndex) {
        case 0: // File Name
          cmp = a.name.toLowerCase().compareTo(b.name.toLowerCase());
          break;
        case 1: // Size
          cmp = a.size.compareTo(b.size);
          break;
        case 2: // Progress
          cmp = a.progress.compareTo(b.progress);
          break;
        case 3: // Priority
          cmp = a.priority.toLowerCase().compareTo(b.priority.toLowerCase());
          break;
        default:
          cmp = 0;
      }
      return _isAscending ? cmp : -cmp;
    });
    return sorted;
  }

  IconData _getFileIcon(String fileName) {
    final lower = fileName.toLowerCase();
    if (lower.endsWith('.mp4') ||
        lower.endsWith('.mkv') ||
        lower.endsWith('.avi') ||
        lower.endsWith('.mov') ||
        lower.endsWith('.webm')) {
      return Icons.movie_outlined;
    } else if (lower.endsWith('.mp3') ||
        lower.endsWith('.flac') ||
        lower.endsWith('.wav') ||
        lower.endsWith('.ogg') ||
        lower.endsWith('.m4a')) {
      return Icons.music_note_outlined;
    } else if (lower.endsWith('.png') ||
        lower.endsWith('.jpg') ||
        lower.endsWith('.jpeg') ||
        lower.endsWith('.gif') ||
        lower.endsWith('.webp')) {
      return Icons.image_outlined;
    } else if (lower.endsWith('.zip') ||
        lower.endsWith('.rar') ||
        lower.endsWith('.tar') ||
        lower.endsWith('.gz') ||
        lower.endsWith('.7z') ||
        lower.endsWith('.iso')) {
      return Icons.archive_outlined;
    } else if (lower.endsWith('.pdf') ||
        lower.endsWith('.doc') ||
        lower.endsWith('.docx') ||
        lower.endsWith('.txt') ||
        lower.endsWith('.epub')) {
      return Icons.description_outlined;
    } else if (lower.endsWith('.exe') ||
        lower.endsWith('.bin') ||
        lower.endsWith('.sh') ||
        lower.endsWith('.appimage') ||
        lower.endsWith('.deb')) {
      return Icons.terminal_outlined;
    }
    return Icons.insert_drive_file_outlined;
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;

    if (_isLoading && _files == null) {
      return Center(
        child: SizedBox(
          width: 24,
          height: 24,
          child: CircularProgressIndicator(
            strokeWidth: 2,
            color: colors.accentGreenColor,
          ),
        ),
      );
    }

    if (_error != null && _files == null) {
      return Center(
        child: Text(
          'Error loading files: $_error',
          style: TextStyle(color: colors.accentRedColor, fontSize: 11),
        ),
      );
    }

    final rawFiles = _files ?? [];
    if (rawFiles.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.folder_open, size: 32, color: colors.tableSecondaryTextColor.withOpacity(0.4)),
            const SizedBox(height: 8),
            Text(
              'No files found for "${widget.torrentName}"',
              style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 12),
            ),
          ],
        ),
      );
    }

    final filteredFiles = _getFilteredAndSortedFiles(rawFiles);
    final totalBytes = rawFiles.fold<int>(0, (sum, f) => sum + f.size);

    final headerStyle = TextStyle(
      color: colors.tableHeaderTextColor,
      fontSize: 11,
      fontWeight: FontWeight.bold,
    );
    final cellStyle = TextStyle(
      color: colors.tableAccentTextColor,
      fontSize: 11,
    );
    final fileNameStyle = TextStyle(
      color: colors.tablePrimaryTextColor,
      fontSize: 11,
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        // Summary & Search Toolbar
        Container(
          padding: const EdgeInsets.only(bottom: 8),
          child: Row(
            children: [
              Text(
                '${rawFiles.length} file${rawFiles.length == 1 ? '' : 's'} (${formatBytesDynamic(totalBytes.toString())})',
                style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 11),
              ),
              const Spacer(),
              SizedBox(
                width: 180,
                height: 26,
                child: TextField(
                  controller: _searchController,
                  style: TextStyle(color: colors.searchBarTextColor, fontSize: 11),
                  decoration: InputDecoration(
                    hintText: 'Filter files...',
                    hintStyle: TextStyle(color: colors.searchBarHintColor, fontSize: 11),
                    contentPadding: const EdgeInsets.symmetric(horizontal: 8, vertical: 0),
                    isDense: true,
                    prefixIcon: Icon(Icons.search, size: 14, color: colors.searchBarIconColor),
                    prefixIconConstraints: const BoxConstraints(minWidth: 24, minHeight: 14),
                    suffixIcon: _searchFilter.isNotEmpty
                        ? InkWell(
                            onTap: () {
                              _searchController.clear();
                              setState(() {
                                _searchFilter = '';
                              });
                            },
                            child: Icon(Icons.clear, size: 14, color: colors.searchBarIconColor),
                          )
                        : null,
                    suffixIconConstraints: const BoxConstraints(minWidth: 20, minHeight: 14),
                    filled: true,
                    fillColor: colors.searchBarBackgroundColor,
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(4),
                      borderSide: BorderSide(color: colors.searchBarBorderColor),
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(4),
                      borderSide: BorderSide(color: colors.searchBarActiveBorderColor),
                    ),
                  ),
                  onChanged: (val) {
                    setState(() {
                      _searchFilter = val;
                    });
                  },
                ),
              ),
            ],
          ),
        ),

        // Files Table
        Expanded(
          child: LayoutBuilder(
            builder: (context, constraints) {
              return SingleChildScrollView(
                scrollDirection: Axis.vertical,
                child: SingleChildScrollView(
                  scrollDirection: Axis.horizontal,
                  child: ConstrainedBox(
                    constraints: BoxConstraints(minWidth: constraints.maxWidth),
                    child: DataTable(
                      sortColumnIndex: _sortColumnIndex,
                      sortAscending: _isAscending,
                      headingRowHeight: 28,
                      dataRowMinHeight: 26,
                      dataRowMaxHeight: 28,
                      columnSpacing: 24,
                      horizontalMargin: 12,
                      headingRowColor:
                          WidgetStateProperty.all(colors.tableHeaderBackgroundColor),
                      columns: [
                        DataColumn(
                          label: Text('File Name', style: headerStyle),
                          onSort: _onSort,
                        ),
                        DataColumn(
                          label: Text('Size', style: headerStyle),
                          numeric: true,
                          onSort: _onSort,
                        ),
                        DataColumn(
                          label: Text('Progress', style: headerStyle),
                          onSort: _onSort,
                        ),
                        DataColumn(
                          label: Text('Priority', style: headerStyle),
                          onSort: _onSort,
                        ),
                      ],
                      rows: filteredFiles.map((file) {
                        return DataRow(
                          cells: [
                            DataCell(
                              Row(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  Icon(
                                    _getFileIcon(file.name),
                                    size: 14,
                                    color: colors.tableSecondaryTextColor,
                                  ),
                                  const SizedBox(width: 8),
                                  Text(
                                    file.name,
                                    style: fileNameStyle,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ],
                              ),
                            ),
                            DataCell(
                              Text(
                                formatBytesDynamic(file.size.toString()),
                                style: cellStyle,
                              ),
                            ),
                            DataCell(_buildProgressBar(context, file.progress)),
                            DataCell(
                              Container(
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 6,
                                  vertical: 2,
                                ),
                                decoration: BoxDecoration(
                                  color: colors.dataCardBackgroundColor,
                                  borderRadius: BorderRadius.circular(3),
                                  border: Border.all(color: colors.dataCardBorderColor, width: 0.5),
                                ),
                                child: Text(
                                  file.priority,
                                  style: TextStyle(
                                    color: colors.tableSecondaryTextColor,
                                    fontSize: 10,
                                    fontWeight: FontWeight.w500,
                                  ),
                                ),
                              ),
                            ),
                          ],
                        );
                      }).toList(),
                    ),
                  ),
                ),
              );
            },
          ),
        ),
      ],
    );
  }

  Widget _buildProgressBar(BuildContext context, double progress) {
    final colors = context.appColors;
    final percent = (progress * 100).clamp(0.0, 100.0);
    final isDone = percent >= 99.99;

    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        SizedBox(
          width: 90,
          height: 6,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(3),
            child: LinearProgressIndicator(
              value: progress.clamp(0.0, 1.0),
              backgroundColor: colors.tableProgressBarTrackColor,
              valueColor: AlwaysStoppedAnimation<Color>(
                isDone ? colors.tableProgressBarProgressColor : colors.accentBlueColor,
              ),
            ),
          ),
        ),
        const SizedBox(width: 8),
        Text(
          '${percent.toStringAsFixed(1)}%',
          style: TextStyle(
            color: isDone ? colors.tableProgressBarProgressColor : colors.tableSecondaryTextColor,
            fontSize: 11,
            fontWeight: FontWeight.w500,
          ),
        ),
      ],
    );
  }
}
