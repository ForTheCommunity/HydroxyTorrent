import 'package:flutter/material.dart';
import 'package:hydroxy_rc/themes/app_theme.dart';

const String matrixThemeCode = 'matrix';

class MatrixTheme extends AppTheme {
  @override
  String get themeName => 'Matrix';
  @override
  String get themeCode => matrixThemeCode;
  @override
  Brightness get brightness => Brightness.dark;

  // App Shell & Layout
  @override
  Color get scaffoldBackgroundColor => const Color(0xFF000000);
  @override
  Color get titleBarBackgroundColor => const Color(0xFF050B05);
  @override
  Color get titleBarBorderColor => const Color(0xFF003B00);
  @override
  Color get titleBarTextColor => const Color(0xFF00FF41);
  @override
  Color get titleBarIconColor => const Color(0xFF00CC33);
  @override
  Color get titleBarLogoColor => const Color(0xFF00FF66);

  @override
  Color get statusBarBackgroundColor => const Color(0xFF000000);
  @override
  Color get statusBarBorderColor => const Color(0xFF003B00);
  @override
  Color get statusBarTextColor => const Color(0xFF00DD33);
  @override
  Color get statusBarIconColor => const Color(0xFF00AA22);

  @override
  Color get scrollbarThumbColor => const Color(0xFF00FF41);
  @override
  Color get scrollbarTrackColor => const Color(0xFF051105);
  @override
  Color get dividerColor => const Color(0xFF003B00);

  // Sidebar & Categories Tree
  @override
  Color get sidebarBackgroundColor => const Color(0xFF030803);
  @override
  Color get sidebarBorderColor => const Color(0xFF003B00);
  @override
  Color get sidebarHeaderTextColor => const Color(0xFF00AA22);
  @override
  Color get sidebarItemTextColor => const Color(0xFF00DD33);
  @override
  Color get sidebarSelectedItemTextColor => const Color(0xFFFFFFFF);
  @override
  Color get sidebarSelectedItemBackgroundColor => const Color(0xFF003311);
  @override
  Color get sidebarItemHoverColor => const Color(0xFF00220A);
  @override
  Color get sidebarIconColor => const Color(0xFF00881B);
  @override
  Color get sidebarSelectedIconColor => const Color(0xFF00FF41);
  @override
  Color get sidebarBadgeBackgroundColor => const Color(0xFF002B0E);
  @override
  Color get sidebarSelectedBadgeBackgroundColor => const Color(0xFF00FF41).withOpacity(0.25);
  @override
  Color get sidebarBadgeTextColor => const Color(0xFF00DD33);
  @override
  Color get sidebarSelectedBadgeTextColor => const Color(0xFF00FF41);
  @override
  Color get sidebarTreeLineColor => const Color(0xFF004415);

  // Search Bar
  @override
  Color get searchBarBackgroundColor => const Color(0xFF020E04);
  @override
  Color get searchBarBorderColor => const Color(0xFF004D12);
  @override
  Color get searchBarErrorBorderColor => const Color(0xFFFF2222);
  @override
  Color get searchBarActiveBorderColor => const Color(0xFF00FF41);
  @override
  Color get searchBarTextColor => const Color(0xFF00FF41);
  @override
  Color get searchBarHintColor => const Color(0xFF00661A);
  @override
  Color get searchBarIconColor => const Color(0xFF00AA22);
  @override
  Color get searchBarRegexActiveBackgroundColor => const Color(0xFF00FF41).withOpacity(0.2);
  @override
  Color get searchBarRegexActiveTextColor => const Color(0xFF00FF41);
  @override
  Color get searchBarRegexInactiveTextColor => const Color(0xFF00661A);

  // Torrent Data Table
  @override
  Color get tableHeaderBackgroundColor => const Color(0xFF050E05);
  @override
  Color get tableHeaderTextColor => const Color(0xFF00CC33);
  @override
  Color get tableRowBackgroundColor => const Color(0xFF000000);
  @override
  Color get tableAlternatingRowBackgroundColor => const Color(0xFF020803);
  @override
  Color get tableRowHoverColor => const Color(0xFF00220A);
  @override
  Color get tableRowSelectedColor => const Color(0xFF004415);
  @override
  Color get tableDividerColor => const Color(0xFF002B0E);
  @override
  Color get tablePrimaryTextColor => const Color(0xFF00FF41);
  @override
  Color get tableSecondaryTextColor => const Color(0xFF00AA22);
  @override
  Color get tableAccentTextColor => const Color(0xFF00FF66);
  @override
  Color get tableProgressBarTrackColor => const Color(0xFF002B0E);
  @override
  Color get tableProgressBarProgressColor => const Color(0xFF00FF41);

  // Details Panel & Tabs
  @override
  Color get detailsPanelBackgroundColor => const Color(0xFF030A04);
  @override
  Color get detailsPanelBorderColor => const Color(0xFF003B00);
  @override
  Color get detailsPanelResizeHandleColor => const Color(0xFF00AA22);
  @override
  Color get tabBarBackgroundColor => const Color(0xFF050E05);
  @override
  Color get tabBarActiveIndicatorColor => const Color(0xFF00FF41);
  @override
  Color get tabBarActiveTextColor => const Color(0xFF00FF41);
  @override
  Color get tabBarInactiveTextColor => const Color(0xFF00881B);
  @override
  Color get tabBarActiveIconColor => const Color(0xFF00FF41);
  @override
  Color get tabBarInactiveIconColor => const Color(0xFF00661A);
  @override
  Color get detailsContentBackgroundColor => const Color(0xFF000501);
  @override
  Color get detailsEmptyTextColor => const Color(0xFF00661A);

  // Piece Map
  @override
  Color get pieceMapBackgroundColor => const Color(0xFF020B03);
  @override
  Color get pieceMapCompletedPieceColor => const Color(0xFF00FF41);
  @override
  Color get pieceMapBorderColor => const Color(0xFF003B00);

  // Data Cards & Status Badges
  @override
  Color get dataCardBackgroundColor => const Color(0xFF040E05);
  @override
  Color get dataCardBorderColor => const Color(0xFF003B00);
  @override
  Color get dataLabelTextColor => const Color(0xFF00AA22);
  @override
  Color get dataValueTextColor => const Color(0xFF00FF41);

  @override
  Color get badgeSuccessBackgroundColor => const Color(0xFF00FF41).withOpacity(0.18);
  @override
  Color get badgeSuccessTextColor => const Color(0xFF00FF41);
  @override
  Color get badgeWarningBackgroundColor => const Color(0xFFDDDD00).withOpacity(0.18);
  @override
  Color get badgeWarningTextColor => const Color(0xFFDDDD00);
  @override
  Color get badgeErrorBackgroundColor => const Color(0xFFFF2222).withOpacity(0.2);
  @override
  Color get badgeErrorTextColor => const Color(0xFFFF3333);
  @override
  Color get badgeInfoBackgroundColor => const Color(0xFF00DDFF).withOpacity(0.18);
  @override
  Color get badgeInfoTextColor => const Color(0xFF00DDFF);

  // Context Menus & Tooltips & Snacks
  @override
  Color get popupMenuBackgroundColor => const Color(0xFF030C04);
  @override
  Color get popupMenuBorderColor => const Color(0xFF004D12);
  @override
  Color get popupMenuTextColor => const Color(0xFF00FF41);
  @override
  Color get popupMenuDividerColor => const Color(0xFF00330D);
  @override
  Color get popupMenuHoverColor => const Color(0xFF002609);
  @override
  Color get snackBarBackgroundColor => const Color(0xFF041205);
  @override
  Color get snackBarTextColor => const Color(0xFF00FF41);
  @override
  Color get tooltipBackgroundColor => const Color(0xFF030C04);
  @override
  Color get tooltipTextColor => const Color(0xFF00FF41);

  // Auth / Login Screen
  @override
  Color get authCardBackgroundColor => const Color(0xFF040F05);
  @override
  Color get authCardBorderColor => const Color(0xFF004D12);
  @override
  Color get authTitleTextColor => const Color(0xFF00FF41);
  @override
  Color get authSubtitleTextColor => const Color(0xFF00AA22);
  @override
  Color get authInputBackgroundColor => const Color(0xFF010601);
  @override
  Color get authInputBorderColor => const Color(0xFF004D12);
  @override
  Color get authInputActiveBorderColor => const Color(0xFF00FF41);
  @override
  Color get authInputTextColor => const Color(0xFF00FF41);
  @override
  Color get authInputHintColor => const Color(0xFF00661A);
  @override
  Color get authButtonBackgroundColor => const Color(0xFF00FF41);
  @override
  Color get authButtonTextColor => const Color(0xFF000000);
  @override
  Color get authButtonHoverColor => const Color(0xFF33FF66);

  // Metrics, Transfer & Accent Colors
  @override
  Color get downloadSpeedColor => const Color(0xFF00FF41);
  @override
  Color get uploadSpeedColor => const Color(0xFF00E5FF);
  @override
  Color get memoryUsageColor => const Color(0xFFFFAA00);
  @override
  Color get accentGreenColor => const Color(0xFF00FF41);
  @override
  Color get accentAmberColor => const Color(0xFFFFD700);
  @override
  Color get accentRedColor => const Color(0xFFFF3333);
  @override
  Color get accentBlueColor => const Color(0xFF00E5FF);
  @override
  Color get accentPurpleColor => const Color(0xFFD500F9);
}
