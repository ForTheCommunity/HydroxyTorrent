import 'package:flutter/material.dart';
import 'package:hydroxy_rc/themes/app_theme.dart';

const String nordThemeCode = 'nord';

class NordTheme extends AppTheme {
  @override
  String get themeName => 'Nord';
  @override
  String get themeCode => nordThemeCode;
  @override
  Brightness get brightness => Brightness.dark;

  // App Shell & Layout
  @override
  Color get scaffoldBackgroundColor => const Color(0xFF2E3440);
  @override
  Color get titleBarBackgroundColor => const Color(0xFF242933);
  @override
  Color get titleBarBorderColor => const Color(0xFF3B4252);
  @override
  Color get titleBarTextColor => const Color(0xFFECEFF4);
  @override
  Color get titleBarIconColor => const Color(0xFFD8DEE9);
  @override
  Color get titleBarLogoColor => const Color(0xFF88C0D0);

  @override
  Color get statusBarBackgroundColor => const Color(0xFF242933);
  @override
  Color get statusBarBorderColor => const Color(0xFF3B4252);
  @override
  Color get statusBarTextColor => const Color(0xFFD8DEE9);
  @override
  Color get statusBarIconColor => const Color(0xFF4C566A);

  @override
  Color get scrollbarThumbColor => const Color(0xFF88C0D0);
  @override
  Color get scrollbarTrackColor => const Color(0xFF3B4252);
  @override
  Color get dividerColor => const Color(0xFF3B4252);

  // Sidebar & Categories Tree
  @override
  Color get sidebarBackgroundColor => const Color(0xFF242933);
  @override
  Color get sidebarBorderColor => const Color(0xFF3B4252);
  @override
  Color get sidebarHeaderTextColor => const Color(0xFF909BB0);
  @override
  Color get sidebarItemTextColor => const Color(0xFFD8DEE9);
  @override
  Color get sidebarSelectedItemTextColor => const Color(0xFFECEFF4);
  @override
  Color get sidebarSelectedItemBackgroundColor => const Color(0xFF434C5E);
  @override
  Color get sidebarItemHoverColor => const Color(0xFF3B4252);
  @override
  Color get sidebarIconColor => const Color(0xFF81A1C1);
  @override
  Color get sidebarSelectedIconColor => const Color(0xFF88C0D0);
  @override
  Color get sidebarBadgeBackgroundColor => const Color(0xFF3B4252);
  @override
  Color get sidebarSelectedBadgeBackgroundColor => const Color(0xFF88C0D0).withOpacity(0.25);
  @override
  Color get sidebarBadgeTextColor => const Color(0xFFD8DEE9);
  @override
  Color get sidebarSelectedBadgeTextColor => const Color(0xFF88C0D0);
  @override
  Color get sidebarTreeLineColor => const Color(0xFF4C566A);

  // Search Bar
  @override
  Color get searchBarBackgroundColor => const Color(0xFF242933);
  @override
  Color get searchBarBorderColor => const Color(0xFF434C5E);
  @override
  Color get searchBarErrorBorderColor => const Color(0xFFBF616A);
  @override
  Color get searchBarActiveBorderColor => const Color(0xFF88C0D0);
  @override
  Color get searchBarTextColor => const Color(0xFFECEFF4);
  @override
  Color get searchBarHintColor => const Color(0xFF6A7791);
  @override
  Color get searchBarIconColor => const Color(0xFF81A1C1);
  @override
  Color get searchBarRegexActiveBackgroundColor => const Color(0xFF88C0D0).withOpacity(0.2);
  @override
  Color get searchBarRegexActiveTextColor => const Color(0xFF88C0D0);
  @override
  Color get searchBarRegexInactiveTextColor => const Color(0xFF6A7791);

  // Torrent Data Table
  @override
  Color get tableHeaderBackgroundColor => const Color(0xFF282E3A);
  @override
  Color get tableHeaderTextColor => const Color(0xFFD8DEE9);
  @override
  Color get tableRowBackgroundColor => const Color(0xFF2E3440);
  @override
  Color get tableAlternatingRowBackgroundColor => const Color(0xFF333A47);
  @override
  Color get tableRowHoverColor => const Color(0xFF3B4252);
  @override
  Color get tableRowSelectedColor => const Color(0xFF434C5E);
  @override
  Color get tableDividerColor => const Color(0xFF434C5E);
  @override
  Color get tableColumnSeparatorColor => const Color(0xFF5E697F);
  @override
  Color get tablePrimaryTextColor => const Color(0xFFECEFF4);
  @override
  Color get tableSecondaryTextColor => const Color(0xFF909BB0);
  @override
  Color get tableAccentTextColor => const Color(0xFF88C0D0);
  @override
  Color get tableProgressBarTrackColor => const Color(0xFF3B4252);
  @override
  Color get tableProgressBarProgressColor => const Color(0xFFA3BE8C);

  // Details Panel & Tabs
  @override
  Color get detailsPanelBackgroundColor => const Color(0xFF282E3A);
  @override
  Color get detailsPanelBorderColor => const Color(0xFF3B4252);
  @override
  Color get detailsPanelResizeHandleColor => const Color(0xFF4C566A);
  @override
  Color get tabBarBackgroundColor => const Color(0xFF242933);
  @override
  Color get tabBarActiveIndicatorColor => const Color(0xFF88C0D0);
  @override
  Color get tabBarActiveTextColor => const Color(0xFFECEFF4);
  @override
  Color get tabBarInactiveTextColor => const Color(0xFF909BB0);
  @override
  Color get tabBarActiveIconColor => const Color(0xFF88C0D0);
  @override
  Color get tabBarInactiveIconColor => const Color(0xFF6A7791);
  @override
  Color get detailsContentBackgroundColor => const Color(0xFF222730);
  @override
  Color get detailsEmptyTextColor => const Color(0xFF6A7791);

  // Piece Map
  @override
  Color get pieceMapBackgroundColor => const Color(0xFF242933);
  @override
  Color get pieceMapCompletedPieceColor => const Color(0xFFA3BE8C);
  @override
  Color get pieceMapBorderColor => const Color(0xFF3B4252);

  // Data Cards & Status Badges
  @override
  Color get dataCardBackgroundColor => const Color(0xFF2E3440);
  @override
  Color get dataCardBorderColor => const Color(0xFF3B4252);
  @override
  Color get dataLabelTextColor => const Color(0xFF909BB0);
  @override
  Color get dataValueTextColor => const Color(0xFFECEFF4);

  @override
  Color get badgeSuccessBackgroundColor => const Color(0xFFA3BE8C).withOpacity(0.2);
  @override
  Color get badgeSuccessTextColor => const Color(0xFFA3BE8C);
  @override
  Color get badgeWarningBackgroundColor => const Color(0xFFEBCB8B).withOpacity(0.2);
  @override
  Color get badgeWarningTextColor => const Color(0xFFEBCB8B);
  @override
  Color get badgeErrorBackgroundColor => const Color(0xFFBF616A).withOpacity(0.2);
  @override
  Color get badgeErrorTextColor => const Color(0xFFBF616A);
  @override
  Color get badgeInfoBackgroundColor => const Color(0xFF81A1C1).withOpacity(0.2);
  @override
  Color get badgeInfoTextColor => const Color(0xFF88C0D0);

  // Context Menus & Tooltips & Snacks
  @override
  Color get popupMenuBackgroundColor => const Color(0xFF2E3440);
  @override
  Color get popupMenuBorderColor => const Color(0xFF434C5E);
  @override
  Color get popupMenuTextColor => const Color(0xFFECEFF4);
  @override
  Color get popupMenuDividerColor => const Color(0xFF3B4252);
  @override
  Color get popupMenuHoverColor => const Color(0xFF3B4252);
  @override
  Color get snackBarBackgroundColor => const Color(0xFF242933);
  @override
  Color get snackBarTextColor => const Color(0xFFECEFF4);
  @override
  Color get tooltipBackgroundColor => const Color(0xFF2E3440);
  @override
  Color get tooltipTextColor => const Color(0xFFECEFF4);

  // Auth / Login Screen
  @override
  Color get authCardBackgroundColor => const Color(0xFF2E3440);
  @override
  Color get authCardBorderColor => const Color(0xFF434C5E);
  @override
  Color get authTitleTextColor => const Color(0xFFECEFF4);
  @override
  Color get authSubtitleTextColor => const Color(0xFF909BB0);
  @override
  Color get authInputBackgroundColor => const Color(0xFF242933);
  @override
  Color get authInputBorderColor => const Color(0xFF3B4252);
  @override
  Color get authInputActiveBorderColor => const Color(0xFF88C0D0);
  @override
  Color get authInputTextColor => const Color(0xFFECEFF4);
  @override
  Color get authInputHintColor => const Color(0xFF6A7791);
  @override
  Color get authButtonBackgroundColor => const Color(0xFF88C0D0);
  @override
  Color get authButtonTextColor => const Color(0xFF2E3440);
  @override
  Color get authButtonHoverColor => const Color(0xFF8FBCBB);

  // Metrics, Transfer & Accent Colors
  @override
  Color get downloadSpeedColor => const Color(0xFFA3BE8C);
  @override
  Color get uploadSpeedColor => const Color(0xFF88C0D0);
  @override
  Color get memoryUsageColor => const Color(0xFFEBCB8B);
  @override
  Color get accentGreenColor => const Color(0xFFA3BE8C);
  @override
  Color get accentAmberColor => const Color(0xFFEBCB8B);
  @override
  Color get accentRedColor => const Color(0xFFBF616A);
  @override
  Color get accentBlueColor => const Color(0xFF81A1C1);
  @override
  Color get accentPurpleColor => const Color(0xFFB48EAD);
}
