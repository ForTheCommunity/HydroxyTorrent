import 'package:flutter/material.dart';
import 'package:hydroxy_rc/themes/app_theme.dart';

const String darkThemeCode = 'dark';

class DarkTheme extends AppTheme {
  @override
  String get themeName => 'Hydroxy Dark';
  @override
  String get themeCode => darkThemeCode;
  @override
  Brightness get brightness => Brightness.dark;

  // App Shell & Layout
  @override
  Color get scaffoldBackgroundColor => const Color(0xFF1E1E1E);
  @override
  Color get titleBarBackgroundColor => const Color(0xFF252526);
  @override
  Color get titleBarBorderColor => const Color(0xFF333333);
  @override
  Color get titleBarTextColor => Colors.white;
  @override
  Color get titleBarIconColor => Colors.white70;
  @override
  Color get titleBarLogoColor => const Color(0xFF00FF66);

  @override
  Color get statusBarBackgroundColor => const Color(0xFF1E1E1E);
  @override
  Color get statusBarBorderColor => const Color(0xFF333333);
  @override
  Color get statusBarTextColor => Colors.white70;
  @override
  Color get statusBarIconColor => Colors.white54;

  @override
  Color get scrollbarThumbColor => const Color(0xFF00FF66);
  @override
  Color get scrollbarTrackColor => const Color(0xFF2B2B2B);
  @override
  Color get dividerColor => const Color(0xFF333333);

  // Sidebar & Categories Tree
  @override
  Color get sidebarBackgroundColor => const Color(0xFF181818);
  @override
  Color get sidebarBorderColor => const Color(0xFF2B2B2B);
  @override
  Color get sidebarHeaderTextColor => Colors.white54;
  @override
  Color get sidebarItemTextColor => Colors.white70;
  @override
  Color get sidebarSelectedItemTextColor => Colors.white;
  @override
  Color get sidebarSelectedItemBackgroundColor => const Color(0xFF2A2D2E);
  @override
  Color get sidebarItemHoverColor => const Color(0xFF222425);
  @override
  Color get sidebarIconColor => Colors.white38;
  @override
  Color get sidebarSelectedIconColor => const Color(0xFF00FF66);
  @override
  Color get sidebarBadgeBackgroundColor => const Color(0xFF2D2D2D);
  @override
  Color get sidebarSelectedBadgeBackgroundColor => const Color.fromRGBO(0, 255, 102, 0.2);
  @override
  Color get sidebarBadgeTextColor => Colors.white54;
  @override
  Color get sidebarSelectedBadgeTextColor => const Color(0xFF00FF66);
  @override
  Color get sidebarTreeLineColor => Colors.white12;

  // Search Bar
  @override
  Color get searchBarBackgroundColor => const Color(0xFF161616);
  @override
  Color get searchBarBorderColor => const Color(0xFF383838);
  @override
  Color get searchBarErrorBorderColor => Colors.redAccent;
  @override
  Color get searchBarActiveBorderColor => const Color(0xFF00FF66);
  @override
  Color get searchBarTextColor => Colors.white;
  @override
  Color get searchBarHintColor => Colors.white24;
  @override
  Color get searchBarIconColor => Colors.white38;
  @override
  Color get searchBarRegexActiveBackgroundColor => const Color.fromRGBO(0, 255, 102, 0.18);
  @override
  Color get searchBarRegexActiveTextColor => const Color(0xFF00FF66);
  @override
  Color get searchBarRegexInactiveTextColor => Colors.white38;

  // Torrent Data Table
  @override
  Color get tableHeaderBackgroundColor => const Color(0xFF252526);
  @override
  Color get tableHeaderTextColor => Colors.white70;
  @override
  Color get tableRowBackgroundColor => const Color(0xFF1E1E1E);
  @override
  Color get tableAlternatingRowBackgroundColor => const Color(0xFF222222);
  @override
  Color get tableRowHoverColor => const Color(0xFF2A2D2E);
  @override
  Color get tableRowSelectedColor => const Color(0xFF094771);
  @override
  Color get tableDividerColor => const Color(0xFF383A40);
  @override
  Color get tableColumnSeparatorColor => const Color(0xFF555964);
  @override
  Color get tablePrimaryTextColor => Colors.white;
  @override
  Color get tableSecondaryTextColor => Colors.white54;
  @override
  Color get tableAccentTextColor => const Color(0xFF00FF66);
  @override
  Color get tableProgressBarTrackColor => const Color(0xFF333333);
  @override
  Color get tableProgressBarProgressColor => const Color(0xFF00FF66);

  // Details Panel & Tabs
  @override
  Color get detailsPanelBackgroundColor => const Color(0xFF252525);
  @override
  Color get detailsPanelBorderColor => const Color(0xFF3D3D3D);
  @override
  Color get detailsPanelResizeHandleColor => Colors.white24;
  @override
  Color get tabBarBackgroundColor => const Color(0xFF252525);
  @override
  Color get tabBarActiveIndicatorColor => const Color(0xFF00FF66);
  @override
  Color get tabBarActiveTextColor => Colors.white;
  @override
  Color get tabBarInactiveTextColor => Colors.white54;
  @override
  Color get tabBarActiveIconColor => const Color(0xFF00FF66);
  @override
  Color get tabBarInactiveIconColor => Colors.white38;
  @override
  Color get detailsContentBackgroundColor => const Color(0xFF1B1B1B);
  @override
  Color get detailsEmptyTextColor => Colors.white38;

  // Piece Map
  @override
  Color get pieceMapBackgroundColor => const Color(0xFF202020);
  @override
  Color get pieceMapCompletedPieceColor => const Color(0xFF00FF66);
  @override
  Color get pieceMapBorderColor => const Color(0xFF333333);

  // Data Cards & Status Badges
  @override
  Color get dataCardBackgroundColor => const Color(0xFF242424);
  @override
  Color get dataCardBorderColor => const Color(0xFF333333);
  @override
  Color get dataLabelTextColor => Colors.white54;
  @override
  Color get dataValueTextColor => Colors.white;

  @override
  Color get badgeSuccessBackgroundColor => const Color.fromRGBO(0, 255, 102, 0.15);
  @override
  Color get badgeSuccessTextColor => const Color(0xFF00FF66);
  @override
  Color get badgeWarningBackgroundColor => const Color.fromRGBO(255, 215, 64, 0.15);
  @override
  Color get badgeWarningTextColor => Colors.amberAccent;
  @override
  Color get badgeErrorBackgroundColor => const Color.fromRGBO(255, 82, 82, 0.15);
  @override
  Color get badgeErrorTextColor => Colors.redAccent;
  @override
  Color get badgeInfoBackgroundColor => const Color.fromRGBO(64, 196, 255, 0.15);
  @override
  Color get badgeInfoTextColor => Colors.lightBlueAccent;

  // Context Menus & Tooltips & Snacks
  @override
  Color get popupMenuBackgroundColor => const Color(0xFF252526);
  @override
  Color get popupMenuBorderColor => const Color(0xFF3E3E42);
  @override
  Color get popupMenuTextColor => Colors.white;
  @override
  Color get popupMenuDividerColor => const Color(0xFF3E3E42);
  @override
  Color get popupMenuHoverColor => const Color(0xFF2A2D2E);
  @override
  Color get snackBarBackgroundColor => const Color(0xFF282828);
  @override
  Color get snackBarTextColor => Colors.white;
  @override
  Color get tooltipBackgroundColor => const Color(0xFF252526);
  @override
  Color get tooltipTextColor => Colors.white;

  // Auth / Login Screen
  @override
  Color get authCardBackgroundColor => const Color(0xFF252526);
  @override
  Color get authCardBorderColor => const Color(0xFF3E3E42);
  @override
  Color get authTitleTextColor => Colors.white;
  @override
  Color get authSubtitleTextColor => Colors.white54;
  @override
  Color get authInputBackgroundColor => const Color(0xFF1E1E1E);
  @override
  Color get authInputBorderColor => const Color(0xFF3A3A3C);
  @override
  Color get authInputActiveBorderColor => const Color(0xFF00FF66);
  @override
  Color get authInputTextColor => Colors.white;
  @override
  Color get authInputHintColor => Colors.white24;
  @override
  Color get authButtonBackgroundColor => const Color(0xFF00FF66);
  @override
  Color get authButtonTextColor => Colors.black;
  @override
  Color get authButtonHoverColor => const Color(0xFF33FF88);

  // Metrics, Transfer & Accent Colors
  @override
  Color get downloadSpeedColor => const Color(0xFF00FF66);
  @override
  Color get uploadSpeedColor => Colors.lightBlueAccent;
  @override
  Color get memoryUsageColor => Colors.orangeAccent;
  @override
  Color get accentGreenColor => const Color(0xFF00FF66);
  @override
  Color get accentAmberColor => Colors.amberAccent;
  @override
  Color get accentRedColor => Colors.redAccent;
  @override
  Color get accentBlueColor => Colors.lightBlueAccent;
  @override
  Color get accentPurpleColor => Colors.purpleAccent;
}
