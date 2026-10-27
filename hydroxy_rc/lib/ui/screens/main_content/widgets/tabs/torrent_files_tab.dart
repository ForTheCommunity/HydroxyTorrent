import 'dart:async';
import 'package:flutter/material.dart';
import 'package:hydroxy_rc/configs/converter.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/network/api.dart';
import 'package:hydroxy_rc/network/models.dart';
import 'package:hydroxy_rc/ui/widgets/info_card.dart';

class FileTreeNode {
  final String name;
  final String fullPath;
  final bool isDirectory;
  final TorrentFileItem? file;
  final List<FileTreeNode> children = [];

  FileTreeNode({
    required this.name,
    required this.fullPath,
    required this.isDirectory,
    this.file,
  });

  int get totalSize {
    if (!isDirectory && file != null) {
      return file!.size;
    }
    return children.fold<int>(0, (sum, child) => sum + child.totalSize);
  }

  double get progress {
    if (!isDirectory && file != null) {
      return file!.progress;
    }
    final size = totalSize;
    if (size <= 0) return 0.0;
    final downloaded = children.fold<double>(
      0.0,
      (sum, child) => sum + (child.totalSize * child.progress),
    );
    return (downloaded / size).clamp(0.0, 1.0);
  }

  String get priority {
    if (!isDirectory && file != null) {
      return file!.priority;
    }
    if (children.isEmpty) return 'Normal';
    final firstPriority = children.first.priority;
    final allSame = children.every((child) => child.priority == firstPriority);
    return allSame ? firstPriority : 'Mixed';
  }
}

class _VisibleTreeItem {
  final FileTreeNode node;
  final int depth;

  _VisibleTreeItem(this.node, this.depth);
}

class TorrentFilesTab extends StatefulWidget {
  final String infoHash;
  final String torrentName;

  /// When true, files are shown as a vertical list of cards (mobile layout)
  /// instead of a wide data table.
  final bool compact;

  const TorrentFilesTab({
    super.key,
    required this.infoHash,
    required this.torrentName,
    this.compact = false,
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

  int? _sortColumnIndex = 0; // Default sort by Name
  bool _isAscending = true;

  final Set<String> _expandedPaths = {};
  bool _hasAutoExpanded = false;

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
        _expandedPaths.clear();
        _hasAutoExpanded = false;
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

  String _sanitizeRelativePath(String rawPath, String torrentName) {
    var p = rawPath.replaceAll(r'\', '/').trim();
    while (p.startsWith('/')) {
      p = p.substring(1).trim();
    }

    if (torrentName.trim().isNotEmpty) {
      final name = torrentName.trim();
      final idx = p.indexOf(name);
      if (idx != -1) {
        p = p.substring(idx);
      }
    }

    // Strip common system path prefixes if present
    final systemPrefixes = ['home/', 'Users/', 'root/', 'var/', 'tmp/'];
    for (final pref in systemPrefixes) {
      if (p.startsWith(pref)) {
        final parts = p.split('/');
        if (parts.length > 3) {
          p = parts.sublist(3).join('/');
        } else if (parts.length > 2) {
          p = parts.sublist(2).join('/');
        }
        break;
      }
    }

    return p;
  }

  FileTreeNode _findOrCreateDirInList(
    List<FileTreeNode> list,
    String dirName,
    String fullPath,
  ) {
    for (final item in list) {
      if (item.isDirectory && item.name == dirName) {
        return item;
      }
    }
    final newDir = FileTreeNode(
      name: dirName,
      fullPath: fullPath,
      isDirectory: true,
    );
    list.add(newDir);
    return newDir;
  }

  List<FileTreeNode> _buildFileTree(List<TorrentFileItem> files) {
    final rootNodes = <FileTreeNode>[];

    for (final file in files) {
      final cleanPath = _sanitizeRelativePath(file.path, widget.torrentName);
      final segments = cleanPath
          .split('/')
          .map((s) => s.trim())
          .where((s) => s.isNotEmpty)
          .toList();

      if (segments.isEmpty) {
        rootNodes.add(FileTreeNode(
          name: file.name.isNotEmpty ? file.name : 'file_${file.id}',
          fullPath: file.path,
          isDirectory: false,
          file: file,
        ));
        continue;
      }

      List<FileTreeNode> currentList = rootNodes;
      String currentPath = '';

      for (int i = 0; i < segments.length; i++) {
        final segment = segments[i];
        currentPath = currentPath.isEmpty ? segment : '$currentPath/$segment';
        final isLast = (i == segments.length - 1);

        if (isLast) {
          currentList.add(FileTreeNode(
            name: segment,
            fullPath: currentPath,
            isDirectory: false,
            file: file,
          ));
        } else {
          final dirNode = _findOrCreateDirInList(currentList, segment, currentPath);
          currentList = dirNode.children;
        }
      }
    }

    // Auto-expand all folders on initial load so user sees structure
    if (!_hasAutoExpanded && rootNodes.isNotEmpty) {
      void expandAll(List<FileTreeNode> nodes) {
        for (final node in nodes) {
          if (node.isDirectory) {
            _expandedPaths.add(node.fullPath);
            expandAll(node.children);
          }
        }
      }

      expandAll(rootNodes);
      _hasAutoExpanded = true;
    }

    return rootNodes;
  }

  bool _nodeMatchesSearch(FileTreeNode node) {
    if (_searchFilter.isEmpty) return true;
    final q = _searchFilter.toLowerCase();
    if (node.name.toLowerCase().contains(q)) return true;
    if (node.isDirectory) {
      return node.children.any((c) => _nodeMatchesSearch(c));
    }
    return false;
  }

  void _sortTreeNodes(List<FileTreeNode> nodes) {
    if (_sortColumnIndex == null) return;

    nodes.sort((a, b) {
      // Keep directories before files
      if (a.isDirectory != b.isDirectory) {
        return a.isDirectory ? -1 : 1;
      }

      final int cmp;
      switch (_sortColumnIndex) {
        case 0: // Name
          cmp = a.name.toLowerCase().compareTo(b.name.toLowerCase());
          break;
        case 1: // Size
          cmp = a.totalSize.compareTo(b.totalSize);
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

    for (final node in nodes) {
      if (node.isDirectory && node.children.isNotEmpty) {
        _sortTreeNodes(node.children);
      }
    }
  }

  List<_VisibleTreeItem> _getVisibleTreeItems(List<FileTreeNode> rootNodes) {
    final result = <_VisibleTreeItem>[];

    void traverse(List<FileTreeNode> nodes, int depth) {
      for (final node in nodes) {
        if (!_nodeMatchesSearch(node)) continue;

        result.add(_VisibleTreeItem(node, depth));

        if (node.isDirectory) {
          final isExpanded = _expandedPaths.contains(node.fullPath) || _searchFilter.isNotEmpty;
          if (isExpanded) {
            traverse(node.children, depth + 1);
          }
        }
      }
    }

    traverse(rootNodes, 0);
    return result;
  }

  void _toggleExpandAll(List<FileTreeNode> rootNodes) {
    setState(() {
      if (_expandedPaths.isNotEmpty) {
        _expandedPaths.clear();
      } else {
        void expandAll(List<FileTreeNode> nodes) {
          for (final node in nodes) {
            if (node.isDirectory) {
              _expandedPaths.add(node.fullPath);
              expandAll(node.children);
            }
          }
        }

        expandAll(rootNodes);
      }
    });
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
            Icon(Icons.folder_open, size: 32, color: Color.fromRGBO(colors.tableSecondaryTextColor.red, colors.tableSecondaryTextColor.green, colors.tableSecondaryTextColor.blue, 0.4)),
            const SizedBox(height: 8),
            Text(
              'No files found for "${widget.torrentName}"',
              style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 12),
            ),
          ],
        ),
      );
    }

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

    // Build hierarchical folder tree
    final treeNodes = _buildFileTree(rawFiles);
    _sortTreeNodes(treeNodes);

    final visibleTreeItems = _getVisibleTreeItems(treeNodes);

    if (widget.compact) {
      return _buildCompactList(context, rawFiles.length, totalBytes, treeNodes, visibleTreeItems);
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        // Summary & Actions Toolbar
        Container(
          padding: const EdgeInsets.only(bottom: 8),
          child: Row(
            children: [
              Text(
                '${rawFiles.length} file${rawFiles.length == 1 ? '' : 's'} (${formatBytesDynamic(totalBytes.toString())})',
                style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 11),
              ),
              const SizedBox(width: 14),

              // Expand / Collapse All Folders Button
              Tooltip(
                message: _expandedPaths.isNotEmpty ? 'Collapse All Folders' : 'Expand All Folders',
                child: InkWell(
                  onTap: () => _toggleExpandAll(treeNodes),
                  borderRadius: BorderRadius.circular(4),
                  child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3.5),
                    decoration: BoxDecoration(
                      color: colors.dataCardBackgroundColor,
                      borderRadius: BorderRadius.circular(4),
                      border: Border.all(color: colors.dataCardBorderColor, width: 0.8),
                    ),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(
                          _expandedPaths.isNotEmpty
                              ? Icons.unfold_less
                              : Icons.unfold_more,
                          size: 13,
                          color: colors.tableSecondaryTextColor,
                        ),
                        const SizedBox(width: 4),
                        Text(
                          _expandedPaths.isNotEmpty ? 'Collapse All' : 'Expand All',
                          style: TextStyle(
                            color: colors.tableSecondaryTextColor,
                            fontSize: 11,
                            fontWeight: FontWeight.w500,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),

              const Spacer(),

              // Search Filter Bar
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

        // Files Tree Table
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
                          label: Text('Name', style: headerStyle),
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
                      rows: visibleTreeItems.map((item) {
                        final node = item.node;
                        final depth = item.depth;
                        final isDirectory = node.isDirectory;
                        final isExpanded = _expandedPaths.contains(node.fullPath);

                        return DataRow(
                          cells: [
                            // Name column with folder tree hierarchy, indent & click-to-expand
                            DataCell(
                              InkWell(
                                onTap: isDirectory
                                    ? () {
                                        setState(() {
                                          if (isExpanded) {
                                            _expandedPaths.remove(node.fullPath);
                                          } else {
                                            _expandedPaths.add(node.fullPath);
                                          }
                                        });
                                      }
                                    : null,
                                child: Row(
                                  mainAxisSize: MainAxisSize.min,
                                  children: [
                                    SizedBox(width: depth * 18.0),
                                    if (isDirectory) ...[
                                      Icon(
                                        isExpanded
                                            ? Icons.arrow_drop_down
                                            : Icons.arrow_right,
                                        size: 18,
                                        color: colors.tableSecondaryTextColor,
                                      ),
                                      const SizedBox(width: 2),
                                      Icon(
                                        isExpanded
                                            ? Icons.folder_open
                                            : Icons.folder,
                                        size: 16,
                                        color: colors.accentAmberColor,
                                      ),
                                    ] else ...[
                                      const SizedBox(width: 20),
                                      Icon(
                                        _getFileIcon(node.name),
                                        size: 14,
                                        color: colors.tableSecondaryTextColor,
                                      ),
                                    ],
                                    const SizedBox(width: 8),
                                    Text(
                                      node.name,
                                      style: isDirectory
                                          ? TextStyle(
                                              color: colors.tablePrimaryTextColor,
                                              fontSize: 11,
                                              fontWeight: FontWeight.bold,
                                            )
                                          : fileNameStyle,
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ],
                                ),
                              ),
                            ),

                            // Size
                            DataCell(
                              Text(
                                formatBytesDynamic(node.totalSize.toString()),
                                style: cellStyle,
                              ),
                            ),

                            // Progress
                            DataCell(_buildProgressBar(context, node.progress)),

                            // Priority
                            DataCell(
                              Container(
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 6,
                                  vertical: 2,
                                ),
                                decoration: BoxDecoration(
                                  color: colors.dataCardBackgroundColor,
                                  borderRadius: BorderRadius.circular(3),
                                  border: Border.all(
                                    color: colors.dataCardBorderColor,
                                    width: 0.5,
                                  ),
                                ),
                                child: Text(
                                  node.priority,
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

  Widget _buildCompactList(
    BuildContext context,
    int fileCount,
    int totalBytes,
    List<FileTreeNode> treeNodes,
    List<_VisibleTreeItem> visibleItems,
  ) {
    final colors = context.appColors;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 12, 12, 6),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              SizedBox(
                height: 36,
                child: TextField(
                  controller: _searchController,
                  style: TextStyle(color: colors.searchBarTextColor, fontSize: 13),
                  decoration: InputDecoration(
                    hintText: 'Filter files...',
                    hintStyle: TextStyle(color: colors.searchBarHintColor, fontSize: 13),
                    contentPadding: const EdgeInsets.symmetric(horizontal: 8, vertical: 0),
                    isDense: true,
                    prefixIcon: Icon(Icons.search, size: 16, color: colors.searchBarIconColor),
                    suffixIcon: _searchFilter.isNotEmpty
                        ? IconButton(
                            icon: Icon(Icons.clear, size: 16, color: colors.searchBarIconColor),
                            onPressed: () {
                              _searchController.clear();
                              setState(() => _searchFilter = '');
                            },
                          )
                        : null,
                    filled: true,
                    fillColor: colors.searchBarBackgroundColor,
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: BorderSide(color: colors.searchBarBorderColor),
                    ),
                    enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: BorderSide(color: colors.searchBarBorderColor),
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: BorderSide(color: colors.searchBarActiveBorderColor),
                    ),
                  ),
                  onChanged: (val) => setState(() => _searchFilter = val),
                ),
              ),
              const SizedBox(height: 8),
              Row(
                children: [
                  Expanded(
                    child: Text(
                      '$fileCount file${fileCount == 1 ? '' : 's'} • ${formatBytesDynamic(totalBytes.toString())}',
                      style: TextStyle(
                        color: colors.tablePrimaryTextColor,
                        fontSize: 13,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                  ),
                  TextButton.icon(
                    style: TextButton.styleFrom(
                      foregroundColor: colors.tableSecondaryTextColor,
                      visualDensity: VisualDensity.compact,
                    ),
                    onPressed: () => _toggleExpandAll(treeNodes),
                    icon: Icon(_expandedPaths.isNotEmpty ? Icons.unfold_less : Icons.unfold_more, size: 16),
                    label: Text(
                      _expandedPaths.isNotEmpty ? 'Collapse All' : 'Expand All',
                      style: const TextStyle(fontSize: 12),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
        Expanded(
          child: ListView.builder(
            padding: const EdgeInsets.fromLTRB(12, 0, 12, 12),
            itemCount: visibleItems.length,
            itemBuilder: (context, index) {
              final item = visibleItems[index];
              final node = item.node;
              final isDirectory = node.isDirectory;
              final isExpanded = _expandedPaths.contains(node.fullPath) || _searchFilter.isNotEmpty;
              final percent = (node.progress * 100).clamp(0.0, 100.0);
              final isDone = percent >= 99.99;
              final progressColor = isDone ? colors.tableProgressBarProgressColor : colors.accentBlueColor;

              return InfoCard(
                margin: EdgeInsets.only(left: (item.depth * 14.0).clamp(0.0, 56.0), bottom: 8),
                padding: const EdgeInsets.fromLTRB(10, 10, 12, 10),
                onTap: isDirectory
                    ? () {
                        setState(() {
                          if (_expandedPaths.contains(node.fullPath)) {
                            _expandedPaths.remove(node.fullPath);
                          } else {
                            _expandedPaths.add(node.fullPath);
                          }
                        });
                      }
                    : null,
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Container(
                      width: 34,
                      height: 34,
                      decoration: BoxDecoration(
                        color: Color.fromRGBO(
                          (isDirectory ? colors.accentAmberColor : colors.accentBlueColor).red,
                          (isDirectory ? colors.accentAmberColor : colors.accentBlueColor).green,
                          (isDirectory ? colors.accentAmberColor : colors.accentBlueColor).blue,
                          0.15,
                        ),
                        borderRadius: BorderRadius.circular(8),
                      ),
                      child: Icon(
                        isDirectory ? (isExpanded ? Icons.folder_open : Icons.folder) : _getFileIcon(node.name),
                        size: 18,
                        color: isDirectory ? colors.accentAmberColor : colors.accentBlueColor,
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Expanded(
                                child: Text(
                                  node.name,
                                  maxLines: 2,
                                  overflow: TextOverflow.ellipsis,
                                  style: TextStyle(
                                    color: colors.tablePrimaryTextColor,
                                    fontSize: 12,
                                    fontWeight: isDirectory ? FontWeight.bold : FontWeight.w500,
                                  ),
                                ),
                              ),
                              if (isDirectory)
                                Icon(
                                  isExpanded ? Icons.expand_less : Icons.expand_more,
                                  size: 18,
                                  color: colors.tableSecondaryTextColor,
                                ),
                            ],
                          ),
                          const SizedBox(height: 6),
                          Row(
                            children: [
                              Expanded(
                                child: ClipRRect(
                                  borderRadius: BorderRadius.circular(3),
                                  child: LinearProgressIndicator(
                                    value: node.progress.clamp(0.0, 1.0),
                                    minHeight: 5,
                                    backgroundColor: colors.tableProgressBarTrackColor,
                                    valueColor: AlwaysStoppedAnimation<Color>(progressColor),
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
                          ),
                          const SizedBox(height: 6),
                          Row(
                            children: [
                              Text(
                                formatBytesDynamic(node.totalSize.toString()),
                                style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 11),
                              ),
                              if (isDirectory) ...[
                                Text(
                                  '  •  ${node.children.length} item${node.children.length == 1 ? '' : 's'}',
                                  style: TextStyle(color: colors.tableSecondaryTextColor, fontSize: 11),
                                ),
                              ],
                              const Spacer(),
                              Container(
                                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                                decoration: BoxDecoration(
                                  color: colors.scaffoldBackgroundColor,
                                  borderRadius: BorderRadius.circular(4),
                                  border: Border.all(color: colors.dataCardBorderColor, width: 0.5),
                                ),
                                child: Text(
                                  node.priority,
                                  style: TextStyle(
                                    color: colors.tableSecondaryTextColor,
                                    fontSize: 10,
                                    fontWeight: FontWeight.w500,
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ],
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
