import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hydroxy_rc/state/themes_bloc/themes_bloc.dart';
import 'package:hydroxy_rc/themes/app_theme.dart';
import 'package:hydroxy_rc/themes/dark_theme.dart';

extension AppThemeExtension on BuildContext {
  AppTheme get appColors {
    try {
      return watch<ThemesBloc>().state.currentAppTheme;
    } catch (_) {
      return DarkTheme();
    }
  }
}
