import 'package:flutter/material.dart';

abstract class AppTheme {
  String get themeName;
  String get themeCode;
  Brightness get brightness;

  Brightness get statusBarBrightness => brightness;
  Brightness get statusBarIconBrightness =>
      brightness == Brightness.dark ? Brightness.light : Brightness.dark;

  // App Shell & Layout
  Color get scaffoldBackgroundColor;
  Color get titleBarBackgroundColor;
  Color get titleBarBorderColor;
  Color get titleBarTextColor;
  Color get titleBarIconColor;
  Color get titleBarLogoColor;

  Color get statusBarBackgroundColor;
  Color get statusBarBorderColor;
  Color get statusBarTextColor;
  Color get statusBarIconColor;

  Color get scrollbarThumbColor;
  Color get scrollbarTrackColor;
  Color get dividerColor;

  // Sidebar & Categories Tree
  Color get sidebarBackgroundColor;
  Color get sidebarBorderColor;
  Color get sidebarHeaderTextColor;
  Color get sidebarItemTextColor;
  Color get sidebarSelectedItemTextColor;
  Color get sidebarSelectedItemBackgroundColor;
  Color get sidebarItemHoverColor;
  Color get sidebarIconColor;
  Color get sidebarSelectedIconColor;
  Color get sidebarBadgeBackgroundColor;
  Color get sidebarSelectedBadgeBackgroundColor;
  Color get sidebarBadgeTextColor;
  Color get sidebarSelectedBadgeTextColor;
  Color get sidebarTreeLineColor;

  // Search Bar
  Color get searchBarBackgroundColor;
  Color get searchBarBorderColor;
  Color get searchBarErrorBorderColor;
  Color get searchBarActiveBorderColor;
  Color get searchBarTextColor;
  Color get searchBarHintColor;
  Color get searchBarIconColor;
  Color get searchBarRegexActiveBackgroundColor;
  Color get searchBarRegexActiveTextColor;
  Color get searchBarRegexInactiveTextColor;

  // Torrent Data Table
  Color get tableHeaderBackgroundColor;
  Color get tableHeaderTextColor;
  Color get tableRowBackgroundColor;
  Color get tableAlternatingRowBackgroundColor;
  Color get tableRowHoverColor;
  Color get tableRowSelectedColor;
  Color get tableDividerColor;
  Color get tableColumnSeparatorColor;
  Color get tablePrimaryTextColor;
  Color get tableSecondaryTextColor;
  Color get tableAccentTextColor;
  Color get tableProgressBarTrackColor;
  Color get tableProgressBarProgressColor;

  // Details Panel & Tabs
  Color get detailsPanelBackgroundColor;
  Color get detailsPanelBorderColor;
  Color get detailsPanelResizeHandleColor;
  Color get tabBarBackgroundColor;
  Color get tabBarActiveIndicatorColor;
  Color get tabBarActiveTextColor;
  Color get tabBarInactiveTextColor;
  Color get tabBarActiveIconColor;
  Color get tabBarInactiveIconColor;
  Color get detailsContentBackgroundColor;
  Color get detailsEmptyTextColor;

  // Piece Map
  Color get pieceMapBackgroundColor;
  Color get pieceMapCompletedPieceColor;
  Color get pieceMapBorderColor;

  // Data Cards & Status Badges
  Color get dataCardBackgroundColor;
  Color get dataCardBorderColor;
  Color get dataLabelTextColor;
  Color get dataValueTextColor;

  Color get badgeSuccessBackgroundColor;
  Color get badgeSuccessTextColor;
  Color get badgeWarningBackgroundColor;
  Color get badgeWarningTextColor;
  Color get badgeErrorBackgroundColor;
  Color get badgeErrorTextColor;
  Color get badgeInfoBackgroundColor;
  Color get badgeInfoTextColor;

  // Context Menus & Tooltips & Snacks
  Color get popupMenuBackgroundColor;
  Color get popupMenuBorderColor;
  Color get popupMenuTextColor;
  Color get popupMenuDividerColor;
  Color get popupMenuHoverColor;
  Color get snackBarBackgroundColor;
  Color get snackBarTextColor;
  Color get tooltipBackgroundColor;
  Color get tooltipTextColor;

  // Auth / Login Screen
  Color get authCardBackgroundColor;
  Color get authCardBorderColor;
  Color get authTitleTextColor;
  Color get authSubtitleTextColor;
  Color get authInputBackgroundColor;
  Color get authInputBorderColor;
  Color get authInputActiveBorderColor;
  Color get authInputTextColor;
  Color get authInputHintColor;
  Color get authButtonBackgroundColor;
  Color get authButtonTextColor;
  Color get authButtonHoverColor;

  // Metrics, Transfer & Accent Colors
  Color get downloadSpeedColor;
  Color get uploadSpeedColor;
  Color get memoryUsageColor;
  Color get accentGreenColor;
  Color get accentAmberColor;
  Color get accentRedColor;
  Color get accentBlueColor;
  Color get accentPurpleColor;

  // Convenience Aliases & Fallbacks
  Color get tableRowOddColor => tableAlternatingRowBackgroundColor;
  Color get sidebarTextColor => sidebarItemTextColor;
  Color get sidebarSelectedBackgroundColor => sidebarSelectedItemBackgroundColor;
  Color get sidebarSelectedTextColor => sidebarSelectedItemTextColor;
  Color get detailsPanelTabSelectedBorderColor => tabBarActiveIndicatorColor;
  Color get detailsPanelTabSelectedTextColor => tabBarActiveTextColor;
  Color get detailsPanelTabUnselectedTextColor => tabBarInactiveTextColor;

  // Material ThemeData integration
  ThemeData toThemeData() {
    return ThemeData(
      brightness: brightness,
      scaffoldBackgroundColor: scaffoldBackgroundColor,
      dividerColor: dividerColor,
      cardColor: dataCardBackgroundColor,
      colorScheme: ColorScheme(
        brightness: brightness,
        primary: accentGreenColor,
        onPrimary: Colors.black,
        secondary: accentBlueColor,
        onSecondary: Colors.white,
        error: accentRedColor,
        onError: Colors.white,
        surface: scaffoldBackgroundColor,
        onSurface: tablePrimaryTextColor,
      ),
      scrollbarTheme: ScrollbarThemeData(
        thumbColor: WidgetStateProperty.all(scrollbarThumbColor),
        trackColor: WidgetStateProperty.all(scrollbarTrackColor),
        thickness: WidgetStateProperty.all(6.0),
        radius: const Radius.circular(4.0),
      ),
      tooltipTheme: TooltipThemeData(
        decoration: BoxDecoration(
          color: tooltipBackgroundColor,
          borderRadius: BorderRadius.circular(4),
          border: Border.all(color: popupMenuBorderColor, width: 0.8),
        ),
        textStyle: TextStyle(
          color: tooltipTextColor,
          fontSize: 11,
        ),
      ),
      snackBarTheme: SnackBarThemeData(
        backgroundColor: snackBarBackgroundColor,
        contentTextStyle: TextStyle(
          color: snackBarTextColor,
          fontSize: 13,
          fontWeight: FontWeight.w500,
        ),
        actionTextColor: accentBlueColor,
        disabledActionTextColor: tableSecondaryTextColor,
        behavior: SnackBarBehavior.floating,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(8),
          side: BorderSide(color: popupMenuBorderColor, width: 1.0),
        ),
        elevation: 8,
      ),
    );
  }
}
