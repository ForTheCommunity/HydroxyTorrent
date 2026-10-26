import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hydroxy_rc/configs/extensions.dart';
import 'package:hydroxy_rc/routes/routes.dart';
import 'package:hydroxy_rc/routes/routes_name.dart';
import 'package:hydroxy_rc/state/auth_bloc/auth_bloc.dart';
import 'package:hydroxy_rc/state/themes_bloc/themes_bloc.dart';
import 'package:hydroxy_rc/state/torrents_bloc/torrents_bloc.dart';

Future<void> main() async {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MultiBlocProvider(
      providers: [
        BlocProvider(create: (_) => ThemesBloc()),
        BlocProvider(create: (_) => AuthBloc()),
        BlocProvider(create: (_) => TorrentsBloc()),
      ],
      child: BlocBuilder<ThemesBloc, ThemesState>(
        builder: (context, state) {
          return MaterialApp(
            theme: context.appColors.toThemeData(),
            debugShowCheckedModeBanner: false,
            initialRoute: RoutesName.initialScreen,
            onGenerateRoute: Routes.generateRoute,
          );
        },
      ),
    );
  }
}
